use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

mod session;
mod pivot;
mod trend;
mod levels;
mod liquidity;
mod imbalance;
mod narrative;

use rust_decimal::Decimal;
use ohlcv_engine::Kline;

#[repr(C)]
pub struct PluginOps {
    pub name: *const std::ffi::c_char,
    pub start: unsafe extern "C" fn(*mut c_void),
    pub stop: unsafe extern "C" fn(*mut c_void),
    pub update: unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> usize,
    pub call_endpoint: unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize,
}

struct PluginState {
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    outbox: Arc<Mutex<Vec<serde_json::Value>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(b"ms_analyzer hazir. Istek bekleniyor.".to_vec())),
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
        3 => { // DataValid
            1
        }
        4 => { // DataMonitor
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
            len
        }
        6 => { // Inbox
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(slice) {
                    if msg["action"].as_str() == Some("analyze") {
                        let symbol = msg["symbol"].as_str().unwrap_or("BTCUSDT").to_string();
                        let interval = msg["interval"].as_str().unwrap_or("15m").to_string();
                        let limit = msg["limit"].as_i64().unwrap_or(100);
                        let from = msg["from"].as_str().unwrap_or("unknown").to_string();
                        
                        let max_limit = (limit * 4).max(96).min(1500);
                        
                        let context = serde_json::json!({
                            "original_limit": limit,
                            "caller": from
                        });

                        let request_msg = serde_json::json!({
                            "to": "plugin_ohlcv_fetcher",
                            "from": "plugin_ms_analyzer",
                            "action": "fetch",
                            "symbol": symbol,
                            "interval": interval,
                            "limit": max_limit,
                            "context": context
                        });
                        
                        let mut q = state.outbox.lock().unwrap();
                        q.push(request_msg);
                        
                        let mut guard = state.data.lock().unwrap();
                        *guard = format!("Ohlcv verisi talep edildi: {} bar (max {} bar istendi)", limit, max_limit).into_bytes();
                    } else if msg["action"].as_str() == Some("fetch_response") {
                        if let Some(context) = msg.get("context") {
                            let original_limit = context["original_limit"].as_u64().unwrap_or(100) as usize;
                            let caller = context["caller"].as_str().unwrap_or("");
                            
                            if let Some(data_array) = msg["data"].as_array() {
                                let mut klines = Vec::new();
                                for row in data_array {
                                    if let Some(arr) = row.as_array() {
                                        if arr.len() >= 11 {
                                            let open_time = arr[0].as_u64().unwrap_or(0);
                                            let open = rust_decimal::Decimal::from_str_exact(arr[1].as_str().unwrap_or("0")).unwrap_or_default();
                                            let high = rust_decimal::Decimal::from_str_exact(arr[2].as_str().unwrap_or("0")).unwrap_or_default();
                                            let low = rust_decimal::Decimal::from_str_exact(arr[3].as_str().unwrap_or("0")).unwrap_or_default();
                                            let close = rust_decimal::Decimal::from_str_exact(arr[4].as_str().unwrap_or("0")).unwrap_or_default();
                                            let volume = rust_decimal::Decimal::from_str_exact(arr[5].as_str().unwrap_or("0")).unwrap_or_default();
                                            let close_time = arr[6].as_u64().unwrap_or(0);
                                            let taker_buy_base = rust_decimal::Decimal::from_str_exact(arr[9].as_str().unwrap_or("0")).unwrap_or_default();
                                            
                                            klines.push(Kline {
                                                open_time, open, high, low, close, volume, close_time,
                                                taker_buy_base_asset_volume: taker_buy_base,
                                            });
                                        }
                                    }
                                }
                                
                                if !klines.is_empty() {
                                    let len = klines.len();
                                    
                                    let core_limit = original_limit.min(len);
                                    let amp_limit = (original_limit * 4).min(1500).min(len);
                                    let acute_limit = 96.min(len);
                                    
                                    let core_klines = &klines[len.saturating_sub(core_limit)..];
                                    let amp_klines = &klines[len.saturating_sub(amp_limit)..];
                                    let acute_klines = &klines[len.saturating_sub(acute_limit)..];
                                    
                                    let report = narrative::generate_report(core_klines, amp_klines, acute_klines);
                                    
                                    let response_msg = serde_json::json!({
                                        "to": caller,
                                        "from": "plugin_ms_analyzer",
                                        "action": "analyze_response",
                                        "report": report
                                    });
                                    
                                    let mut q = state.outbox.lock().unwrap();
                                    q.push(response_msg.clone());
                                    
                                    let mut guard = state.data.lock().unwrap();
                                    *guard = serde_json::to_vec_pretty(&response_msg).unwrap_or_default();
                                }
                            }
                        }
                    }
                }
            }
            0
        }
        7 => { // Outbox
            let mut q = state.outbox.lock().unwrap();
            if !q.is_empty() {
                let json_array = serde_json::Value::Array(q.clone());
                q.clear();
                let bytes = serde_json::to_vec(&json_array).unwrap_or_default();
                let len = bytes.len().min(out_max_len);
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                len
            } else {
                0
            }
        }
        _ => 0,
    }
}
