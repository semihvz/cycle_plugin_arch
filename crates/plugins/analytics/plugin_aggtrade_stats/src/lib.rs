use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct TradeEntry {
    pub trade_id: i64,
    pub price: f64,
    pub quantity: f64,
    pub usdt_value: f64,
    pub buyer_is_maker: bool,
    pub timestamp_ms: u64,
}

#[derive(Debug, Default, Clone)]
pub struct SymbolMetrics {
    pub usdt_volume_per_sec: f64,
    pub trades_per_sec: usize,
    pub last_trade_id: i64,
    pub last_price: f64,
    pub maker_volume_pct: f64,
    pub taker_volume_pct: f64,
    pub maker_trades_pct: f64,
    pub taker_trades_pct: f64,
}

pub struct AggTradeStatsEngine {
    pub window_ms: AtomicU64,
    pub trade_history: Mutex<HashMap<String, VecDeque<TradeEntry>>>,
    pub symbol_metrics: Mutex<HashMap<String, SymbolMetrics>>,
    pub last_seen_trade_id: Mutex<HashMap<String, i64>>,
}

impl AggTradeStatsEngine {
    pub fn new() -> Self {
        Self {
            window_ms: AtomicU64::new(1000), // Default 1000ms (1 second)
            trade_history: Mutex::new(HashMap::new()),
            symbol_metrics: Mutex::new(HashMap::new()),
            last_seen_trade_id: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_window_ms(&self, ms: u64) {
        if ms >= 100 {
            self.window_ms.store(ms, Ordering::Relaxed);
        }
    }

    pub fn process_aggtrade_payload(&self, json_data: &Value, now_ms: u64) -> String {
        let mut history_guard = self.trade_history.lock().unwrap();
        let mut metrics_guard = self.symbol_metrics.lock().unwrap();
        let mut last_seen_guard = self.last_seen_trade_id.lock().unwrap();

        let window_ms = self.window_ms.load(Ordering::Relaxed).max(100);
        let window_secs = window_ms as f64 / 1000.0;

        if let Some(obj) = json_data.as_object() {
            for (symbol, item) in obj.iter() {
                let trade_id = parse_i64(&item["trade_id"]);
                
                // Deduplicate if identical trade_id already recorded for this symbol
                let is_duplicate = if trade_id > 0 {
                    if let Some(&last_id) = last_seen_guard.get(symbol) {
                        last_id == trade_id
                    } else {
                        false
                    }
                } else {
                    false
                };

                let price = parse_f64(&item["price"]);
                let quantity = parse_f64(&item["quantity"]);
                let usdt_val = price * quantity;
                let buyer_is_maker = parse_bool(&item["buyer_is_maker"])
                    .or_else(|| parse_bool(&item["m"]))
                    .unwrap_or(false);
                let timestamp_ms = now_ms;

                if !is_duplicate && (price > 0.0 || quantity > 0.0 || trade_id > 0) {
                    if trade_id > 0 {
                        last_seen_guard.insert(symbol.clone(), trade_id);
                    }
                    let deque = history_guard.entry(symbol.clone()).or_insert_with(VecDeque::new);
                    deque.push_back(TradeEntry {
                        trade_id,
                        price,
                        quantity,
                        usdt_value: usdt_val,
                        buyer_is_maker,
                        timestamp_ms,
                    });
                }
            }
        }

        // Calculate rolling window metrics per symbol
        let window_start_ms = now_ms.saturating_sub(window_ms);
        let mut sorted_symbols: Vec<String> = history_guard.keys().cloned().collect();
        sorted_symbols.sort();

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str(&format!("📊 AGGTRADE METRİKLERİ (ÖLÇÜM PENCERESİ: {} ms / {:.1} sn)\n", window_ms, window_secs));
        report.push_str("============================================================\n");

        if sorted_symbols.is_empty() {
            report.push_str("Henüz işlem verisi alınmadı.\n");
        } else {
            for symbol in &sorted_symbols {
                if let Some(deque) = history_guard.get_mut(symbol) {
                    // Prune entries older than window_ms
                    while let Some(front) = deque.front() {
                        if front.timestamp_ms < window_start_ms {
                            deque.pop_front();
                        } else {
                            break;
                        }
                    }

                    let total_window_volume: f64 = deque.iter().map(|t| t.usdt_value).sum();
                    let total_window_trades = deque.len();

                    let usdt_volume_per_sec = total_window_volume / window_secs;
                    let trades_per_sec = (total_window_trades as f64 / window_secs).round() as usize;

                    let last_trade = deque.back();
                    let last_trade_id = last_trade.map(|t| t.trade_id).unwrap_or(0);
                    let last_price = last_trade.map(|t| t.price).unwrap_or(0.0);

                    let maker_volume: f64 = deque.iter().filter(|t| t.buyer_is_maker).map(|t| t.usdt_value).sum();
                    let taker_volume: f64 = deque.iter().filter(|t| !t.buyer_is_maker).map(|t| t.usdt_value).sum();

                    let maker_trade_count = deque.iter().filter(|t| t.buyer_is_maker).count();
                    let taker_trade_count = deque.iter().filter(|t| !t.buyer_is_maker).count();

                    let (maker_volume_pct, taker_volume_pct) = if total_window_volume > 0.0 {
                        ((maker_volume / total_window_volume) * 100.0, (taker_volume / total_window_volume) * 100.0)
                    } else {
                        (0.0, 0.0)
                    };

                    let (maker_trades_pct, taker_trades_pct) = if total_window_trades > 0 {
                        ((maker_trade_count as f64 / total_window_trades as f64) * 100.0, (taker_trade_count as f64 / total_window_trades as f64) * 100.0)
                    } else {
                        (0.0, 0.0)
                    };

                    metrics_guard.insert(
                        symbol.clone(),
                        SymbolMetrics {
                            usdt_volume_per_sec,
                            trades_per_sec,
                            last_trade_id,
                            last_price,
                            maker_volume_pct,
                            taker_volume_pct,
                            maker_trades_pct,
                            taker_trades_pct,
                        },
                    );

                    report.push_str(&format!(
                        "[{}]  Hacim: {:.8} USDT/sn | İşlem: {}/sn (Son: {:.8}, ID: {})\n\
                         └─► Maker/Taker Hacim: %{:.2} Maker / %{:.2} Taker\n\
                         └─► Maker/Taker Adet : %{:.2} Maker / %{:.2} Taker\n",
                        symbol, usdt_volume_per_sec, trades_per_sec, last_price, last_trade_id,
                        maker_volume_pct, taker_volume_pct, maker_trades_pct, taker_trades_pct
                    ));
                }
            }
        }
        report.push_str("============================================================\n");

        report
    }

    pub fn get_formatted_report(&self) -> String {
        let metrics_guard = self.symbol_metrics.lock().unwrap();
        let window_ms = self.window_ms.load(Ordering::Relaxed);
        let window_secs = window_ms as f64 / 1000.0;
        let mut sorted_symbols: Vec<String> = metrics_guard.keys().cloned().collect();
        sorted_symbols.sort();

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str(&format!("📊 AGGTRADE METRİKLERİ (ÖLÇÜM PENCERESİ: {} ms / {:.1} sn)\n", window_ms, window_secs));
        report.push_str("============================================================\n");

        if sorted_symbols.is_empty() {
            report.push_str("Henüz işlem verisi alınmadı.\n");
        } else {
            for symbol in &sorted_symbols {
                if let Some(m) = metrics_guard.get(symbol) {
                    report.push_str(&format!(
                        "[{}]  Hacim: {:.8} USDT/sn | İşlem: {}/sn (Son: {:.8}, ID: {})\n\
                         └─► Maker/Taker Hacim: %{:.2} Maker / %{:.2} Taker\n\
                         └─► Maker/Taker Adet : %{:.2} Maker / %{:.2} Taker\n",
                        symbol, m.usdt_volume_per_sec, m.trades_per_sec, m.last_price, m.last_trade_id,
                        m.maker_volume_pct, m.taker_volume_pct, m.maker_trades_pct, m.taker_trades_pct
                    ));
                }
            }
        }
        report.push_str("============================================================\n");
        report
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
    engine: Arc<AggTradeStatsEngine>,
    data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let engine = Arc::new(AggTradeStatsEngine::new());
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
            
            // Parse plugin configuration / parameters if passed in payload
            if payload_len > 0 && !payload.is_null() {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(config) = serde_json::from_slice::<Value>(slice) {
                    let params = config.get("plugin_params").unwrap_or(&config);
                    
                    if let Some(ms) = params.get("window_ms").or_else(|| params.get("interval_ms")).and_then(|v| v.as_u64()) {
                        state.engine.set_window_ms(ms);
                    } else if let Some(secs) = params.get("window_secs").or_else(|| params.get("interval_secs")).and_then(|v| v.as_u64()) {
                        state.engine.set_window_ms(secs * 1000);
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
        4 => { // DataMonitor (TUI Monitoring)
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            if len > 0 && !out_buf.is_null() {
                std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
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
                let stream_id = std::str::from_utf8(header).unwrap_or("").trim_matches(char::from(0)).trim().to_string();

                if stream_id == "stream_aggtrades" || stream_id.contains("aggtrade") || stream_id.contains("trade") {
                    if let Ok(json_data) = serde_json::from_slice::<Value>(&slice[32..]) {
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let report = state.engine.process_aggtrade_payload(&json_data, now_ms);
                        
                        // Update DataMonitor memory buffer safely (without stdout println!)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aggtrade_configurable_window_ms() {
        let engine = AggTradeStatsEngine::new();
        engine.set_window_ms(5000); // 5-second window
        assert_eq!(engine.window_ms.load(Ordering::Relaxed), 5000);

        let now_ms = 1700000000000u64;
        let payload = serde_json::json!({
            "BTCUSDT": {
                "trade_id": 1001,
                "price": "50000.0",
                "quantity": "1.0",
                "buyer_is_maker": true,
                "local_recv_time_ms": now_ms
            }
        });

        let report = engine.process_aggtrade_payload(&payload, now_ms);
        assert!(report.contains("ÖLÇÜM PENCERESİ: 5000 ms / 5.0 sn"));
        // 50000 USDT total in 5s window => 10000 USDT/sec
        assert!(report.contains("10000.00000000 USDT/sn"));
    }
}
