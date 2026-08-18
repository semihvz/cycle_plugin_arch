use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct StreamConfig {
    stream_type: String, // bookTicker, aggTrade, markPrice@1s, forceOrder, depth, etc.
    symbols: Vec<String>,
    custom_url: Option<String>,
}

struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    config: Arc<Mutex<StreamConfig>>,
    data: Arc<Mutex<Vec<u8>>>,
    shutdown_tx: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .expect("Tokio runtime oluşturulamadı");

    let state = Box::new(PluginState {
        runtime,
        is_running: Arc::new(AtomicBool::new(false)),
        config: Arc::new(Mutex::new(StreamConfig {
            stream_type: "bookTicker".to_string(),
            symbols: Vec::new(),
            custom_url: None,
        })),
        data: Arc::new(Mutex::new(b"{}".to_vec())),
        shutdown_tx: Mutex::new(None),
    });

    *state_out = Box::into_raw(state) as *mut c_void;
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

            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                parse_config(slice, &state.config);
            }

            let is_running = state.is_running.clone();
            let config = state.config.lock().unwrap();
            let stream_type = config.stream_type.clone();
            let symbols = config.symbols.clone();
            let custom_url = config.custom_url.clone();
            drop(config);

            let data = state.data.clone();
            let (tx, rx) = tokio::sync::watch::channel(false);

            *state.shutdown_tx.lock().unwrap() = Some(tx);
            is_running.store(true, Ordering::Relaxed);

            state.runtime.spawn(async move {
                universal_stream_loop(stream_type, symbols, custom_url, is_running, data, rx).await;
            });
            0
        }
        1 => { // Stop
            state.is_running.store(false, Ordering::Relaxed);
            if let Some(tx) = state.shutdown_tx.lock().unwrap().take() {
                let _ = tx.send(true);
            }
            0
        }
        2 => { // IsWorking
            let running = state.is_running.load(Ordering::Relaxed);
            if out_max_len >= 1 {
                *out_buf = if running { 1 } else { 0 };
                1
            } else {
                0
            }
        }
        3 => { // Inbox / Config payload
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                parse_config(slice, &state.config);
            }
            0
        }
        4 | 5 => { // DataMonitor / RawData
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            if len > 0 {
                std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
            }
            len
        }
        _ => 0,
    }
}

fn parse_config(slice: &[u8], config_target: &Arc<Mutex<StreamConfig>>) {
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(slice) {
        let mut cfg = config_target.lock().unwrap();

        if let Some(url) = json.get("url").and_then(|u| u.as_str()) {
            cfg.custom_url = Some(url.to_string());
        }

        if let Some(st) = json.get("stream").and_then(|s| s.as_str()) {
            cfg.stream_type = st.to_string();
        }

        if let Some(arr) = json.get("symbols").and_then(|s| s.as_array()) {
            let mut list = Vec::new();
            for item in arr {
                if let Some(s) = item.as_str() {
                    list.push(s.to_uppercase());
                }
            }
            if !list.is_empty() {
                cfg.symbols = list;
            }
        } else if let Some(s) = json.get("symbol").and_then(|s| s.as_str()) {
            cfg.symbols = vec![s.to_uppercase()];
        }
    }
}

async fn universal_stream_loop(
    stream_type: String,
    symbols: Vec<String>,
    custom_url: Option<String>,
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;
    use std::time::{SystemTime, UNIX_EPOCH};

    let ws_url = if let Some(url) = custom_url {
        url
    } else {
        build_binance_ws_url(&stream_type, &symbols)
    };

    println!("\x1b[94m\x1b[1m[plugin_binance_ws]\x1b[0m WebSocket Akışına Bağlanılıyor: {}", ws_url);

    let mut retry_count = 0;
    while is_running.load(Ordering::Relaxed) {
        if retry_count > 0 {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }

        if let Ok((ws_stream, _)) = connect_async(&ws_url).await {
            let (_, mut read) = ws_stream.split();
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => { return; }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                let recv_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
                                if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(&text) {
                                    let json = wrapper.get("data").unwrap_or(&wrapper);
                                    let symbol = json["s"].as_str().unwrap_or("").to_string();

                                    let mut guard = data.lock().unwrap();
                                    let mut combined: serde_json::Value = serde_json::from_slice(&guard).unwrap_or_else(|_| serde_json::json!({}));

                                    if !symbol.is_empty() {
                                        let mut frame = json.clone();
                                        if let Some(obj) = frame.as_object_mut() {
                                            obj.insert("local_recv_time_ms".to_string(), serde_json::json!(recv_ms));
                                        }
                                        combined[symbol] = frame;
                                    } else if combined.is_array() {
                                        if let Some(arr) = combined.as_array_mut() {
                                            arr.push(json.clone());
                                            if arr.len() > 50 { arr.remove(0); }
                                        }
                                    } else {
                                        combined = json.clone();
                                    }

                                    *guard = serde_json::to_vec_pretty(&combined).unwrap_or_default();
                                }
                            }
                            Some(Err(_)) | None => { break; }
                            _ => {}
                        }
                    }
                }
            }
        }
        retry_count += 1;
    }
}

fn build_binance_ws_url(stream_type: &str, symbols: &[String]) -> String {
    let clean_stream = stream_type.trim();

    if symbols.is_empty() || symbols.iter().any(|s| s == "ALL") {
        if clean_stream == "forceOrder" || clean_stream == "!forceOrder@arr" {
            "wss://fstream.binance.com/ws/!forceOrder@arr".to_string()
        } else if clean_stream == "markPrice" || clean_stream == "markPrice@1s" || clean_stream == "!markPrice@arr@1s" {
            "wss://fstream.binance.com/ws/!<markPrice@arr@1s".to_string()
        } else if clean_stream == "bookTicker" || clean_stream == "!bookTicker" {
            "wss://fstream.binance.com/ws/!bookTicker".to_string()
        } else if clean_stream == "aggTrade" || clean_stream == "!aggTrade@arr" {
            "wss://fstream.binance.com/ws/!aggTrade@arr".to_string()
        } else {
            format!("wss://fstream.binance.com/ws/{}", clean_stream)
        }
    } else {
        let formatted_streams: Vec<String> = symbols.iter().map(|sym| {
            let s = sym.to_lowercase();
            if clean_stream.contains('@') {
                format!("{}@{}", s, clean_stream.split('@').nth(1).unwrap_or(clean_stream))
            } else {
                format!("{}@{}", s, clean_stream)
            }
        }).collect();
        format!("wss://fstream.binance.com/stream?streams={}", formatted_streams.join("/"))
    }
}
