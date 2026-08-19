use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PricePoint {
    pub timestamp_ms: u64,
    pub best_bid: f64,
    pub best_ask: f64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct DerivativeValues {
    pub d1_velocity: f64,     // 1st derivative: rate of price change (USDT/s)
    pub d2_acceleration: f64, // 2nd derivative: rate of velocity change (USDT/s²)
    pub d3_jerk: f64,         // 3rd derivative: rate of acceleration change (USDT/s³)
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SymbolDerivativesMetrics {
    pub symbol: String,
    pub latest_best_bid: f64,
    pub latest_best_ask: f64,
    pub bid_instantaneous: DerivativeValues,
    pub ask_instantaneous: DerivativeValues,
    pub bid_30s_avg: DerivativeValues,
    pub ask_30s_avg: DerivativeValues,
    pub sample_count: usize,
    pub last_updated_ms: u64,
}

pub struct BookTickerDerivativesEngine {
    pub window_ms: AtomicU64,
    pub history: Mutex<HashMap<String, VecDeque<PricePoint>>>,
    pub metrics: Mutex<HashMap<String, SymbolDerivativesMetrics>>,
}

impl BookTickerDerivativesEngine {
    pub fn new() -> Self {
        Self {
            window_ms: AtomicU64::new(30000), // Default 30,000 ms (30 seconds)
            history: Mutex::new(HashMap::new()),
            metrics: Mutex::new(HashMap::new()),
        }
    }

    pub fn set_window_ms(&self, ms: u64) {
        if ms >= 1000 {
            self.window_ms.store(ms, Ordering::Relaxed);
        }
    }

    pub fn process_bestprice_payload(&self, json_data: &Value, now_ms: u64) -> String {
        let mut history_guard = self.history.lock().unwrap();
        let mut metrics_guard = self.metrics.lock().unwrap();
        let window_ms = self.window_ms.load(Ordering::Relaxed).max(1000);
        let window_secs = window_ms as f64 / 1000.0;

        // Ingest new points
        if let Some(obj) = json_data.as_object() {
            for (symbol, item) in obj.iter() {
                let best_bid = parse_f64(&item["best_bid"]);
                let best_ask = parse_f64(&item["best_ask"]);
                let event_time = parse_u64(&item["event_time"])
                    .or_else(|| parse_u64(&item["local_recv_time_ms"]))
                    .unwrap_or(now_ms);

                if best_bid > 0.0 || best_ask > 0.0 {
                    let deque = history_guard.entry(symbol.clone()).or_insert_with(VecDeque::new);
                    // Prevent pushing exact duplicate timestamps
                    if deque.back().map_or(true, |last| last.timestamp_ms < event_time) {
                        deque.push_back(PricePoint {
                            timestamp_ms: event_time,
                            best_bid,
                            best_ask,
                        });
                    }
                }
            }
        }

        // Compute rolling 30s window derivatives
        let window_start_ms = now_ms.saturating_sub(window_ms);
        let mut sorted_symbols: Vec<String> = history_guard.keys().cloned().collect();
        sorted_symbols.sort();

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str(&format!(
            "📈 BOOKTICKER TÜREV ANALİZİ (PENCERE: {} ms / {:.1} sn)\n",
            window_ms, window_secs
        ));
        report.push_str("============================================================\n");

        if sorted_symbols.is_empty() {
            report.push_str("Henüz en iyi alış/satış fiyat verisi alınmadı.\n");
        } else {
            for symbol in &sorted_symbols {
                if let Some(deque) = history_guard.get_mut(symbol) {
                    // Prune points older than 30 seconds
                    while let Some(front) = deque.front() {
                        if front.timestamp_ms < window_start_ms {
                            deque.pop_front();
                        } else {
                            break;
                        }
                    }

                    let sample_count = deque.len();
                    let latest_bid = deque.back().map(|p| p.best_bid).unwrap_or(0.0);
                    let latest_ask = deque.back().map(|p| p.best_ask).unwrap_or(0.0);
                    let last_up_ms = deque.back().map(|p| p.timestamp_ms).unwrap_or(now_ms);

                    // Compute derivatives for Bid and Ask
                    let (bid_inst, bid_avg) = compute_derivatives(deque, true);
                    let (ask_inst, ask_avg) = compute_derivatives(deque, false);

                    metrics_guard.insert(
                        symbol.clone(),
                        SymbolDerivativesMetrics {
                            symbol: symbol.clone(),
                            latest_best_bid: latest_bid,
                            latest_best_ask: latest_ask,
                            bid_instantaneous: bid_inst.clone(),
                            ask_instantaneous: ask_inst.clone(),
                            bid_30s_avg: bid_avg.clone(),
                            ask_30s_avg: ask_avg.clone(),
                            sample_count,
                            last_updated_ms: last_up_ms,
                        },
                    );

                    report.push_str(&format!(
                        "[{}]  Bid: {:.4} | Ask: {:.4} (Örnek: {})\n\
                         ├─► ALİŞ (BID) TÜREVLERİ (Anlık / 30s Ort):\n\
                         │   ├─ 1. Türev (Hız/Vel) : {:+10.4} USDT/s | Ort: {:+10.4} USDT/s\n\
                         │   ├─ 2. Türev (İvme/Acc): {:+10.4} USDT/s²| Ort: {:+10.4} USDT/s²\n\
                         │   └─ 3. Türev (Sars/Jrk): {:+10.4} USDT/s³| Ort: {:+10.4} USDT/s³\n\
                         └─► SATIŞ (ASK) TÜREVLERİ (Anlık / 30s Ort):\n\
                             ├─ 1. Türev (Hız/Vel) : {:+10.4} USDT/s | Ort: {:+10.4} USDT/s\n\
                             ├─ 2. Türev (İvme/Acc): {:+10.4} USDT/s²| Ort: {:+10.4} USDT/s²\n\
                             └─ 3. Türev (Sars/Jrk): {:+10.4} USDT/s³| Ort: {:+10.4} USDT/s³\n\n",
                        symbol,
                        latest_bid,
                        latest_ask,
                        sample_count,
                        bid_inst.d1_velocity,
                        bid_avg.d1_velocity,
                        bid_inst.d2_acceleration,
                        bid_avg.d2_acceleration,
                        bid_inst.d3_jerk,
                        bid_avg.d3_jerk,
                        ask_inst.d1_velocity,
                        ask_avg.d1_velocity,
                        ask_inst.d2_acceleration,
                        ask_avg.d2_acceleration,
                        ask_inst.d3_jerk,
                        ask_avg.d3_jerk,
                    ));
                }
            }
        }
        report.push_str("============================================================\n");
        report
    }

    pub fn get_formatted_report(&self) -> String {
        let metrics_guard = self.metrics.lock().unwrap();
        let window_ms = self.window_ms.load(Ordering::Relaxed);
        let window_secs = window_ms as f64 / 1000.0;
        let mut sorted_symbols: Vec<String> = metrics_guard.keys().cloned().collect();
        sorted_symbols.sort();

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str(&format!(
            "📈 BOOKTICKER TÜREV ANALİZİ (PENCERE: {} ms / {:.1} sn)\n",
            window_ms, window_secs
        ));
        report.push_str("============================================================\n");

        if sorted_symbols.is_empty() {
            report.push_str("Henüz en iyi alış/satış fiyat verisi alınmadı.\n");
        } else {
            for symbol in &sorted_symbols {
                if let Some(m) = metrics_guard.get(symbol) {
                    report.push_str(&format!(
                        "[{}]  Bid: {:.4} | Ask: {:.4} (Örnek: {})\n\
                         ├─► ALİŞ (BID) TÜREVLERİ (Anlık / 30s Ort):\n\
                         │   ├─ 1. Türev (Hız/Vel) : {:+10.4} USDT/s | Ort: {:+10.4} USDT/s\n\
                         │   ├─ 2. Türev (İvme/Acc): {:+10.4} USDT/s²| Ort: {:+10.4} USDT/s²\n\
                         │   └─ 3. Türev (Sars/Jrk): {:+10.4} USDT/s³| Ort: {:+10.4} USDT/s³\n\
                         └─► SATIŞ (ASK) TÜREVLERİ (Anlık / 30s Ort):\n\
                             ├─ 1. Türev (Hız/Vel) : {:+10.4} USDT/s | Ort: {:+10.4} USDT/s\n\
                             ├─ 2. Türev (İvme/Acc): {:+10.4} USDT/s²| Ort: {:+10.4} USDT/s²\n\
                             └─ 3. Türev (Sars/Jrk): {:+10.4} USDT/s³| Ort: {:+10.4} USDT/s³\n\n",
                        symbol,
                        m.latest_best_bid,
                        m.latest_best_ask,
                        m.sample_count,
                        m.bid_instantaneous.d1_velocity,
                        m.bid_30s_avg.d1_velocity,
                        m.bid_instantaneous.d2_acceleration,
                        m.bid_30s_avg.d2_acceleration,
                        m.bid_instantaneous.d3_jerk,
                        m.bid_30s_avg.d3_jerk,
                        m.ask_instantaneous.d1_velocity,
                        m.ask_30s_avg.d1_velocity,
                        m.ask_instantaneous.d2_acceleration,
                        m.ask_30s_avg.d2_acceleration,
                        m.ask_instantaneous.d3_jerk,
                        m.ask_30s_avg.d3_jerk,
                    ));
                }
            }
        }
        report.push_str("============================================================\n");
        report
    }
}

/// Calculate 1st, 2nd, and 3rd derivatives over a sequence of PricePoints
fn compute_derivatives(points: &VecDeque<PricePoint>, is_bid: bool) -> (DerivativeValues, DerivativeValues) {
    if points.len() < 2 {
        return (DerivativeValues::default(), DerivativeValues::default());
    }

    // Extract price array & time array (in seconds)
    let mut times: Vec<f64> = Vec::with_capacity(points.len());
    let mut prices: Vec<f64> = Vec::with_capacity(points.len());

    for p in points {
        times.push(p.timestamp_ms as f64 / 1000.0);
        prices.push(if is_bid { p.best_bid } else { p.best_ask });
    }

    // 1st derivatives (velocities): v[i] = (price[i] - price[i-1]) / dt
    let mut v_times: Vec<f64> = Vec::new();
    let mut v_vals: Vec<f64> = Vec::new();
    for i in 1..prices.len() {
        let dt = times[i] - times[i - 1];
        if dt > 0.0 {
            let v = (prices[i] - prices[i - 1]) / dt;
            v_times.push(times[i]);
            v_vals.push(v);
        }
    }

    // 2nd derivatives (accelerations): a[i] = (v[i] - v[i-1]) / dt
    let mut a_times: Vec<f64> = Vec::new();
    let mut a_vals: Vec<f64> = Vec::new();
    for i in 1..v_vals.len() {
        let dt = v_times[i] - v_times[i - 1];
        if dt > 0.0 {
            let a = (v_vals[i] - v_vals[i - 1]) / dt;
            a_times.push(v_times[i]);
            a_vals.push(a);
        }
    }

    // 3rd derivatives (jerks): j[i] = (a[i] - a[i-1]) / dt
    let mut j_vals: Vec<f64> = Vec::new();
    for i in 1..a_vals.len() {
        let dt = a_times[i] - a_times[i - 1];
        if dt > 0.0 {
            let j = (a_vals[i] - a_vals[i - 1]) / dt;
            j_vals.push(j);
        }
    }

    let inst_d1 = v_vals.last().copied().unwrap_or(0.0);
    let inst_d2 = a_vals.last().copied().unwrap_or(0.0);
    let inst_d3 = j_vals.last().copied().unwrap_or(0.0);

    let avg_d1 = if !v_vals.is_empty() { v_vals.iter().sum::<f64>() / v_vals.len() as f64 } else { 0.0 };
    let avg_d2 = if !a_vals.is_empty() { a_vals.iter().sum::<f64>() / a_vals.len() as f64 } else { 0.0 };
    let avg_d3 = if !j_vals.is_empty() { j_vals.iter().sum::<f64>() / j_vals.len() as f64 } else { 0.0 };

    (
        DerivativeValues {
            d1_velocity: inst_d1,
            d2_acceleration: inst_d2,
            d3_jerk: inst_d3,
        },
        DerivativeValues {
            d1_velocity: avg_d1,
            d2_acceleration: avg_d2,
            d3_jerk: avg_d3,
        },
    )
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

fn parse_u64(val: &Value) -> Option<u64> {
    if let Some(i) = val.as_u64() {
        Some(i)
    } else if let Some(i) = val.as_i64() {
        Some(i as u64)
    } else if let Some(s) = val.as_str() {
        s.parse::<u64>().ok()
    } else {
        None
    }
}

struct PluginState {
    is_running: Arc<AtomicBool>,
    engine: Arc<BookTickerDerivativesEngine>,
    data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let engine = Arc::new(BookTickerDerivativesEngine::new());
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

                if stream_id == "stream_bestprice" || stream_id.contains("bestprice") || stream_id.contains("bookticker") {
                    if let Ok(json_data) = serde_json::from_slice::<Value>(&slice[32..]) {
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let report = state.engine.process_bestprice_payload(&json_data, now_ms);

                        // Update RAM buffer safely
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
    fn test_derivatives_calculation_precision() {
        let engine = BookTickerDerivativesEngine::new();
        engine.set_window_ms(30000);

        // Simulate 4 sequential ticks with constant velocity and acceleration
        // t0 = 1000s, bid = 100.0
        // t1 = 1001s, bid = 102.0 (v1 = +2.0)
        // t2 = 1002s, bid = 105.0 (v2 = +3.0 => a1 = +1.0)
        // t3 = 1003s, bid = 109.0 (v3 = +4.0 => a2 = +1.0 => j1 = 0.0)
        let t0 = 1000000u64;
        let t1 = 1001000u64;
        let t2 = 1002000u64;
        let t3 = 1003000u64;

        let p0 = serde_json::json!({ "BTCUSDT": { "best_bid": "100.0", "best_ask": "100.5", "event_time": t0 } });
        let p1 = serde_json::json!({ "BTCUSDT": { "best_bid": "102.0", "best_ask": "102.5", "event_time": t1 } });
        let p2 = serde_json::json!({ "BTCUSDT": { "best_bid": "105.0", "best_ask": "105.5", "event_time": t2 } });
        let p3 = serde_json::json!({ "BTCUSDT": { "best_bid": "109.0", "best_ask": "109.5", "event_time": t3 } });

        engine.process_bestprice_payload(&p0, t0);
        engine.process_bestprice_payload(&p1, t1);
        engine.process_bestprice_payload(&p2, t2);
        let report = engine.process_bestprice_payload(&p3, t3);

        assert!(report.contains("BOOKTICKER TÜREV ANALİZİ"));
        assert!(report.contains("BTCUSDT"));

        let metrics_guard = engine.metrics.lock().unwrap();
        let btc = metrics_guard.get("BTCUSDT").unwrap();

        assert_eq!(btc.sample_count, 4);
        assert_eq!(btc.latest_best_bid, 109.0);
        assert_eq!(btc.latest_best_ask, 109.5);

        // Velocity at t3 should be +4.0
        assert!((btc.bid_instantaneous.d1_velocity - 4.0).abs() < 1e-5);
        // Acceleration at t3 should be +1.0
        assert!((btc.bid_instantaneous.d2_acceleration - 1.0).abs() < 1e-5);
        // Jerk at t3 should be 0.0
        assert!((btc.bid_instantaneous.d3_jerk - 0.0).abs() < 1e-5);
    }
}
