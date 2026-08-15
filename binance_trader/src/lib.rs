use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use std::fs;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};

type HmacSha256 = Hmac<Sha256>;

#[repr(C)]
pub struct PluginOps {
    pub name: *const std::ffi::c_char,
    pub start: unsafe extern "C" fn(*mut c_void),
    pub stop: unsafe extern "C" fn(*mut c_void),
    pub update: unsafe extern "C" fn(*mut c_void, *mut u8, usize) -> usize,
    pub call_endpoint: unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize,
}

#[derive(Clone, serde::Deserialize, serde::Serialize)]
struct Config {
    api_key: String,
    api_secret: String,
}

struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    outbox: Arc<Mutex<Vec<serde_json::Value>>>,
    config: Arc<Mutex<Option<Config>>>,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("Tokio runtime olusturulamadi");

    let mut config_opt = None;
    if let Ok(content) = fs::read_to_string("trader.cfg") {
        if let Ok(cfg) = serde_json::from_str::<Config>(&content) {
            config_opt = Some(cfg);
        }
    } else {
        // Create template
        let template = Config {
            api_key: "YOUR_API_KEY".to_string(),
            api_secret: "YOUR_API_SECRET".to_string(),
        };
        let _ = fs::write("trader.cfg", serde_json::to_string_pretty(&template).unwrap_or_default());
    }

    let status_msg = if config_opt.is_some() {
        "Config yuklendi. Emir gondermeye hazir."
    } else {
        "Config bulunamadi. trader.cfg olusturuldu, lutfen bilgilerinizi girip yeniden baslatin."
    };

    let state = Box::new(PluginState {
        runtime,
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(status_msg.as_bytes().to_vec())),
        outbox: Arc::new(Mutex::new(Vec::new())),
        config: Arc::new(Mutex::new(config_opt)),
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
            unsafe {
                std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
            }
            len
        }
        6 => { // Inbox
            if payload_len > 0 {
                let slice = unsafe { std::slice::from_raw_parts(payload, payload_len) };
                if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(slice) {
                    let action = msg["action"].as_str().unwrap_or("");
                    let caller = msg["from"].as_str().unwrap_or("unknown").to_string();
                    
                    let config_opt = state.config.lock().unwrap().clone();
                    if let Some(config) = config_opt {
                        let outbox = state.outbox.clone();
                        let data = state.data.clone();
                        
                        match action {
                            "get_balance" => {
                                {
                                    let mut guard = data.lock().unwrap();
                                    *guard = b"Bakiye sorgulaniyor...".to_vec();
                                }
                                state.runtime.spawn(async move {
                                    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                                    let query = format!("timestamp={}", ts);
                                    
                                    let mut mac = HmacSha256::new_from_slice(config.api_secret.as_bytes()).unwrap();
                                    mac.update(query.as_bytes());
                                    let signature = hex::encode(mac.finalize().into_bytes());
                                    
                                    let url = format!("https://fapi.binance.com/fapi/v2/balance?{}&signature={}", query, signature);
                                    
                                    let mut headers = HeaderMap::new();
                                    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(&config.api_key).unwrap());
                                    
                                    let client = reqwest::Client::new();
                                    if let Ok(resp) = client.get(&url).headers(headers).send().await {
                                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                                            let response_msg = serde_json::json!({
                                                "to": caller,
                                                "from": "plugin_binance_trader",
                                                "action": "balance_response",
                                                "data": json
                                            });
                                            let mut q = outbox.lock().unwrap();
                                            q.push(response_msg);
                                            
                                            let mut guard = data.lock().unwrap();
                                            *guard = b"Bakiye basariyla alindi.".to_vec();
                                        }
                                    }
                                });
                            }
                            "place_order" => {
                                let symbol = msg["symbol"].as_str().unwrap_or("BTCUSDT").to_string();
                                let side = msg["side"].as_str().unwrap_or("BUY").to_string();
                                let position_side = msg["positionSide"].as_str().unwrap_or("LONG").to_string(); // LONG, SHORT
                                let type_ = msg["type"].as_str().unwrap_or("MARKET").to_string();
                                let quantity = msg["quantity"].as_f64().unwrap_or(0.001);
                                
                                {
                                    let mut guard = data.lock().unwrap();
                                    *guard = format!("Emir gonderiliyor: {} {} {} {}", position_side, side, quantity, symbol).into_bytes();
                                }
                                
                                state.runtime.spawn(async move {
                                    let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();
                                    let query = format!("symbol={}&side={}&positionSide={}&type={}&quantity={}&timestamp={}", 
                                        symbol, side, position_side, type_, quantity, ts);
                                    
                                    let mut mac = HmacSha256::new_from_slice(config.api_secret.as_bytes()).unwrap();
                                    mac.update(query.as_bytes());
                                    let signature = hex::encode(mac.finalize().into_bytes());
                                    
                                    let url = "https://fapi.binance.com/fapi/v1/order";
                                    let body = format!("{}&signature={}", query, signature);
                                    
                                    let mut headers = HeaderMap::new();
                                    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(&config.api_key).unwrap());
                                    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/x-www-form-urlencoded"));
                                    
                                    let client = reqwest::Client::new();
                                    if let Ok(resp) = client.post(url).headers(headers).body(body).send().await {
                                        let status = resp.status();
                                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                                            let response_msg = serde_json::json!({
                                                "to": caller,
                                                "from": "plugin_binance_trader",
                                                "action": "order_response",
                                                "status_code": status.as_u16(),
                                                "data": json
                                            });
                                            let mut q = outbox.lock().unwrap();
                                            q.push(response_msg.clone());
                                            
                                            let mut guard = data.lock().unwrap();
                                            *guard = serde_json::to_vec_pretty(&response_msg).unwrap_or_default();
                                        }
                                    }
                                });
                            }
                            _ => {}
                        }
                    } else {
                        let mut guard = state.data.lock().unwrap();
                        *guard = b"HATA: API Anahtarlari (trader.cfg) yapilandirilmadi!".to_vec();
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
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                }
                len
            } else {
                0
            }
        }
        _ => 0,
    }
}
