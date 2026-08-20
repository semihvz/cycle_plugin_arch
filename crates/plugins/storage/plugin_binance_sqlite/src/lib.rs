pub mod models;
pub mod storage;

pub use models::*;
pub use storage::*;

use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use serde_json::Value;

struct PluginState {
    is_running: Arc<AtomicBool>,
    storage: Arc<Mutex<Option<Arc<SqliteStorage>>>>,
    db_path: Arc<Mutex<String>>,
}

fn parse_f64(val: &Value) -> f64 {
    if let Some(n) = val.as_f64() {
        n
    } else if let Some(s) = val.as_str() {
        s.parse::<f64>().unwrap_or(0.0)
    } else {
        0.0
    }
}

fn parse_i64(val: &Value) -> i64 {
    if let Some(n) = val.as_i64() {
        n
    } else if let Some(s) = val.as_str() {
        s.parse::<i64>().unwrap_or(0)
    } else {
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        storage: Arc::new(Mutex::new(None)),
        db_path: Arc::new(Mutex::new("data/binance_market_data.db".to_string())),
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
            let mut db_path = "data/binance_market_data.db".to_string();
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(config) = serde_json::from_slice::<Value>(slice) {
                    if let Some(params) = config.get("plugin_params") {
                        if let Some(path) = params.get("db_path").and_then(|p| p.as_str()) {
                            db_path = path.to_string();
                        }
                    }
                }
            }

            *state.db_path.lock().unwrap() = db_path.clone();

            match SqliteStorage::new(&db_path) {
                Ok(storage_inst) => {
                    *state.storage.lock().unwrap() = Some(Arc::new(storage_inst));
                    state.is_running.store(true, Ordering::Relaxed);
                    0
                }
                Err(e) => {
                    eprintln!("[plugin_binance_sqlite] SQLite init error: {}", e);
                    0
                }
            }
        }
        1 => { // Stop
            state.is_running.store(false, Ordering::Relaxed);
            0
        }
        2 => { // IsWorking
            if state.is_running.load(Ordering::Relaxed) { 1 } else { 0 }
        }
        4 => { // DataMonitor (TUI View)
            let mut report = String::new();
            report.push_str("=== BINANCE SQLITE RECORDER STATUS ===\n\n");

            let running = state.is_running.load(Ordering::Relaxed);
            report.push_str(&format!("Status: {}\n", if running { "RUNNING" } else { "STOPPED" }));

            let current_db_path = state.db_path.lock().unwrap().clone();
            report.push_str(&format!("Database File: {}\n", current_db_path));

            let storage_guard = state.storage.lock().unwrap();
            if let Some(storage) = storage_guard.as_ref() {
                let bytes = storage.get_file_size_bytes();
                let mb = bytes as f64 / (1024.0 * 1024.0);
                report.push_str(&format!("DB Size: {:.2} MB ({} bytes)\n\n", mb, bytes));

                let stats = &storage.stats;
                let mark_cnt = stats.mark_price_count.load(Ordering::Relaxed);
                let best_cnt = stats.best_price_count.load(Ordering::Relaxed);
                let trade_cnt = stats.trade_count.load(Ordering::Relaxed);
                let liq_cnt = stats.liquidation_count.load(Ordering::Relaxed);
                let depth_cnt = stats.depth_count.load(Ordering::Relaxed);
                let total = mark_cnt + best_cnt + trade_cnt + liq_cnt + depth_cnt;
                let last_ts = stats.last_insert_time_ms.load(Ordering::Relaxed);

                report.push_str("[ Storage Statistics ]\n");
                report.push_str(&format!("- Mark Price Records: {}\n", mark_cnt));
                report.push_str(&format!("- Best Price Records: {}\n", best_cnt));
                report.push_str(&format!("- Trade Records: {}\n", trade_cnt));
                report.push_str(&format!("- Liquidation Records: {}\n", liq_cnt));
                report.push_str(&format!("- Depth Records: {}\n", depth_cnt));
                report.push_str(&format!("- Total Records Written: {}\n", total));
                report.push_str(&format!("- Last Record Time (ms): {}\n", last_ts));
            } else {
                report.push_str("\nDatabase not initialized yet.\n");
            }
            report.push_str("======================================\n");

            let data = report.into_bytes();
            let len = data.len().min(out_max_len);
            if len > 0 {
                std::ptr::copy_nonoverlapping(data.as_ptr(), out_buf, len);
            }
            len
        }
        6 => { // Inbox (Data routed from FlowEngine)
            if !state.is_running.load(Ordering::Relaxed) {
                return 0;
            }

            if payload_len > 32 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                let header = &slice[0..32];
                let stream_id = std::str::from_utf8(header).unwrap_or("").trim_matches(char::from(0)).to_string();

                let storage_guard = state.storage.lock().unwrap();
                if let Some(storage) = storage_guard.as_ref() {
                    if let Ok(json_data) = serde_json::from_slice::<Value>(&slice[32..]) {
                        match stream_id.as_str() {
                            "stream_markprice" => {
                                if let Some(obj) = json_data.as_object() {
                                    for (symbol, item) in obj.iter() {
                                        let rec = MarkPriceRecord {
                                            symbol: symbol.clone(),
                                            mark_price: parse_f64(&item["mark_price"]),
                                            index_price: parse_f64(&item["index_price"]),
                                            funding_rate: parse_f64(&item["funding_rate"]),
                                            next_funding_time: parse_i64(&item["next_funding_time"]),
                                            event_time: parse_i64(&item["event_time"]),
                                            local_recv_time_ms: parse_i64(&item["local_recv_time_ms"]),
                                        };
                                        let _ = storage.insert_mark_price(&rec);
                                    }
                                }
                            }
                            "stream_bestprice" => {
                                if let Some(obj) = json_data.as_object() {
                                    for (symbol, item) in obj.iter() {
                                        let rec = BestPriceRecord {
                                            symbol: symbol.clone(),
                                            best_bid: parse_f64(&item["best_bid"]),
                                            best_bid_qty: parse_f64(&item["best_bid_qty"]),
                                            best_ask: parse_f64(&item["best_ask"]),
                                            best_ask_qty: parse_f64(&item["best_ask_qty"]),
                                            event_time: parse_i64(&item["event_time"]),
                                            local_recv_time_ms: parse_i64(&item["local_recv_time_ms"]),
                                        };
                                        let _ = storage.insert_best_price(&rec);
                                    }
                                }
                            }
                            "stream_trades" | "stream_aggtrades" => {
                                if let Some(obj) = json_data.as_object() {
                                    for (symbol, item) in obj.iter() {
                                        let rec = TradeRecord {
                                            symbol: symbol.clone(),
                                            trade_id: parse_i64(&item["trade_id"]),
                                            price: parse_f64(&item["price"]),
                                            quantity: parse_f64(&item["quantity"]),
                                            buyer_is_maker: item["buyer_is_maker"].as_bool().unwrap_or(false),
                                            event_time: parse_i64(&item["event_time"]),
                                            local_recv_time_ms: parse_i64(&item["local_recv_time_ms"]),
                                        };
                                        let _ = storage.insert_trade(&rec);
                                    }
                                }
                            }
                            "stream_liquidations" => {
                                if let Some(arr) = json_data.as_array() {
                                    for item in arr {
                                        let rec = LiquidationRecord {
                                            symbol: item["symbol"].as_str().unwrap_or("").to_string(),
                                            side: item["side"].as_str().unwrap_or("").to_string(),
                                            order_type: item["type"].as_str().unwrap_or("").to_string(),
                                            price: parse_f64(&item["price"]),
                                            average_price: parse_f64(&item["average_price"]),
                                            original_qty: parse_f64(&item["original_qty"]),
                                            filled_qty: parse_f64(&item["filled_qty"]),
                                            event_time: parse_i64(&item["event_time"]),
                                            local_recv_time_ms: parse_i64(&item["local_recv_time_ms"]),
                                        };
                                        let _ = storage.insert_liquidation(&rec);
                                    }
                                }
                            }
                            "stream_depth" => {
                                if let Some(obj) = json_data.as_object() {
                                    for (symbol, item) in obj.iter() {
                                        let rec = DepthRecord {
                                            symbol: symbol.clone(),
                                            bids_json: item["bids"].to_string(),
                                            asks_json: item["asks"].to_string(),
                                            last_update_id: parse_i64(&item["last_update_id"]),
                                            event_time: parse_i64(&item["event_time"]),
                                            local_recv_time_ms: parse_i64(&item["local_recv_time_ms"]),
                                        };
                                        let _ = storage.insert_depth(&rec);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            0
        }
        _ => 0,
    }
}
