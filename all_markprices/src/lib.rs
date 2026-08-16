use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;

struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
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
        .expect("Tokio runtime olusturulamadi");

    let state = Box::new(PluginState {
        runtime,
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(b"Baslatilmadi. 's' tusuna basin.".to_vec())),
        shutdown_tx: Mutex::new(None),
    });

    *state_out = Box::into_raw(state) as *mut c_void;
    handle_endpoint
}

unsafe extern "C" fn handle_endpoint(
    plugin_state: *mut c_void,
    endpoint_id: u32,
    _payload: *const u8,
    _payload_len: usize,
    out_buf: *mut u8,
    out_max_len: usize,
) -> usize {
    let state = &*(plugin_state as *const PluginState);

    match endpoint_id {
        0 => { // Start
            if state.is_running.load(Ordering::Relaxed) {
                return 0;
            }
            let is_running = state.is_running.clone();
            let data = state.data.clone();
            let (tx, rx) = tokio::sync::watch::channel(false);

            *state.shutdown_tx.lock().unwrap() = Some(tx);
            is_running.store(true, Ordering::Relaxed);

            state.runtime.spawn(async move {
                stream_combined_markprices(is_running, data, rx).await;
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

async fn stream_combined_markprices(
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    // Combined stream for markPrice@1s
    let url = "wss://fstream.binance.com/stream?streams=btcusdt@markPrice@1s/ethusdt@markPrice@1s/aceusdt@markPrice@1s";

    let mut retry_count = 0;
    while is_running.load(Ordering::Relaxed) {
        {
            let mut guard = data.lock().unwrap();
            *guard = b"Binance Combined Streams baglaniliyor...".to_vec();
        }

        if retry_count > 0 {
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
        }

        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                retry_count = 0;
                {
                    let mut guard = data.lock().unwrap();
                    *guard = b"Baglanti kuruldu. Veri bekleniyor...".to_vec();
                }

                let (_, mut read) = ws_stream.split();
                let mut latest_data = HashMap::new();
                
                loop {
                    tokio::select! {
                        _ = shutdown_rx.changed() => { break; }
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    use std::time::{SystemTime, UNIX_EPOCH};
                                    let recv_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                                    
                                    if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(&text) {
                                        if let Some(json) = wrapper.get("data") {
                                            let symbol = json["s"].as_str().unwrap_or("").to_string();
                                            let mark_price = json["p"].as_str().unwrap_or("0");
                                            let index_price = json["i"].as_str().unwrap_or("0");
                                            let estimated_settle_price = json["P"].as_str().unwrap_or("0");
                                            let funding_rate = json["r"].as_str().unwrap_or("0");
                                            let next_funding_time = json["T"].as_i64().unwrap_or(0);
                                            let event_time = json["E"].as_i64().unwrap_or(0);
                                            
                                            let recv_ms = recv_time.as_millis() as i64;

                                            let output = serde_json::json!({
                                                "mark_price": mark_price,
                                                "index_price": index_price,
                                                "estimated_settle_price": estimated_settle_price,
                                                "funding_rate": funding_rate,
                                                "next_funding_time": next_funding_time,
                                                "event_time": event_time,
                                                "local_recv_time_ms": recv_ms
                                            });

                                            latest_data.insert(symbol.clone(), output.clone());

                                            let mut guard = data.lock().unwrap();
                                            *guard = serde_json::to_vec_pretty(&latest_data).unwrap_or_default();
                                        }
                                    }
                                }
                                Some(Err(_)) => { break; }
                                None => { break; }
                                _ => {}
                            }
                        }
                    }
                }
            }
            Err(_) => {
                retry_count += 1;
            }
        }
    }
}
