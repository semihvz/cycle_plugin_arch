use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

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
    current_config: Arc<Mutex<Option<(String, String, i64)>>>,
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
        current_config: Arc::new(Mutex::new(None)),
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
            if state.is_running.load(Ordering::Relaxed) {
                return 0;
            }
            state.is_running.store(true, Ordering::Relaxed);
            
            // Okunan dinamik parametreleri payload'dan al
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(params) = serde_json::from_slice::<serde_json::Value>(slice) {
                    let symbol = params["symbol"].as_str().unwrap_or("BTCUSDT").to_string();
                    let interval = params["interval"].as_str().unwrap_or("15m").to_string();
                    let limit = params["limit"].as_i64().unwrap_or(1500);
                    let mut config_guard = state.current_config.lock().unwrap();
                    *config_guard = Some((symbol, interval, limit));
                }
            }
            
            let is_running = state.is_running.clone();
            let data = state.data.clone();
            let current_config = state.current_config.clone();
            
            state.runtime.spawn(async move {
                while is_running.load(Ordering::Relaxed) {
                    let config_opt = { current_config.lock().unwrap().clone() };
                    
                    if let Some((symbol, interval, limit)) = config_opt {
                        let url = format!("https://fapi.binance.com/fapi/v1/klines?symbol={}&interval={}&limit={}", symbol, interval, limit);
                        if let Ok(resp) = reqwest::get(&url).await {
                            if let Ok(klines) = resp.json::<serde_json::Value>().await {
                                let mut guard = data.lock().unwrap();
                                *guard = serde_json::to_vec(&klines).unwrap_or_default();
                            }
                        }
                    }
                    
                    // Fetch every 10 seconds for real-time OHLCV updates (can be adjusted)
                    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
                }
            });
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
        4 | 5 => { // DataMonitor & RawData
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            if len > 0 {
                std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
            }
            len
        }
        6 => { // Inbox
            if payload_len > 32 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                let data_slice = &slice[32..];
                
                if let Ok(req) = serde_json::from_slice::<serde_json::Value>(data_slice) {
                    let symbol = req["symbol"].as_str().unwrap_or("BTCUSDT").to_string();
                    let interval = req["interval"].as_str().unwrap_or("15m").to_string();
                    let limit = req["limit"].as_i64().unwrap_or(1500);
                    
                    let mut config_guard = state.current_config.lock().unwrap();
                    *config_guard = Some((symbol, interval, limit));
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
