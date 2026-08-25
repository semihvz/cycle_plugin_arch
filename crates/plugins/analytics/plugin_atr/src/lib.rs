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
pub struct AtrMetrics {
    pub symbol: String,
    pub interval: String,
    pub period: usize,
    pub bar_count: usize,
    pub latest_close: f64,
    pub latest_tr: f64,
    pub latest_atr: f64,
    pub atr_pct_of_close: f64,
    pub atr_history: Vec<f64>,
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

pub fn calculate_atr_14(symbol: &str, interval: &str, bars: &[Bar], period: usize) -> Option<AtrMetrics> {
    if bars.is_empty() {
        return None;
    }

    let mut tr_list = Vec::with_capacity(bars.len());
    for i in 0..bars.len() {
        let tr = if i == 0 {
            bars[i].high - bars[i].low
        } else {
            let high_low = bars[i].high - bars[i].low;
            let high_prev_close = (bars[i].high - bars[i - 1].close).abs();
            let low_prev_close = (bars[i].low - bars[i - 1].close).abs();
            high_low.max(high_prev_close).max(low_prev_close)
        };
        tr_list.push(tr);
    }

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let latest_close = bars.last().map(|b| b.close).unwrap_or(0.0);
    let latest_tr = *tr_list.last().unwrap_or(&0.0);

    if tr_list.len() < period {
        let sum: f64 = tr_list.iter().sum();
        let avg_atr = if tr_list.is_empty() { 0.0 } else { sum / tr_list.len() as f64 };
        let atr_pct = if latest_close > 0.0 { (avg_atr / latest_close) * 100.0 } else { 0.0 };
        return Some(AtrMetrics {
            symbol: symbol.to_string(),
            interval: interval.to_string(),
            period,
            bar_count: bars.len(),
            latest_close,
            latest_tr,
            latest_atr: avg_atr,
            atr_pct_of_close: atr_pct,
            atr_history: vec![avg_atr],
            last_updated_ms: now_ms,
        });
    }

    // Wilder's ATR(14)
    let mut atr_series = Vec::with_capacity(bars.len());
    let first_sma: f64 = tr_list[0..period].iter().sum::<f64>() / (period as f64);
    for _ in 0..period - 1 {
        atr_series.push(0.0);
    }
    atr_series.push(first_sma);

    let period_f = period as f64;
    let mut current_atr = first_sma;

    for i in period..tr_list.len() {
        current_atr = (current_atr * (period_f - 1.0) + tr_list[i]) / period_f;
        atr_series.push(current_atr);
    }

    let latest_atr = *atr_series.last().unwrap_or(&0.0);
    let atr_pct = if latest_close > 0.0 { (latest_atr / latest_close) * 100.0 } else { 0.0 };

    Some(AtrMetrics {
        symbol: symbol.to_string(),
        interval: interval.to_string(),
        period,
        bar_count: bars.len(),
        latest_close,
        latest_tr,
        latest_atr,
        atr_pct_of_close: atr_pct,
        atr_history: atr_series.iter().filter(|&&v| v > 0.0).copied().collect(),
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
        .expect("Tokio runtime could not be created");

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
            
            // Format report
            let mut report = String::new();
            report.push_str("============================================================\n");
            report.push_str("📊 ATR (14) VERİ KANALI [1m / 100 BAR]\n");
            report.push_str("============================================================\n");
            
            if guard.is_empty() {
                report.push_str("Henüz ATR verisi hesaplanmadı (ohlcv_fetcher verisi bekleniyor).\n");
            } else {
                for (stream_id, val) in guard.iter() {
                    if let Ok(metrics) = serde_json::from_value::<AtrMetrics>(val.clone()) {
                        report.push_str(&format!(
                            "[{}] Sym: {:<8} | Close: {:<10.2} | TR: {:<8.4} | ATR(14): {:<8.4} | ATR%: {:<6.2}% | Bars: {}\n",
                            stream_id, metrics.symbol, metrics.latest_close, metrics.latest_tr, metrics.latest_atr, metrics.atr_pct_of_close, metrics.bar_count
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
                                        close_time: parse_u64(&row_arr[6]),
                                    });
                                }
                            }
                        }

                        if !bars.is_empty() {
                            if let Some(metrics) = calculate_atr_14(&symbol, &interval, &bars, 14) {
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
