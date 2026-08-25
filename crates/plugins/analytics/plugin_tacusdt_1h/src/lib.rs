use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use rusqlite::Connection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bar {
    pub open_time: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub close_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSetup {
    pub id: usize,
    pub symbol: String,
    pub interval: String,
    pub entry_time: u64,
    pub entry_time_utc: String,
    pub entry_price: f64,
    pub lowest_100: f64,
    pub atr_14: f64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub position_size_usdt: f64,
    pub risk_usdt: f64,
    pub reward_usdt: f64,
    pub exit_price: Option<f64>,
    pub pnl_usdt: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectorStatus {
    pub symbol: String,
    pub interval: String,
    pub total_bars_fetched: usize,
    pub total_trades_detected: usize,
    pub total_lookback_bars_persisted: usize,
    pub win_rate_pct: f64,
    pub net_pnl_usdt: f64,
    pub db_file_path: String,
    pub db_size_mb: f64,
    pub last_trade_summary: String,
    pub last_updated_ms: u64,
}

fn parse_f64(val: &Value) -> f64 {
    if let Some(s) = val.as_str() {
        s.parse::<f64>().unwrap_or(0.0)
    } else if let Some(f) = val.as_f64() {
        f
    } else if let Some(i) = val.as_i64() {
        i as f64
    } else {
        0.0
    }
}

fn parse_u64(val: &Value) -> u64 {
    if let Some(u) = val.as_u64() {
        u
    } else if let Some(i) = val.as_i64() {
        i as u64
    } else if let Some(s) = val.as_str() {
        s.parse::<u64>().unwrap_or(0)
    } else {
        0
    }
}

fn format_timestamp(ts_ms: u64) -> String {
    let secs = (ts_ms / 1000) as i64;
    let days = secs / 86400;
    let rem_secs = secs % 86400;
    let hours = rem_secs / 3600;
    let mins = (rem_secs % 3600) / 60;

    let mut year = 1970;
    let mut day_count = days;
    loop {
        let leap = if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) { 366 } else { 365 };
        if day_count < leap {
            break;
        }
        day_count -= leap;
        year += 1;
    }
    let leap = if (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0) { 1 } else { 0 };
    let month_days = [31, 28 + leap, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1;
    for &md in &month_days {
        if day_count < md {
            break;
        }
        day_count -= md;
        month += 1;
    }
    let day = day_count + 1;

    format!("{:04}-{:02}-{:02} {:02}:{:02} UTC", year, month, day, hours, mins)
}

pub fn calculate_atr_series(bars: &[Bar], period: usize) -> Vec<f64> {
    let mut tr_list = Vec::with_capacity(bars.len());
    for i in 0..bars.len() {
        let tr = if i == 0 {
            bars[i].high - bars[i].low
        } else {
            let hl = bars[i].high - bars[i].low;
            let hp = (bars[i].high - bars[i - 1].close).abs();
            let lp = (bars[i].low - bars[i - 1].close).abs();
            hl.max(hp).max(lp)
        };
        tr_list.push(tr);
    }

    let mut atr = vec![0.0; bars.len()];
    if bars.len() < period {
        return atr;
    }

    let first_sma: f64 = tr_list[0..period].iter().sum::<f64>() / period as f64;
    atr[period - 1] = first_sma;

    let period_f = period as f64;
    let mut prev_atr = first_sma;
    for i in period..bars.len() {
        let current_atr = (prev_atr * (period_f - 1.0) + tr_list[i]) / period_f;
        atr[i] = current_atr;
        prev_atr = current_atr;
    }

    atr
}

pub fn process_and_persist_tacusdt_1h(symbol: &str, interval: &str, bars: &[Bar], db_path: &str) -> CollectorStatus {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let atr_series = calculate_atr_series(bars, 14);
    let lookback = 100;
    let fixed_pos_size = 50.0; // 50 USDT

    let mut detected_trades = Vec::new();
    let mut trade_id = 1;

    for i in lookback..bars.len() {
        let entry_bar = &bars[i];
        let entry_price = entry_bar.open;

        let window_100 = &bars[(i - lookback)..i];
        let lowest_100 = window_100.iter().map(|b| b.low).fold(f64::INFINITY, f64::min);
        let atr = atr_series[i - 1].max(0.00001);
        let raw_sl = lowest_100 - (2.0 * atr);
        let sl_dist = (entry_price - raw_sl).max(entry_price * 0.005);
        let stop_loss = entry_price - sl_dist;
        let take_profit = entry_price + (2.0 * sl_dist);

        let risk_ratio = sl_dist / entry_price;
        let risk_usdt = fixed_pos_size * risk_ratio;
        let reward_usdt = 2.0 * risk_usdt;

        let mut closed = false;
        let mut exit_price = None;
        let mut status = "OPEN".to_string();
        let mut pnl_usdt = 0.0;

        for k in i..bars.len() {
            let sim_bar = &bars[k];
            if sim_bar.low <= stop_loss && sim_bar.high >= take_profit {
                closed = true;
                exit_price = Some(stop_loss);
                status = "LOSS".to_string();
                pnl_usdt = -risk_usdt;
                break;
            } else if sim_bar.high >= take_profit {
                closed = true;
                exit_price = Some(take_profit);
                status = "WIN".to_string();
                pnl_usdt = reward_usdt;
                break;
            } else if sim_bar.low <= stop_loss {
                closed = true;
                exit_price = Some(stop_loss);
                status = "LOSS".to_string();
                pnl_usdt = -risk_usdt;
                break;
            }
        }

        if closed {
            detected_trades.push((
                TradeSetup {
                    id: trade_id,
                    symbol: symbol.to_string(),
                    interval: interval.to_string(),
                    entry_time: entry_bar.open_time,
                    entry_time_utc: format_timestamp(entry_bar.open_time),
                    entry_price,
                    lowest_100,
                    atr_14: atr,
                    stop_loss,
                    take_profit,
                    position_size_usdt: fixed_pos_size,
                    risk_usdt,
                    reward_usdt,
                    exit_price,
                    pnl_usdt,
                    status,
                },
                window_100.to_vec(),
            ));
            trade_id += 1;
        }
    }

    let wins_count = detected_trades.iter().filter(|(t, _)| t.status == "WIN").count();
    let total_trades = detected_trades.len();
    let net_pnl_usdt: f64 = detected_trades.iter().map(|(t, _)| t.pnl_usdt).sum();
    let win_rate_pct = if total_trades > 0 { (wins_count as f64 / total_trades as f64) * 100.0 } else { 0.0 };

    // Persist to SQLite Database in data/ directory
    let mut total_lookback_persisted = 0;
    let mut last_summary = "Henüz işlem kaydedilmedi".to_string();

    if let Ok(mut conn) = Connection::open(db_path) {
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS closed_trades (
                trade_id INTEGER PRIMARY KEY,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                entry_time_utc TEXT NOT NULL,
                entry_unix_ms INTEGER NOT NULL,
                entry_price REAL NOT NULL,
                lowest_100_price REAL NOT NULL,
                atr_14 REAL NOT NULL,
                stop_loss_price REAL NOT NULL,
                take_profit_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                position_size_usdt REAL NOT NULL,
                risk_usdt REAL NOT NULL,
                target_reward_usdt REAL NOT NULL,
                result TEXT NOT NULL,
                pnl_usdt REAL NOT NULL
            );",
            [],
        );

        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS trade_lookback_bars (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                trade_id INTEGER NOT NULL,
                bar_offset INTEGER NOT NULL,
                open_time_ms INTEGER NOT NULL,
                open_time_utc TEXT NOT NULL,
                open REAL NOT NULL,
                high REAL NOT NULL,
                low REAL NOT NULL,
                close REAL NOT NULL,
                volume REAL NOT NULL,
                close_time_ms INTEGER NOT NULL,
                FOREIGN KEY (trade_id) REFERENCES closed_trades (trade_id)
            );",
            [],
        );

        if let Ok(tx) = conn.transaction() {
            let _ = tx.execute("DELETE FROM closed_trades;", []);
            let _ = tx.execute("DELETE FROM trade_lookback_bars;", []);

            for (t, lookback_bars) in &detected_trades {
                let _ = tx.execute(
                    "INSERT INTO closed_trades VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16);",
                    rusqlite::params![
                        t.id as i64, t.symbol, "LONG", t.entry_time_utc, t.entry_time as i64,
                        t.entry_price, t.lowest_100, t.atr_14, t.stop_loss,
                        t.take_profit, t.exit_price.unwrap_or(0.0), t.position_size_usdt,
                        t.risk_usdt, t.reward_usdt, t.status, t.pnl_usdt
                    ],
                );

                for (idx_off, l_bar) in lookback_bars.iter().enumerate() {
                    let offset = (idx_off as i32) - 100;
                    let _ = tx.execute(
                        "INSERT INTO trade_lookback_bars (trade_id, bar_offset, open_time_ms, open_time_utc, open, high, low, close, volume, close_time_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);",
                        rusqlite::params![
                            t.id as i64, offset, l_bar.open_time as i64, format_timestamp(l_bar.open_time),
                            l_bar.open, l_bar.high, l_bar.low, l_bar.close, l_bar.volume, l_bar.close_time as i64
                        ],
                    );
                    total_lookback_persisted += 1;
                }
            }

            let _ = tx.commit();
        }

        if let Some((last_t, _)) = detected_trades.last() {
            last_summary = format!(
                "Trade #{:<4} | {} | Entry: {:.5} | SL: {:.5} | TP: {:.5} | Result: {} | PnL: {:+.2} USDT",
                last_t.id, last_t.entry_time_utc, last_t.entry_price, last_t.stop_loss, last_t.take_profit, last_t.status, last_t.pnl_usdt
            );
        }
    }

    let db_size_mb = std::fs::metadata(db_path)
        .map(|m| m.len() as f64 / (1024.0 * 1024.0))
        .unwrap_or(0.0);

    CollectorStatus {
        symbol: symbol.to_string(),
        interval: interval.to_string(),
        total_bars_fetched: bars.len(),
        total_trades_detected: total_trades,
        total_lookback_bars_persisted: total_lookback_persisted,
        win_rate_pct,
        net_pnl_usdt,
        db_file_path: db_path.to_string(),
        db_size_mb,
        last_trade_summary: last_summary,
        last_updated_ms: now_ms,
    }
}

async fn fetch_and_run_tacusdt_1h(data_arc: Arc<Mutex<HashMap<String, Value>>>, db_path: String) {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let url_1h = "https://fapi.binance.com/fapi/v1/klines?symbol=TACUSDT&interval=1h&limit=1500";
    let resp = client.get(url_1h).send().await;

    if let Ok(r) = resp {
        if r.status().is_success() {
            if let Ok(raw) = r.json::<Vec<Vec<serde_json::Value>>>().await {
                let mut bars = Vec::with_capacity(raw.len());
                for row in raw {
                    if row.len() >= 6 {
                        bars.push(Bar {
                            open_time: parse_u64(&row[0]),
                            open: parse_f64(&row[1]),
                            high: parse_f64(&row[2]),
                            low: parse_f64(&row[3]),
                            close: parse_f64(&row[4]),
                            volume: parse_f64(&row[5]),
                            close_time: if row.len() > 6 { parse_u64(&row[6]) } else { 0 },
                        });
                    }
                }

                if bars.len() > 100 {
                    let status = process_and_persist_tacusdt_1h("TACUSDT", "1h", &bars, &db_path);
                    if let Ok(val) = serde_json::to_value(&status) {
                        let mut guard = data_arc.lock().unwrap();
                        guard.insert("tacusdt_1h_status".to_string(), val);
                    }
                }
            }
        }
    }
}

struct PluginState {
    runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<HashMap<String, Value>>>,
    outbox: Arc<Mutex<Vec<Value>>>,
    _stream_configs: Arc<Mutex<HashMap<String, (String, String)>>>,
    db_path: String,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Tokio runtime olusturulamadi");

    let data = Arc::new(Mutex::new(HashMap::new()));
    let is_running = Arc::new(AtomicBool::new(false));
    let db_path = "/home/smhvz/Desktop/cycle-orc/data/tacusdt_1h_collector.db".to_string();

    let data_sync = data.clone();
    let db_path_sync = db_path.clone();
    runtime.spawn(async move {
        fetch_and_run_tacusdt_1h(data_sync, db_path_sync).await;
    });

    let state = Box::new(PluginState {
        runtime,
        is_running,
        data,
        outbox: Arc::new(Mutex::new(Vec::new())),
        _stream_configs: Arc::new(Mutex::new(HashMap::new())),
        db_path,
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
            state.is_running.store(true, Ordering::Relaxed);

            let data_arc = state.data.clone();
            let db_path_clone = state.db_path.clone();
            let is_running_arc = state.is_running.clone();

            state.runtime.spawn(async move {
                while is_running_arc.load(Ordering::Relaxed) {
                    fetch_and_run_tacusdt_1h(data_arc.clone(), db_path_clone.clone()).await;
                    tokio::time::sleep(tokio::time::Duration::from_secs(15)).await;
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

            if guard.is_empty() {
                drop(guard);
                let data_arc = state.data.clone();
                let db_path_clone = state.db_path.clone();
                state.runtime.spawn(async move {
                    fetch_and_run_tacusdt_1h(data_arc, db_path_clone).await;
                });
            }

            let guard = state.data.lock().unwrap();
            let mut report_str = String::new();
            report_str.push_str("========================================================================================--\n");
            report_str.push_str("🔥 TACUSDT 1h TÜM ZAMANLAR İŞLEM VE 100-BAR KAYIT EKLENTİSİ (PLUGIN_TACUSDT_1H)\n");
            report_str.push_str("========================================================================================--\n");

            for (stream_id, val) in guard.iter() {
                if let Ok(st) = serde_json::from_value::<CollectorStatus>(val.clone()) {
                    report_str.push_str(&format!(
                        "[{}] Symbol: {} | Interval: {} | Toplam Çekilen 1h Bar: {}\n",
                        stream_id, st.symbol, st.interval, st.total_bars_fetched
                    ));
                    report_str.push_str(&format!(
                        "📊 İşlem Sayısı: {} adet | Ham Win Rate: %{:.2} | Net PnL: {:+.2} USDT\n",
                        st.total_trades_detected, st.win_rate_pct, st.net_pnl_usdt
                    ));
                    report_str.push_str(&format!(
                        "💾 SQLite Veritabanı Yolu   : {} ({:.2} MB)\n",
                        st.db_file_path, st.db_size_mb
                    ));
                    report_str.push_str("------------------------------------------------------------------------------------------\n");
                    report_str.push_str(&format!(
                        "📌 Son Kaydedilen İşlem     : {}\n",
                        st.last_trade_summary
                    ));
                }
            }
            report_str.push_str("========================================================================================--\n");

            let mut response_map = serde_json::Map::new();
            response_map.insert("formatted_report".to_string(), Value::String(report_str));
            response_map.insert("metrics".to_string(), serde_json::to_value(&*guard).unwrap_or_default());

            if let Ok(bytes) = serde_json::to_vec(&Value::Object(response_map)) {
                let len = bytes.len().min(out_max_len);
                if len > 0 {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                }
                len
            } else {
                0
            }
        }
        6 => { 0 }
        7 => {
            let mut q = state.outbox.lock().unwrap();
            if !q.is_empty() {
                let json_array = Value::Array(q.clone());
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
