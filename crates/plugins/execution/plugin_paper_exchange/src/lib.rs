pub mod models;
pub mod storage;
pub mod engine;

pub use models::*;
pub use storage::*;
pub use engine::*;

use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use serde_json::Value;

#[allow(dead_code)]
struct PluginState {
    is_running: Arc<AtomicBool>,
    engine: Arc<PaperEngine>,
    data: Arc<Mutex<Vec<u8>>>,
    outbox: Arc<Mutex<Vec<Value>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    
    let db_path = "data/paper_exchange.db";
    let storage_inst = Storage::new(db_path).expect("Could not init SQLite storage");
    let paper_engine = PaperEngine::new(Arc::new(storage_inst));
    
    // Test account
    paper_engine.create_account("admin", 10000.0);

    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        engine: Arc::new(paper_engine),
        data: Arc::new(Mutex::new(b"Paper Exchange HAZIR.".to_vec())),
        outbox: Arc::new(Mutex::new(Vec::new())),
    });

    unsafe {
        *state_out = Box::into_raw(state) as *mut c_void;
    }

    handle_endpoint
}

unsafe extern "C" fn handle_endpoint(
    plugin_state: *mut c_void,
    endpoint_id: u32,
    payload: *const u8,
    payload_len: usize,
    out_buf: *mut u8,
    out_max_len: usize,
) -> usize {
    let state = &*(plugin_state as *const PluginState);

    match endpoint_id {
        0 => { // Start
            state.is_running.store(true, Ordering::Relaxed);
            0
        }
        1 => { // Stop
            state.is_running.store(false, Ordering::Relaxed);
            0
        }
        2 => { // IsWorking
            if state.is_running.load(Ordering::Relaxed) { 1 } else { 0 }
        }
        3 => { // DataValid (Stream from ms_analyzer or others)
            0
        }
        4 => { // DataMonitor (TUI M key view)
            let mut report = String::new();
            report.push_str("=== PAPER EXCHANGE DURUMU ===\n\n");
            
            if let Some(acc) = state.engine.accounts.get("admin") {
                report.push_str(&format!("[ Bakiye ]\nCüzdan: {:.2} USDT | Margin: {:.2} USDT\n\n", acc.wallet_balance, acc.margin_balance));
            }
            
            report.push_str("[ Fiyat Bilgileri (Market Data) ]\n");
            let mut has_prices = false;
            for price_entry in state.engine.latest_prices.iter() {
                let sym = price_entry.key();
                let last_price = price_entry.value();
                let mark_price = state.engine.mark_prices.get(sym).map(|v| *v).unwrap_or(0.0);
                report.push_str(&format!("- {}: Last/Best: {} | Mark: {}\n", sym, last_price, mark_price));
                has_prices = true;
            }
            if !has_prices {
                report.push_str("Henüz fiyat verisi alınmadı.\n");
            }
            report.push_str("\n");
            
            report.push_str("[ Açık Pozisyonlar ]\n");
            let mut has_pos = false;
            if let Some(user_pos) = state.engine.positions.get("admin") {
                for pos in user_pos.iter() {
                    let p = pos.value();
                    if p.amount > 0.0 {
                        has_pos = true;
                        let side_str = if p.side == PositionSide::Long { "LONG" } else { "SHORT" };
                        report.push_str(&format!("- {} {} | Miktar: {:.3} | Giriş: {:.2} | Kaldıraç: {:.0}x | Likidasyon: {:.2} | PnL: {:.2} USDT\n", 
                            p.symbol, side_str, p.amount, p.entry_price, p.leverage, p.liquidation_price, p.unrealized_pnl));
                    }
                }
            }
            if !has_pos { report.push_str("Yok\n"); }
            report.push_str("\n");
            
            report.push_str("[ Bekleyen Emirler ]\n");
            let mut has_order = false;
            for orders in state.engine.active_orders.iter() {
                for o in orders.value().iter() {
                    has_order = true;
                    let type_str = format!("{:?}", o.order_type);
                    let side_str = format!("{:?}", o.side);
                    report.push_str(&format!("- {} {} {} | Fiyat: {} | Stop: {} | Miktar: {}\n",
                        o.symbol, side_str, type_str, o.price, o.stop_price, o.amount));
                }
            }
            if !has_order { report.push_str("Yok\n"); }
            report.push_str("\n");
            
            report.push_str("[ Sistem Logları ]\n");
            if let Ok(msgs) = state.engine.system_messages.lock() {
                for msg in msgs.iter() {
                    report.push_str(&format!("* {}\n", msg));
                }
            }
            
            report.push_str("=============================\n");
            
            let data = report.into_bytes();
            let len = data.len().min(out_max_len);
            std::ptr::copy_nonoverlapping(data.as_ptr(), out_buf, len);
            len
        }
        6 => { // Inbox
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                
                let mut parsed_msg = serde_json::from_slice::<serde_json::Value>(slice);
                let mut stream_id_opt = None;
                
                if parsed_msg.is_err() && payload_len > 32 {
                    // It might be from FlowEngine which prepends a 32-byte stream_id header
                    let header = &slice[0..32];
                    stream_id_opt = Some(std::str::from_utf8(header).unwrap_or("").trim_matches(char::from(0)).to_string());
                    parsed_msg = serde_json::from_slice::<serde_json::Value>(&slice[32..]);
                }

                if let Ok(msg) = parsed_msg {
                    if let Some(stream_id) = stream_id_opt {
                        // Data from FlowEngine streams
                        if stream_id == "stream_bestprice" {
                            if let Some(obj) = msg.as_object() {
                                for (symbol, data) in obj.iter() {
                                    let ask = data.get("best_ask").and_then(|v| v.as_str()).unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                                    if ask > 0.0 {
                                        state.engine.on_last_price_update(symbol, ask);
                                    }
                                }
                            }
                        } else if stream_id == "stream_markprice" {
                            if let Some(obj) = msg.as_object() {
                                for (symbol, data) in obj.iter() {
                                    let mark = data.get("mark_price").and_then(|v| v.as_str()).unwrap_or("0").parse::<f64>().unwrap_or(0.0);
                                    if mark > 0.0 {
                                        state.engine.on_mark_price_update(symbol, mark);
                                    }
                                }
                            }
                        }
                    } else {
                        // Manual input from TUI
                        if let Some(action) = msg.get("action").and_then(|v| v.as_str()) {
                            if action == "submit_order" {
                                match serde_json::from_value::<Order>(msg["data"].clone()) {
                                    Ok(order) => {
                                        let user_id = msg.get("user_id").and_then(|v| v.as_str()).unwrap_or("admin");
                                        if let Err(e) = state.engine.submit_order(user_id, order) {
                                            state.engine.log_msg(format!("Order submit error: {}", e));
                                        }
                                    }
                                    Err(e) => {
                                        state.engine.log_msg(format!("Order parse error: {}", e));
                                    }
                                }
                            } else if action == "close_position" {
                                let user_id = msg.get("user_id").and_then(|v| v.as_str()).unwrap_or("admin");
                                let symbol = msg.get("symbol").and_then(|v| v.as_str()).unwrap_or("");
                                
                                // Submit reverse market orders for all matching positions
                                let mut to_submit = Vec::new();
                                if let Some(user_pos) = state.engine.positions.get(user_id) {
                                    for pos_ref in user_pos.iter() {
                                        let pos = pos_ref.value();
                                        if pos.symbol == symbol && pos.amount > 0.0 {
                                            let rev_side = if pos.side == crate::models::PositionSide::Long { crate::models::OrderSide::Sell } else { crate::models::OrderSide::Buy };
                                            let rev_pos = pos.side.clone();
                                            to_submit.push(Order {
                                                id: format!("pos_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()),
                                                user_id: user_id.to_string(),
                                                symbol: symbol.to_string(),
                                                side: rev_side,
                                                position_side: rev_pos,
                                                order_type: crate::models::OrderType::Market,
                                                price: 0.0,
                                                stop_price: 0.0,
                                                amount: pos.amount,
                                                leverage: pos.leverage,
                                                executed: 0.0,
                                                timestamp: 0,
                                            });
                                        }
                                    }
                                }
                                
                                if !to_submit.is_empty() {
                                    for order in to_submit {
                                        if let Err(e) = state.engine.submit_order(user_id, order) {
                                            state.engine.log_msg(format!("Close Position error: {}", e));
                                        }
                                    }
                                } else {
                                    state.engine.log_msg(format!("No open position for {} to close", symbol));
                                }
                            } else if action == "cancel_order" {
                                if let Some(order_id) = msg.get("order_id").and_then(|v| v.as_str()) {
                                    let ok = state.engine.cancel_order(order_id);
                                    let mut out = state.outbox.lock().unwrap();
                                    out.push(serde_json::json!({ "action": "cancel_order_response", "order_id": order_id, "success": ok }));
                                }
                            } else if action == "cancel_all_orders" {
                                let symbol_opt = msg.get("symbol").and_then(|v| v.as_str());
                                let count = state.engine.cancel_all_orders(symbol_opt);
                                let mut out = state.outbox.lock().unwrap();
                                out.push(serde_json::json!({ "action": "cancel_all_orders_response", "count": count }));
                            } else if action == "deposit" {
                                let user_id = msg.get("user_id").and_then(|v| v.as_str()).unwrap_or("admin");
                                let amount = msg.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
                                if let Ok(new_bal) = state.engine.deposit(user_id, amount) {
                                    let mut out = state.outbox.lock().unwrap();
                                    out.push(serde_json::json!({ "action": "deposit_response", "user_id": user_id, "wallet_balance": new_bal }));
                                }
                            } else if action == "set_balance" {
                                let user_id = msg.get("user_id").and_then(|v| v.as_str()).unwrap_or("admin");
                                let amount = msg.get("amount").and_then(|v| v.as_f64()).unwrap_or(10000.0);
                                if let Ok(new_bal) = state.engine.set_balance(user_id, amount) {
                                    let mut out = state.outbox.lock().unwrap();
                                    out.push(serde_json::json!({ "action": "set_balance_response", "user_id": user_id, "wallet_balance": new_bal }));
                                }
                            } else if action == "close_all_positions" {
                                let user_id = msg.get("user_id").and_then(|v| v.as_str()).unwrap_or("admin");
                                if let Ok(count) = state.engine.close_all_positions(user_id) {
                                    let mut out = state.outbox.lock().unwrap();
                                    out.push(serde_json::json!({ "action": "close_all_positions_response", "count": count }));
                                }
                            } else if action == "get_orders" {
                                let symbol_filter = msg.get("symbol").and_then(|v| v.as_str());
                                let mut list = Vec::new();
                                for entry in state.engine.active_orders.iter() {
                                    let sym = entry.key();
                                    if symbol_filter.is_none() || symbol_filter == Some(sym.as_str()) {
                                        for order in entry.value().iter() {
                                            list.push(order.clone());
                                        }
                                    }
                                }
                                let mut out = state.outbox.lock().unwrap();
                                out.push(serde_json::json!({ "action": "get_orders_response", "orders": list }));
                            } else if action == "get_history" {
                                let limit = msg.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                                if let Ok(history) = state.engine.storage.get_closed_positions(limit) {
                                    let mut out = state.outbox.lock().unwrap();
                                    out.push(serde_json::json!({ "action": "get_history_response", "history": history }));
                                }
                            }
                        }
                    }
                }
            }
            0
        }
        7 => { // Outbox
            let mut out = state.outbox.lock().unwrap();
            if out.is_empty() {
                0
            } else {
                let msg = out.remove(0);
                if let Ok(json_str) = serde_json::to_string(&msg) {
                    let bytes = json_str.as_bytes();
                    let len = bytes.len().min(out_max_len);
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                    len
                } else {
                    0
                }
            }
        }
        _ => 0,
    }
}
