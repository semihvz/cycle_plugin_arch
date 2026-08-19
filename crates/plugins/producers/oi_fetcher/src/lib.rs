use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

#[repr(C)]
pub struct PluginOps {
    pub name: *const std::ffi::c_char,
    pub start: unsafe extern "C" fn(*mut c_void),
    pub stop: unsafe extern "C" fn(*mut c_void),
    pub update: unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> usize,
    pub call_endpoint: unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize,
}

struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    outbox: Arc<Mutex<Vec<serde_json::Value>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("Tokio runtime olusturulamadi");

    let state = Box::new(PluginState {
        runtime,
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(b"Hazir. Istek bekleniyor.".to_vec())),
        outbox: Arc::new(Mutex::new(Vec::new())),
    });

    unsafe { *state_out = Box::into_raw(state) as *mut c_void; }

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
                    if msg["action"].as_str() == Some("fetch_oi") {
                        let symbol = msg["symbol"].as_str().unwrap_or("BTCUSDT").to_string();
                        let interval = msg["interval"].as_str().unwrap_or("5m").to_string();
                        let limit = msg["limit"].as_i64().unwrap_or(30);
                        let from = msg["from"].as_str().unwrap_or("").to_string();
                        let context = msg["context"].clone();
                        
                        let outbox = state.outbox.clone();
                        let data = state.data.clone();
                        
                        {
                            let mut guard = data.lock().unwrap();
                            *guard = format!("Istek isleniyor: {} {} {}", symbol, interval, limit).into_bytes();
                        }
                        
                        state.runtime.spawn(async move {
                            // OI verisini çekerken 'period' parametresi kullanılıyor.
                            let url = format!("https://fapi.binance.com/futures/data/openInterestHist?symbol={}&period={}&limit={}", symbol, interval, limit);
                            if let Ok(resp) = reqwest::get(&url).await {
                                if let Ok(oi_data) = resp.json::<serde_json::Value>().await {
                                    let mut response_msg = serde_json::json!({
                                        "to": from,
                                        "action": "fetch_oi_response",
                                        "symbol": symbol,
                                        "interval": interval,
                                        "data": oi_data,
                                        "type": "oi"
                                    });
                                    
                                    if !context.is_null() {
                                        response_msg["context"] = context;
                                    }
                                    
                                    let mut q = outbox.lock().unwrap();
                                    q.push(response_msg);
                                    
                                    let mut guard = data.lock().unwrap();
                                    *guard = format!("OI verisi cekildi, kuyruga eklendi: {} {} {}", symbol, interval, limit).into_bytes();
                                }
                            }
                        });
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
