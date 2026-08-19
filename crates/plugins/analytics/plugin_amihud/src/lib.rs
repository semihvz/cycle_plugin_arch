use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct TradeEntry {
    pub trade_id: i64,
    pub price: f64,
    pub quantity: f64,
    pub usdt_value: f64,
    pub timestamp_ms: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AmihudMetrics {
    pub symbol: String,
    pub start_price: f64,
    pub latest_price: f64,
    pub price_change_pct: f64,
    pub abs_return_pct: f64,
    pub total_volume_usdt: f64,
    pub raw_amihud: f64,
    pub amihud_per_1m_usdt: f64,
    pub trade_count: usize,
    pub window_ms: u64,
    pub last_updated_ms: u64,
    pub liquidity_level: String,
}

pub struct AmihudEngine {
    pub window_ms: AtomicU64,
    pub trade_history: Mutex<HashMap<String, VecDeque<TradeEntry>>>,
    pub metrics: Mutex<HashMap<String, AmihudMetrics>>,
    pub last_seen_trade_id: Mutex<HashMap<String, i64>>,
}

impl AmihudEngine {
    pub fn new() -> Self {
        Self {
            window_ms: AtomicU64::new(60000), // Default 60 seconds (60,000 ms)
            trade_history: Mutex::new(HashMap::new()),
            metrics: Mutex::new(HashMap::new()),
            last_seen_trade_id: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_window_ms(&self, ms: u64) {
        if ms >= 1000 {
            self.window_ms.store(ms, Ordering::Relaxed);
        }
    }

    pub fn process_trade_payload(&self, json_data: &Value, now_ms: u64) -> String {
        let mut history_guard = self.trade_history.lock().unwrap();
        let mut metrics_guard = self.metrics.lock().unwrap();
        let mut last_seen_guard = self.last_seen_trade_id.lock().unwrap();

        let window_ms = self.window_ms.load(Ordering::Relaxed).max(1000);
        let window_secs = window_ms as f64 / 1000.0;

        // Ingest trades from payload
        if let Some(obj) = json_data.as_object() {
            for (symbol, item) in obj.iter() {
                let trade_id = parse_i64(&item["trade_id"]);

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
                let event_time = parse_u64(&item["event_time"])
                    .or_else(|| parse_u64(&item["local_recv_time_ms"]))
                    .unwrap_or(now_ms);

                let usdt_value = price * quantity;

                if !is_duplicate && (price > 0.0 || quantity > 0.0 || trade_id > 0) {
                    if trade_id > 0 {
                        last_seen_guard.insert(symbol.clone(), trade_id);
                    }
                    let deque = history_guard.entry(symbol.clone()).or_insert_with(VecDeque::new);
                    deque.push_back(TradeEntry {
                        trade_id,
                        price,
                        quantity,
                        usdt_value,
                        timestamp_ms: event_time,
                    });
                }
            }
        }

        // Compute rolling Amihud Illiquidity Ratio
        let mut sorted_symbols: Vec<String> = history_guard.keys().cloned().collect();
        sorted_symbols.sort();

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str(&format!(
            "📊 AMIHUD İLLİKİDİTE ANALİZİ (PENCERE: {} ms / {:.1} sn)\n",
            window_ms, window_secs
        ));
        report.push_str("============================================================\n");

        if sorted_symbols.is_empty() {
            report.push_str("Henüz işlem verisi (aggtrades/trades) alınmadı.\n");
        } else {
            for symbol in &sorted_symbols {
                if let Some(deque) = history_guard.get_mut(symbol) {
                    let latest_ts = deque.back().map(|t| t.timestamp_ms).unwrap_or(now_ms);
                    let symbol_window_start_ms = latest_ts.saturating_sub(window_ms);

                    // Prune entries older than window
                    while let Some(front) = deque.front() {
                        if front.timestamp_ms < symbol_window_start_ms {
                            deque.pop_front();
                        } else {
                            break;
                        }
                    }

                    let trade_count = deque.len();
                    if trade_count == 0 {
                        continue;
                    }

                    let start_price = deque.front().map(|t| t.price).unwrap_or(0.0);
                    let latest_price = deque.back().map(|t| t.price).unwrap_or(0.0);
                    let last_up_ms = deque.back().map(|t| t.timestamp_ms).unwrap_or(now_ms);

                    let total_volume_usdt: f64 = deque.iter().map(|t| t.usdt_value).sum();

                    let (price_change_pct, abs_return_pct, raw_amihud, amihud_per_1m) =
                        if start_price > 0.0 && total_volume_usdt > 0.0 {
                            let rel_return = (latest_price - start_price) / start_price;
                            let abs_ret = rel_return.abs();
                            let raw = abs_ret / total_volume_usdt;
                            let per_1m = (abs_ret * 100.0) / (total_volume_usdt / 1_000_000.0);
                            (rel_return * 100.0, abs_ret * 100.0, raw, per_1m)
                        } else {
                            (0.0, 0.0, 0.0, 0.0)
                        };

                    let liquidity_level = classify_liquidity(amihud_per_1m, total_volume_usdt);

                    let metrics_item = AmihudMetrics {
                        symbol: symbol.clone(),
                        start_price,
                        latest_price,
                        price_change_pct,
                        abs_return_pct,
                        total_volume_usdt,
                        raw_amihud,
                        amihud_per_1m_usdt: amihud_per_1m,
                        trade_count,
                        window_ms,
                        last_updated_ms: last_up_ms,
                        liquidity_level: liquidity_level.clone(),
                    };

                    metrics_guard.insert(symbol.clone(), metrics_item);

                    report.push_str(&format!(
                        "[{}]  İlk Fiyat: {:.4} -> Son Fiyat: {:.4} (Değişim: {:+.2}%)\n\
                         ├─ Toplam Hacim: ${:.2} (İşlem Sayısı: {})\n\
                         ├─ Amihud İllikidite Oranı (1M $ başına): {:.6}%\n\
                         ├─ Ham Amihud Skoru (Raw): {:.12}\n\
                         └─ Likidite Seviyesi: {}\n\n",
                        symbol,
                        start_price,
                        latest_price,
                        price_change_pct,
                        total_volume_usdt,
                        trade_count,
                        amihud_per_1m,
                        raw_amihud,
                        liquidity_level
                    ));
                }
            }
        }
        report
    }

    pub fn get_metrics_json(&self) -> String {
        let metrics_guard = self.metrics.lock().unwrap();
        serde_json::to_string_pretty(&*metrics_guard).unwrap_or_else(|_| "{}".to_string())
    }
}

fn classify_liquidity(amihud_per_1m: f64, volume: f64) -> String {
    if volume <= 0.0 {
        "BILINMIYOR (Hacim Yok)".to_string()
    } else if amihud_per_1m <= 0.05 {
        "AŞIRI YÜKSEK (Very High)".to_string()
    } else if amihud_per_1m <= 0.20 {
        "YÜKSEK (High)".to_string()
    } else if amihud_per_1m <= 1.00 {
        "ORTA (Medium)".to_string()
    } else if amihud_per_1m <= 5.00 {
        "DÜŞÜK (Low)".to_string()
    } else {
        "AŞIRI DÜŞÜK / İLLİKİT (Very Low)".to_string()
    }
}

fn parse_f64(val: &Value) -> f64 {
    match val {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

fn parse_i64(val: &Value) -> i64 {
    match val {
        Value::Number(n) => n.as_i64().unwrap_or(0),
        Value::String(s) => s.parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

fn parse_u64(val: &Value) -> Option<u64> {
    match val {
        Value::Number(n) => n.as_u64(),
        Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    }
}

// C ABI Struct
struct PluginState {
    is_running: Arc<AtomicBool>,
    engine: Arc<AmihudEngine>,
    data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        engine: Arc::new(AmihudEngine::new()),
        data: Arc::new(Mutex::new(Vec::new())),
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
                    if let Some(ms) = params.get("window_ms").and_then(|v| v.as_u64()) {
                        state.engine.set_window_ms(ms);
                    } else if let Some(secs) = params.get("window_secs").and_then(|v| v.as_u64()) {
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
        4 | 5 => { // DataMonitor (4) / RawData (5)
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
                let stream_id = std::str::from_utf8(header)
                    .unwrap_or("")
                    .trim_matches(char::from(0))
                    .trim()
                    .to_string();

                if stream_id.contains("aggtrades") || stream_id.contains("trades") {
                    if let Ok(json_data) = serde_json::from_slice::<Value>(&slice[32..]) {
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let report = state.engine.process_trade_payload(&json_data, now_ms);

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
    fn test_amihud_calculation_precision() {
        let engine = AmihudEngine::new();
        engine.set_window_ms(60000);

        let t0 = 1000000u64;
        let t1 = 1005000u64;

        // Trade 1: price 100.0, qty 10.0 => usdt_value = 1000.0
        let p0 = serde_json::json!({
            "BTCUSDT": {
                "trade_id": 1,
                "price": "100.0",
                "quantity": "10.0",
                "event_time": t0
            }
        });

        // Trade 2: price 102.0, qty 10.0 => usdt_value = 1020.0
        // Total volume = 2020.0 USDT
        // Return = (102.0 - 100.0) / 100.0 = 0.02 (2.0%)
        // Raw Amihud = 0.02 / 2020.0 = ~9.90099e-6
        // Amihud per 1M USDT = (0.02 * 100) / (2020.0 / 1_000_000) = 2.0 / 0.00202 = 990.099%
        let p1 = serde_json::json!({
            "BTCUSDT": {
                "trade_id": 2,
                "price": "102.0",
                "quantity": "10.0",
                "event_time": t1
            }
        });

        engine.process_trade_payload(&p0, t0);
        let report = engine.process_trade_payload(&p1, t1);

        assert!(report.contains("AMIHUD İLLİKİDİTE ANALİZİ"));
        assert!(report.contains("BTCUSDT"));

        let metrics_guard = engine.metrics.lock().unwrap();
        let btc = metrics_guard.get("BTCUSDT").unwrap();

        assert_eq!(btc.trade_count, 2);
        assert_eq!(btc.start_price, 100.0);
        assert_eq!(btc.latest_price, 102.0);
        assert!((btc.price_change_pct - 2.0).abs() < 1e-5);
        assert_eq!(btc.total_volume_usdt, 2020.0);
        assert!((btc.raw_amihud - (0.02 / 2020.0)).abs() < 1e-8);
        assert!((btc.amihud_per_1m_usdt - 990.0990099).abs() < 1e-3);
    }
}
