use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OhlcvCandle {
    pub symbol: String,
    pub timestamp_sec: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub trades_count: usize,
    pub buy_volume: f64,
    pub sell_volume: f64,
}

impl OhlcvCandle {
    pub fn new(symbol: String, timestamp_sec: u64, price: f64, quantity: f64, buyer_is_maker: bool) -> Self {
        let usdt_val = price * quantity;
        let (buy_vol, sell_vol) = if buyer_is_maker {
            (0.0, quantity) // Buyer is maker => Taker sell
        } else {
            (quantity, 0.0) // Seller is maker => Taker buy
        };

        Self {
            symbol,
            timestamp_sec,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: quantity,
            quote_volume: usdt_val,
            trades_count: 1,
            buy_volume: buy_vol,
            sell_volume: sell_vol,
        }
    }

    pub fn update(&mut self, price: f64, quantity: f64, buyer_is_maker: bool) {
        if price > self.high {
            self.high = price;
        }
        if price < self.low {
            self.low = price;
        }
        self.close = price;
        self.volume += quantity;
        self.quote_volume += price * quantity;
        self.trades_count += 1;

        if buyer_is_maker {
            self.sell_volume += quantity;
        } else {
            self.buy_volume += quantity;
        }
    }
}

pub struct OhlcvEngine {
    pub history_limit: AtomicU64,
    pub active_candles: Mutex<HashMap<String, OhlcvCandle>>,
    pub completed_candles: Mutex<HashMap<String, VecDeque<OhlcvCandle>>>,
    pub last_seen_trade_id: Mutex<HashMap<String, i64>>,
}

impl OhlcvEngine {
    pub fn new() -> Self {
        Self {
            history_limit: AtomicU64::new(60), // Default keep last 60 1-second candles
            active_candles: Mutex::new(HashMap::new()),
            completed_candles: Mutex::new(HashMap::new()),
            last_seen_trade_id: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_history_limit(&self, limit: usize) {
        if limit > 0 {
            self.history_limit.store(limit as u64, Ordering::Relaxed);
        }
    }

    pub fn process_trade(&self, symbol: &str, trade_id: i64, price: f64, quantity: f64, buyer_is_maker: bool, timestamp_ms: u64) {
        if price <= 0.0 || quantity <= 0.0 {
            return;
        }

        let mut last_seen = self.last_seen_trade_id.lock().unwrap();
        if trade_id > 0 {
            if let Some(&last_id) = last_seen.get(symbol) {
                if last_id == trade_id {
                    return; // Skip duplicate trade
                }
            }
            last_seen.insert(symbol.to_string(), trade_id);
        }

        let timestamp_sec = timestamp_ms / 1000;
        let mut active_guard = self.active_candles.lock().unwrap();
        let mut completed_guard = self.completed_candles.lock().unwrap();
        let history_limit = self.history_limit.load(Ordering::Relaxed) as usize;

        if let Some(active) = active_guard.get_mut(symbol) {
            if active.timestamp_sec == timestamp_sec {
                active.update(price, quantity, buyer_is_maker);
            } else if active.timestamp_sec < timestamp_sec {
                // Move active candle to completed history
                let old_candle = active.clone();
                let deque = completed_guard.entry(symbol.to_string()).or_insert_with(VecDeque::new);
                deque.push_back(old_candle);
                while deque.len() > history_limit {
                    deque.pop_front();
                }

                // Start new active candle
                *active = OhlcvCandle::new(symbol.to_string(), timestamp_sec, price, quantity, buyer_is_maker);
            }
        } else {
            // First candle for symbol
            active_guard.insert(symbol.to_string(), OhlcvCandle::new(symbol.to_string(), timestamp_sec, price, quantity, buyer_is_maker));
        }
    }

    pub fn process_aggtrade_payload(&self, json_data: &Value, now_ms: u64) -> String {
        if let Some(obj) = json_data.as_object() {
            for (symbol, item) in obj.iter() {
                let trade_id = parse_i64(&item["trade_id"]);
                let price = parse_f64(&item["price"]);
                let quantity = parse_f64(&item["quantity"]);
                let buyer_is_maker = parse_bool(&item["buyer_is_maker"])
                    .or_else(|| parse_bool(&item["m"]))
                    .unwrap_or(false);

                let event_time = parse_u64(&item["event_time"]);
                let local_time = parse_u64(&item["local_recv_time_ms"]);
                let timestamp_ms = if event_time > 0 {
                    event_time
                } else if local_time > 0 {
                    local_time
                } else {
                    now_ms
                };

                self.process_trade(symbol, trade_id, price, quantity, buyer_is_maker, timestamp_ms);
            }
        }

        self.get_formatted_report()
    }

    pub fn get_formatted_report(&self) -> String {
        let active_guard = self.active_candles.lock().unwrap();
        let completed_guard = self.completed_candles.lock().unwrap();

        let mut sorted_symbols: Vec<String> = active_guard.keys().cloned().collect();
        sorted_symbols.sort();

        let mut report = String::new();
        report.push_str("================================================================================\n");
        report.push_str("📊 SANİYELİK OHLCV ÇUBUKLARI (1-Second Real-Time Candle Engine)\n");
        report.push_str("================================================================================\n");

        if sorted_symbols.is_empty() {
            report.push_str("Henüz işlem verisi alınmadı. RAM router akışı bekleniyor...\n");
        } else {
            for symbol in &sorted_symbols {
                if let Some(candle) = active_guard.get(symbol) {
                    let change_pct = if candle.open > 0.0 {
                        ((candle.close - candle.open) / candle.open) * 100.0
                    } else {
                        0.0
                    };

                    let (dir_icon, dir_text) = if candle.close > candle.open {
                        ("🟢", "BOĞA")
                    } else if candle.close < candle.open {
                        ("🔴", "AYI")
                    } else {
                        ("⚪", "NÖTR")
                    };

                    report.push_str(&format!(
                        "[{}] Zz: {} s | {} {} ({:+.8}%)\n",
                        symbol, candle.timestamp_sec, dir_icon, dir_text, change_pct
                    ));
                    report.push_str(&format!(
                        "  ├─► Açılış: {:.8} | Yüksek: {:.8} | Düşük: {:.8} | Kapanış: {:.8}\n",
                        candle.open, candle.high, candle.low, candle.close
                    ));
                    report.push_str(&format!(
                        "  ├─► Hacim : {:.8} (Quote: {:.8} USDT) | İşlem Adedi: {}\n",
                        candle.volume, candle.quote_volume, candle.trades_count
                    ));
                    report.push_str(&format!(
                        "  └─► Taker Dağılımı: Alım {:.8} / Satım {:.8}\n",
                        candle.buy_volume, candle.sell_volume
                    ));

                    if let Some(history) = completed_guard.get(symbol) {
                        let len = history.len();
                        report.push_str(&format!("  └─► Geçmiş Tamamlanan Mumlar (Son {}/{}): ", len, self.history_limit.load(Ordering::Relaxed)));
                        let tail_slice: Vec<_> = history.iter().rev().take(5).collect();
                        for (idx, h_candle) in tail_slice.iter().rev().enumerate() {
                            let icon = if h_candle.close >= h_candle.open { "🟩" } else { "🟥" };
                            report.push_str(&format!("{} [{:.8}]", icon, h_candle.close));
                            if idx + 1 < tail_slice.len() {
                                report.push_str(" -> ");
                            }
                        }
                        report.push_str("\n");
                    }
                    report.push_str("--------------------------------------------------------------------------------\n");
                }
            }
        }
        report.push_str("================================================================================\n");

        report
    }

    pub fn get_raw_json(&self) -> String {
        let active_guard = self.active_candles.lock().unwrap();
        let completed_guard = self.completed_candles.lock().unwrap();

        let output = serde_json::json!({
            "active_candles": *active_guard,
            "completed_candles": *completed_guard
        });

        serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string())
    }
}

fn parse_f64(val: &Value) -> f64 {
    if let Some(f) = val.as_f64() {
        f
    } else if let Some(s) = val.as_str() {
        s.parse::<f64>().unwrap_or(0.0)
    } else if let Some(i) = val.as_i64() {
        i as f64
    } else {
        0.0
    }
}

fn parse_i64(val: &Value) -> i64 {
    if let Some(i) = val.as_i64() {
        i
    } else if let Some(s) = val.as_str() {
        s.parse::<i64>().unwrap_or(0)
    } else {
        0
    }
}

fn parse_u64(val: &Value) -> u64 {
    if let Some(u) = val.as_u64() {
        u
    } else if let Some(i) = val.as_i64() {
        i.max(0) as u64
    } else if let Some(s) = val.as_str() {
        s.parse::<u64>().unwrap_or(0)
    } else {
        0
    }
}

fn parse_bool(val: &Value) -> Option<bool> {
    if let Some(b) = val.as_bool() {
        Some(b)
    } else if let Some(s) = val.as_str() {
        s.parse::<bool>().ok()
    } else {
        None
    }
}

struct PluginState {
    is_running: Arc<AtomicBool>,
    engine: Arc<OhlcvEngine>,
    data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let engine = Arc::new(OhlcvEngine::new());
    let initial_report = engine.get_formatted_report();

    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        engine,
        data: Arc::new(Mutex::new(initial_report.into_bytes())),
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
    if plugin_state.is_null() {
        return 0;
    }
    let state = &*(plugin_state as *const PluginState);

    match endpoint_id {
        0 => { // Start
            state.is_running.store(true, Ordering::Relaxed);

            if payload_len > 0 && !payload.is_null() {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(config) = serde_json::from_slice::<Value>(slice) {
                    let params = config.get("plugin_params").unwrap_or(&config);
                    if let Some(limit) = params.get("history_limit").and_then(|v| v.as_u64()) {
                        state.engine.set_history_limit(limit as usize);
                    }
                }
            }
            0
        }
        1 => { // Stop
            state.is_running.store(false, Ordering::Relaxed);
            0
        }
        2 => { // IsWorking
            let running = state.is_running.load(Ordering::Relaxed);
            if out_max_len >= 1 && !out_buf.is_null() {
                *out_buf = if running { 1 } else { 0 };
                1
            } else {
                if running { 1 } else { 0 }
            }
        }
        3 => { // DataValid
            1
        }
        4 => { // DataMonitor (TUI Monitoring view)
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            if len > 0 && !out_buf.is_null() {
                std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
            }
            len
        }
        5 => { // RawData (JSON candles)
            let raw_json = state.engine.get_raw_json().into_bytes();
            let len = raw_json.len().min(out_max_len);
            if len > 0 && !out_buf.is_null() {
                std::ptr::copy_nonoverlapping(raw_json.as_ptr(), out_buf, len);
            }
            len
        }
        6 => { // Inbox (Data routed from FlowEngine)
            if !state.is_running.load(Ordering::Relaxed) {
                return 0;
            }

            if payload_len > 32 && !payload.is_null() {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                let header = &slice[0..32];
                let stream_id = std::str::from_utf8(header)
                    .unwrap_or("")
                    .trim_matches(char::from(0))
                    .trim()
                    .to_string();

                if stream_id == "stream_aggtrades" || stream_id.contains("aggtrade") || stream_id.contains("trade") {
                    if let Ok(json_data) = serde_json::from_slice::<Value>(&slice[32..]) {
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let report = state.engine.process_aggtrade_payload(&json_data, now_ms);

                        let mut data_guard = state.data.lock().unwrap();
                        *data_guard = report.into_bytes();
                    }
                }
            }
            0
        }
        _ => 0,
    }
}
