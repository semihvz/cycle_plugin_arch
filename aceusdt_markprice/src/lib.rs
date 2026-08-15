// ═══════════════════════════════════════════════════════════════════
// Plugin: aceusdt_best_price
// Binance Futures BookTicker Stream — ACEUSDT
// HFT C-ABI Eklenti (init_plugin / RawEndpointFn)
// ═══════════════════════════════════════════════════════════════════

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

// ── Plugin State ──────────────────────────────────────────────────
struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    /// RAM'deki canlı veri (en son bookTicker JSON'u, okunabilir formatta)
    data: Arc<Mutex<Vec<u8>>>,
    /// WebSocket stream'ini kapatmak için sinyal
    shutdown_tx: Mutex<Option<tokio::sync::watch::Sender<bool>>>,
}

// ── C-ABI: init_plugin ───────────────────────────────────────────
// Orkestratör bu fonksiyonu çağırarak eklentiyi başlatır.
// state_out: Eklenti dahili state pointer'ı (orkestratör bunu bilmez)
// Dönüş: Endpoint handler fonksiyon pointer'ı
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

// ── C-ABI: Endpoint Handler ──────────────────────────────────────
// Endpoint ID'leri (orchestrator/src/endpoint.rs ile birebir eşleşir):
//   0 = Start, 1 = Stop, 2 = IsWorking, 3 = DataValid,
//   4 = DataMonitor, 5 = RawData, 6 = Inbox, 7 = Outbox
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
        // ── START ─────────────────────────────────────────────
        0 => {
            if state.is_running.load(Ordering::Relaxed) {
                return 0; // Zaten çalışıyor
            }

            let is_running = state.is_running.clone();
            let data = state.data.clone();
            let (tx, rx) = tokio::sync::watch::channel(false);

            *state.shutdown_tx.lock().unwrap() = Some(tx);
            is_running.store(true, Ordering::Relaxed);

            state.runtime.spawn(async move {
                stream_book_ticker(is_running, data, rx).await;
            });

            0
        }

        // ── STOP ──────────────────────────────────────────────
        1 => {
            state.is_running.store(false, Ordering::Relaxed);
            if let Some(tx) = state.shutdown_tx.lock().unwrap().take() {
                let _ = tx.send(true);
            }
            0
        }

        // ── IS_WORKING ────────────────────────────────────────
        2 => {
            let running = state.is_running.load(Ordering::Relaxed);
            if out_max_len >= 1 {
                *out_buf = if running { 1 } else { 0 };
                1
            } else {
                0
            }
        }

        // ── DATA_MONITOR (4) / RAW_DATA (5) ──────────────────
        4 | 5 => {
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

// ── WebSocket Stream Fonksiyonu ──────────────────────────────────
async fn stream_book_ticker(
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    use futures_util::StreamExt;
    use tokio_tungstenite::connect_async;
    use tokio_tungstenite::tungstenite::Message;

    let url = "wss://fstream.binance.com/ws/aceusdt@markPrice@1s";

    let (db_tx, mut db_rx) = tokio::sync::mpsc::unbounded_channel::<serde_json::Value>();
    let db_latency_us = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let db_lat_clone = db_latency_us.clone();

    // SQLite DB Yazıcı Thread (Ayrı Arka Plan Süreci)
    std::thread::spawn(move || {
        if let Ok(conn) = rusqlite::Connection::open("ACEUSDT_data.db") {
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS markprice (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    symbol TEXT,
                    mark_price REAL,
                    index_price REAL,
                    estimated_settle_price REAL,
                    funding_rate REAL,
                    next_funding_time INTEGER,
                    event_time INTEGER,
                    local_recv_time_ms INTEGER
                )",
                [],
            );

            if let Ok(mut stmt) = conn.prepare(
                "INSERT INTO markprice (symbol, mark_price, index_price, estimated_settle_price, funding_rate, next_funding_time, event_time, local_recv_time_ms)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
            ) {
                while let Some(record) = db_rx.blocking_recv() {
                    let start = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as i64;

                    let symbol = record["symbol"].as_str().unwrap_or("");
                    let mark_price: f64 = record["mark_price"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let index_price: f64 = record["index_price"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let est_settle_price: f64 = record["estimated_settle_price"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let funding_rate: f64 = record["funding_rate"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
                    let next_funding_time = record["next_funding_time"].as_i64().unwrap_or(0);
                    let event_time = record["event_time"].as_i64().unwrap_or(0);
                    let local_recv_time_ms = record["local_recv_time_ms"].as_i64().unwrap_or(0);

                    let _ = stmt.execute(rusqlite::params![
                        symbol,
                        mark_price,
                        index_price,
                        est_settle_price,
                        funding_rate,
                        next_funding_time,
                        event_time,
                        local_recv_time_ms
                    ]);

                    let end = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as i64;
                    db_lat_clone.store(end - start, Ordering::Relaxed);
                }
            }
        }
    });

    // Bağlantı koptuğunda otomatik yeniden bağlanma döngüsü
    while is_running.load(Ordering::Relaxed) {
        {
            let mut guard = data.lock().unwrap();
            *guard = b"Binance Futures'a baglaniliyor...".to_vec();
        }

        match connect_async(url).await {
            Ok((ws_stream, _)) => {
                {
                    let mut guard = data.lock().unwrap();
                    *guard = b"Baglanti kuruldu. Veri bekleniyor...".to_vec();
                }

                let (_, mut read) = ws_stream.split();

                loop {
                    tokio::select! {
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Text(text))) => {
                                    use std::time::{SystemTime, UNIX_EPOCH};
                                    let recv_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                                    
                                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                        let mark_price = json["p"].as_str().unwrap_or("0");
                                        let index_price = json["i"].as_str().unwrap_or("0");
                                        let est_settle_price = json["P"].as_str().unwrap_or("0");
                                        let funding_rate = json["r"].as_str().unwrap_or("0");
                                        let next_funding_time = json["T"].as_i64().unwrap_or(0);
                                        let event_time = json["E"].as_i64().unwrap_or(0);
                                        
                                        let recv_ms = recv_time.as_millis() as i64;
                                        let exchange_latency_ms = recv_ms.saturating_sub(event_time);

                                        let write_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
                                        let processing_latency_us = write_time.as_micros().saturating_sub(recv_time.as_micros()) as i64;
                                        let current_db_lat = db_latency_us.load(Ordering::Relaxed);

                                        // RAM'e JSON binary olarak yaz
                                        let output = serde_json::json!({
                                            "symbol": "ACEUSDT",
                                            "mark_price": mark_price,
                                            "index_price": index_price,
                                            "estimated_settle_price": est_settle_price,
                                            "funding_rate": funding_rate,
                                            "next_funding_time": next_funding_time,
                                            "event_time": event_time,
                                            "local_recv_time_ms": recv_ms,
                                            "local_write_time_ms": write_time.as_millis() as i64,
                                            "exchange_latency_ms": exchange_latency_ms,
                                            "processing_latency_us": processing_latency_us,
                                            "db_write_latency_us": current_db_lat
                                        });

                                        let _ = db_tx.send(output.clone());

                                        let bytes = serde_json::to_vec(&output).unwrap_or_default();
                                        let mut guard = data.lock().unwrap();
                                        *guard = bytes;
                                    }
                                }
                                Some(Ok(_)) => {} // Ping/Pong
                                Some(Err(_)) => break, // Bağlantı hatası, yeniden dene
                                None => break,         // Stream kapandı
                            }
                        }
                        _ = shutdown_rx.changed() => {
                            // Stop sinyali alındı
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                let err_msg = format!("Baglanti hatasi: {}. 5 sn sonra tekrar denenecek...", e);
                {
                    let mut guard = data.lock().unwrap();
                    *guard = err_msg.into_bytes();
                }
            }
        }

        // Bağlantı koptuğunda 5 saniye bekle, sonra yeniden dene
        if is_running.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    is_running.store(false, Ordering::Relaxed);
}
