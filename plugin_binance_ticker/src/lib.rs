use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    symbols: Arc<Mutex<Vec<String>>>,
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
        symbols: Arc::new(Mutex::new(Vec::new())),
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

            // Parse payload for dynamic symbols if passed
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                parse_and_set_symbols(slice, &state.symbols);
            }

            let is_running = state.is_running.clone();
            let symbols = state.symbols.lock().unwrap().clone();
            let data = state.data.clone();
            let (tx, rx) = tokio::sync::watch::channel(false);

            *state.shutdown_tx.lock().unwrap() = Some(tx);
            is_running.store(true, Ordering::Relaxed);

            state.runtime.spawn(async move {
                stream_ticker(symbols, is_running, data, rx).await;
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
        3 => { // Inbox / Config payload (set_symbols)
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                parse_and_set_symbols(slice, &state.symbols);
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

fn parse_and_set_symbols(slice: &[u8], symbols_target: &Arc<Mutex<Vec<String>>>) {
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(slice) {
        let mut list = Vec::new();
        if let Some(arr) = json.get("symbols").and_then(|s| s.as_array()) {
            for item in arr {
                if let Some(s) = item.as_str() {
                    list.push(s.to_uppercase());
                }
            }
        } else if let Some(s) = json.get("symbol").and_then(|s| s.as_str()) {
            list.push(s.to_uppercase());
        }
        if !list.is_empty() {
            *symbols_target.lock().unwrap() = list;
        }
    }
}

async fn stream_ticker(
    symbols: Vec<String>,
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;
    use std::time::{SystemTime, UNIX_EPOCH};

    let ws_url = if symbols.is_empty() || symbols.iter().any(|s| s == "ALL") {
        "wss://fstream.binance.com/ws/!bookTicker".to_string()
    } else {
        let streams: Vec<String> = symbols.iter().map(|s| format!("{}@bookTicker", s.to_lowercase())).collect();
        format!("wss://fstream.binance.com/stream?streams={}", streams.join("/"))
    };

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
                                    if !symbol.is_empty() {
                                        let output = serde_json::json!({
                                            "symbol": symbol,
                                            "best_bid": json["b"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                                            "best_bid_qty": json["B"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                                            "best_ask": json["a"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                                            "best_ask_qty": json["A"].as_str().unwrap_or("0").parse::<f64>().unwrap_or(0.0),
                                            "event_time": json["E"].as_i64().unwrap_or(0),
                                            "local_recv_time_ms": recv_ms
                                        });
                                        let mut guard = data.lock().unwrap();
                                        let mut combined: serde_json::Value = serde_json::from_slice(&guard).unwrap_or_else(|_| serde_json::json!({}));
                                        combined[symbol] = output;
                                        *guard = serde_json::to_vec_pretty(&combined).unwrap_or_default();
                                    }
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
