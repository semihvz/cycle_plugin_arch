use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
pub struct RsiMetrics {
    pub symbol: String,
    pub interval: String,
    pub period: usize,
    pub bar_count: usize,
    pub latest_close: f64,
    pub latest_rsi: f64,
    pub avg_gain: f64,
    pub avg_loss: f64,
    pub rs: f64,
    pub state: String,
    pub rsi_history: Vec<f64>,
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

pub fn calculate_rsi_14(symbol: &str, interval: &str, bars: &[Bar], period: usize) -> Option<RsiMetrics> {
    if bars.len() < 2 {
        return None;
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let latest_close = bars.last().map(|b| b.close).unwrap_or(0.0);

    let mut gains = Vec::with_capacity(bars.len() - 1);
    let mut losses = Vec::with_capacity(bars.len() - 1);

    for i in 1..bars.len() {
        let diff = bars[i].close - bars[i - 1].close;
        if diff > 0.0 {
            gains.push(diff);
            losses.push(0.0);
        } else {
            gains.push(0.0);
            losses.push(-diff);
        }
    }

    let num_changes = gains.len();
    if num_changes < period {
        let sum_gain: f64 = gains.iter().sum();
        let sum_loss: f64 = losses.iter().sum();
        let avg_g = sum_gain / num_changes as f64;
        let avg_l = sum_loss / num_changes as f64;
        let (rs, rsi) = if avg_l == 0.0 {
            if avg_g == 0.0 { (0.0, 50.0) } else { (f64::INFINITY, 100.0) }
        } else {
            let rs_val = avg_g / avg_l;
            (rs_val, 100.0 - (100.0 / (1.0 + rs_val)))
        };

        let state = if rsi >= 70.0 {
            "OVERBOUGHT"
        } else if rsi <= 30.0 {
            "OVERSOLD"
        } else {
            "NEUTRAL"
        };

        return Some(RsiMetrics {
            symbol: symbol.to_string(),
            interval: interval.to_string(),
            period,
            bar_count: bars.len(),
            latest_close,
            latest_rsi: rsi,
            avg_gain: avg_g,
            avg_loss: avg_l,
            rs,
            state: state.to_string(),
            rsi_history: vec![rsi],
            last_updated_ms: now_ms,
        });
    }

    // Wilder's RSI calculation
    let mut rsi_series = Vec::with_capacity(num_changes - period + 1);

    // Initial 14-period average
    let mut avg_g: f64 = gains[0..period].iter().sum::<f64>() / (period as f64);
    let mut avg_l: f64 = losses[0..period].iter().sum::<f64>() / (period as f64);

    let first_rs = if avg_l == 0.0 {
        if avg_g == 0.0 { 0.0 } else { f64::INFINITY }
    } else {
        avg_g / avg_l
    };
    let first_rsi = if avg_l == 0.0 {
        if avg_g == 0.0 { 50.0 } else { 100.0 }
    } else {
        100.0 - (100.0 / (1.0 + first_rs))
    };
    rsi_series.push(first_rsi);

    let period_f = period as f64;

    for i in period..num_changes {
        avg_g = (avg_g * (period_f - 1.0) + gains[i]) / period_f;
        avg_l = (avg_l * (period_f - 1.0) + losses[i]) / period_f;

        let rs_val = if avg_l == 0.0 {
            if avg_g == 0.0 { 0.0 } else { f64::INFINITY }
        } else {
            avg_g / avg_l
        };

        let rsi_val = if avg_l == 0.0 {
            if avg_g == 0.0 { 50.0 } else { 100.0 }
        } else {
            100.0 - (100.0 / (1.0 + rs_val))
        };

        rsi_series.push(rsi_val);
    }

    let latest_rsi = *rsi_series.last().unwrap_or(&50.0);
    let rs = if avg_l == 0.0 {
        if avg_g == 0.0 { 0.0 } else { f64::INFINITY }
    } else {
        avg_g / avg_l
    };

    let state = if latest_rsi >= 70.0 {
        "OVERBOUGHT"
    } else if latest_rsi <= 30.0 {
        "OVERSOLD"
    } else {
        "NEUTRAL"
    };

    Some(RsiMetrics {
        symbol: symbol.to_string(),
        interval: interval.to_string(),
        period,
        bar_count: bars.len(),
        latest_close,
        latest_rsi,
        avg_gain: avg_g,
        avg_loss: avg_l,
        rs,
        state: state.to_string(),
        rsi_history: rsi_series,
        last_updated_ms: now_ms,
    })
}

struct PluginState {
    _runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<HashMap<String, Value>>>,
    outbox: Arc<Mutex<Vec<Value>>>,
    stream_configs: Arc<Mutex<HashMap<String, (String, String)>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let _runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Tokio runtime olusturulamadi");

    let state = Box::new(PluginState {
        _runtime,
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(HashMap::new())),
        outbox: Arc::new(Mutex::new(Vec::new())),
        stream_configs: Arc::new(Mutex::new(HashMap::new())),
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
            state.is_running.store(true, Ordering::Relaxed);

            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(config) = serde_json::from_slice::<Value>(slice) {
                    if let Some(inputs) = config.get("plugin_inputs").and_then(|i| i.as_array()) {
                        let mut q = state.outbox.lock().unwrap();
                        for input in inputs {
                            if let (Some(source), Some(params), Some(stream_id)) = (
                                input.get("source").and_then(|s| s.as_str()),
                                input.get("params").and_then(|p| p.as_object()),
                                input.get("stream_id").and_then(|s| s.as_str()),
                            ) {
                                let mut req = serde_json::Map::new();
                                req.insert("to".to_string(), serde_json::json!(source));
                                req.insert("stream_id".to_string(), serde_json::json!(stream_id));
                                for (k, v) in params {
                                    req.insert(k.clone(), v.clone());
                                }

                                let symbol = req.get("symbol").and_then(|v| v.as_str()).unwrap_or("BTCUSDT").to_string();
                                let interval = req.get("interval").and_then(|v| v.as_str()).unwrap_or("1m").to_string();

                                let mut configs = state.stream_configs.lock().unwrap();
                                configs.insert(stream_id.to_string(), (symbol, interval));

                                q.push(Value::Object(req));
                            }
                        }
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
            if state.is_running.load(Ordering::Relaxed) { 1 } else { 0 }
        }
        3 => { // DataValid
            1
        }
        4 | 5 => { // DataMonitor & RawData
            let guard = state.data.lock().unwrap();

            let mut report = String::new();
            report.push_str("============================================================\n");
            report.push_str("📊 RSI (14) VERİ KANALI [1m / 1440 BAR - REAL-TIME 1s]\n");
            report.push_str("============================================================\n");

            if guard.is_empty() {
                report.push_str("Henüz RSI verisi hesaplanmadı (ohlcv_fetcher verisi bekleniyor).\n");
            } else {
                for (stream_id, val) in guard.iter() {
                    if let Ok(metrics) = serde_json::from_value::<RsiMetrics>(val.clone()) {
                        report.push_str(&format!(
                            "[{}] Sym: {:<8} | Close: {:<10.2} | RSI(14): {:<6.2} | State: {:<10} | Bars: {}\n",
                            stream_id, metrics.symbol, metrics.latest_close, metrics.latest_rsi, metrics.state, metrics.bar_count
                        ));
                    }
                }
            }
            report.push_str("============================================================\n");

            let mut response_map = serde_json::Map::new();
            response_map.insert("formatted_report".to_string(), Value::String(report));
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
        6 => { // Inbox
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                let (stream_id, data_slice) = if payload_len > 32 {
                    let header = &slice[0..32];
                    let s_id = std::str::from_utf8(header)
                        .unwrap_or("")
                        .trim_matches(char::from(0))
                        .to_string();
                    (if s_id.is_empty() { "default".to_string() } else { s_id }, &slice[32..])
                } else {
                    ("default".to_string(), slice)
                };

                let (symbol, interval) = {
                    let configs = state.stream_configs.lock().unwrap();
                    configs.get(&stream_id)
                        .cloned()
                        .unwrap_or_else(|| (
                            if stream_id.contains("btc") { "BTCUSDT".to_string() }
                            else if stream_id.contains("eth") { "ETHUSDT".to_string() }
                            else if stream_id.contains("tac") { "TACUSDT".to_string() }
                            else { "BTCUSDT".to_string() },
                            "1m".to_string()
                        ))
                };

                if let Ok(data_value) = serde_json::from_slice::<Value>(data_slice) {
                    let arr_opt = if data_value.is_array() {
                        data_value.as_array().cloned()
                    } else if let Some(arr) = data_value.get("data").and_then(|d| d.as_array()) {
                        Some(arr.clone())
                    } else {
                        None
                    };

                    if let Some(arr) = arr_opt {
                        let mut bars = Vec::new();
                        for row in arr {
                            if let Some(row_arr) = row.as_array() {
                                if row_arr.len() >= 6 {
                                    bars.push(Bar {
                                        open_time: parse_u64(&row_arr[0]),
                                        open: parse_f64(&row_arr[1]),
                                        high: parse_f64(&row_arr[2]),
                                        low: parse_f64(&row_arr[3]),
                                        close: parse_f64(&row_arr[4]),
                                        volume: parse_f64(&row_arr[5]),
                                        close_time: if row_arr.len() > 6 { parse_u64(&row_arr[6]) } else { 0 },
                                    });
                                }
                            }
                        }

                        if !bars.is_empty() {
                            if let Some(metrics) = calculate_rsi_14(&symbol, &interval, &bars, 14) {
                                if let Ok(val) = serde_json::to_value(&metrics) {
                                    let mut guard = state.data.lock().unwrap();
                                    guard.insert(stream_id, val);
                                }
                            }
                        }
                    }
                }
            }
            0
        }
        7 => { // Outbox
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
