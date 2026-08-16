use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    // data holds a serialized JSON of demultiplexed streams:
    // {
    //   "stream_markprice": { "BTCUSDT": { "mark_price": ... } },
    //   "stream_bestprice": { "BTCUSDT": { "best_ask": ... } },
    //   "stream_liquidations": [ ... ],
    // }
    data: Arc<Mutex<Vec<u8>>>,
    shutdown_tx: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Tokio runtime olusturulamadi");

    let state = Box::new(PluginState {
        runtime,
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(br#"{"stream_markprice":{},"stream_bestprice":{},"stream_liquidations":[],"stream_aggtrades":{},"stream_depth":{}}"#.to_vec())),
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
            
            let mut symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string(), "ACEUSDT".to_string()];
            
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(config) = serde_json::from_slice::<serde_json::Value>(slice) {
                    if let Some(params) = config.get("plugin_params") {
                        if let Some(syms) = params.get("symbols").and_then(|s| s.as_array()) {
                            symbols.clear();
                            for s in syms {
                                if let Some(s_str) = s.as_str() {
                                    symbols.push(s_str.to_string());
                                }
                            }
                        }
                    }
                }
            }

            let is_running = state.is_running.clone();
            let data = state.data.clone();
            let (tx, rx) = tokio::sync::watch::channel(false);

            *state.shutdown_tx.lock().unwrap() = Some(tx);
            is_running.store(true, Ordering::Relaxed);

            state.runtime.spawn(async move {
                stream_gateway(symbols, is_running, data, rx).await;
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

async fn stream_gateway(
    symbols: Vec<String>,
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    let mut public_streams = Vec::new();
    let mut market_streams = Vec::new();
    for sym in &symbols {
        let s = sym.to_lowercase();
        market_streams.push(format!("{}@markPrice@1s", s));
        public_streams.push(format!("{}@bookTicker", s));
        public_streams.push(format!("{}@depth20@100ms", s));
        public_streams.push(format!("{}@trade", s));
    }
    market_streams.push("!forceOrder@arr".to_string());

    let public_url = format!("wss://fstream.binance.com/public/stream?streams={}", public_streams.join("/"));
    let market_url = format!("wss://fstream.binance.com/market/stream?streams={}", market_streams.join("/"));

    let is_running_1 = is_running.clone();
    let data_1 = data.clone();
    let shutdown_rx_1 = shutdown_rx.clone();

    let is_running_2 = is_running.clone();
    let data_2 = data.clone();
    let shutdown_rx_2 = shutdown_rx;

    let handle1 = tokio::spawn(async move {
        run_websocket_loop(public_url, is_running_1, data_1, shutdown_rx_1).await;
    });

    let handle2 = tokio::spawn(async move {
        run_websocket_loop(market_url, is_running_2, data_2, shutdown_rx_2).await;
    });

    let _ = tokio::join!(handle1, handle2);
}

async fn run_websocket_loop(
    url: String,
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;
    use std::time::{SystemTime, UNIX_EPOCH};

    let mut retry_count = 0;
    while is_running.load(Ordering::Relaxed) {
        if retry_count > 0 {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }

        if let Ok((ws_stream, _)) = connect_async(&url).await {
            let (_, mut read) = ws_stream.split();
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => { break; }
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                let recv_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
                                if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(&text) {
                                    if let Some(stream_name) = wrapper.get("stream").and_then(|s| s.as_str()) {
                                        if let Some(json) = wrapper.get("data") {
                                            let e_type = json.get("e").and_then(|v| v.as_str()).unwrap_or("");
                                            
                                            let mut guard = data.lock().unwrap();
                                            let mut combined: serde_json::Value = serde_json::from_slice(&guard).unwrap_or_else(|_| serde_json::json!({}));
                                            if !combined.is_object() {
                                                combined = serde_json::json!({});
                                            }
                                            {
                                                let obj = combined.as_object_mut().unwrap();
                                                if !obj.contains_key("stream_markprice") { obj.insert("stream_markprice".to_string(), serde_json::json!({})); }
                                                if !obj.contains_key("stream_bestprice") { obj.insert("stream_bestprice".to_string(), serde_json::json!({})); }
                                                if !obj.contains_key("stream_liquidations") { obj.insert("stream_liquidations".to_string(), serde_json::json!([])); }
                                                if !obj.contains_key("stream_aggtrades") { obj.insert("stream_aggtrades".to_string(), serde_json::json!({})); }
                                                if !obj.contains_key("stream_depth") { obj.insert("stream_depth".to_string(), serde_json::json!({})); }
                                            }

                                            if e_type == "forceOrder" {
                                                if let Some(o) = json.get("o") {
                                                    let symbol = o["s"].as_str().unwrap_or("").to_string();
                                                    let output = serde_json::json!({
                                                        "symbol": symbol,
                                                        "side": o["S"].as_str().unwrap_or(""),
                                                        "type": o["o"].as_str().unwrap_or(""),
                                                        "price": o["p"].as_str().unwrap_or("0"),
                                                        "average_price": o["ap"].as_str().unwrap_or("0"),
                                                        "original_qty": o["q"].as_str().unwrap_or("0"),
                                                        "filled_qty": o["z"].as_str().unwrap_or("0"),
                                                        "event_time": json["E"].as_i64().unwrap_or(0),
                                                        "local_recv_time_ms": recv_ms
                                                    });
                                                    if let Some(arr) = combined["stream_liquidations"].as_array_mut() {
                                                        arr.push(output);
                                                        if arr.len() > 50 { arr.remove(0); }
                                                    }
                                                }
                                            } else if e_type == "markPriceUpdate" {
                                                let symbol = json["s"].as_str().unwrap_or("").to_string();
                                                let output = serde_json::json!({
                                                    "mark_price": json["p"].as_str().unwrap_or("0"),
                                                    "index_price": json["i"].as_str().unwrap_or("0"),
                                                    "estimated_settle_price": json["P"].as_str().unwrap_or("0"),
                                                    "funding_rate": json["r"].as_str().unwrap_or("0"),
                                                    "next_funding_time": json["T"].as_i64().unwrap_or(0),
                                                    "event_time": json["E"].as_i64().unwrap_or(0),
                                                    "local_recv_time_ms": recv_ms
                                                });
                                                combined["stream_markprice"][symbol] = output;
                                            } else if e_type == "bookTicker" || stream_name.to_lowercase().ends_with("@bookticker") {
                                                let symbol = json["s"].as_str().unwrap_or("").to_string();
                                                let output = serde_json::json!({
                                                    "best_bid": json["b"].as_str().unwrap_or("0"),
                                                    "best_bid_qty": json["B"].as_str().unwrap_or("0"),
                                                    "best_ask": json["a"].as_str().unwrap_or("0"),
                                                    "best_ask_qty": json["A"].as_str().unwrap_or("0"),
                                                    "event_time": json["E"].as_i64().unwrap_or(0),
                                                    "local_recv_time_ms": recv_ms
                                                });
                                                combined["stream_bestprice"][symbol] = output;
                                            } else if e_type == "aggTrade" || e_type == "trade" || stream_name.to_lowercase().ends_with("@trade") {
                                                let symbol = json["s"].as_str().unwrap_or("").to_string();
                                                let output = serde_json::json!({
                                                    "trade_id": json["t"].as_i64().unwrap_or(0),
                                                    "price": json["p"].as_str().unwrap_or("0"),
                                                    "quantity": json["q"].as_str().unwrap_or("0"),
                                                    "buyer_is_maker": json["m"].as_bool().unwrap_or(false),
                                                    "event_time": json["E"].as_i64().unwrap_or(0),
                                                    "local_recv_time_ms": recv_ms
                                                });
                                                combined["stream_aggtrades"][symbol] = output;
                                            } else if stream_name.to_lowercase().ends_with("@depth20@100ms") {
                                                let symbol = stream_name.split('@').next().unwrap_or("").to_uppercase();
                                                let output = serde_json::json!({
                                                    "bids": json["b"],
                                                    "asks": json["a"],
                                                    "last_update_id": json["lastUpdateId"].as_i64().unwrap_or(0),
                                                    "event_time": json["E"].as_i64().unwrap_or(0),
                                                    "local_recv_time_ms": recv_ms
                                                });
                                                combined["stream_depth"][symbol] = output;
                                            }

                                            *guard = serde_json::to_vec_pretty(&combined).unwrap_or_default();
                                        }
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
