use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Raw input bar (e.g. 1-minute candle).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bar {
    pub open_time: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    #[serde(default)]
    pub quote_volume: Option<f64>,
    #[serde(default)]
    pub trades_count: Option<usize>,
    #[serde(default)]
    pub buy_volume: Option<f64>,
    #[serde(default)]
    pub sell_volume: Option<f64>,
    #[serde(default)]
    pub close_time: Option<u64>,
}

/// Output resampled candle (e.g., 15m, 1h, 4h, 1d).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResampledCandle {
    pub symbol: String,
    pub target_interval: String,
    pub open_time: u64,
    pub close_time: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub quote_volume: f64,
    pub trades_count: usize,
    pub buy_volume: f64,
    pub sell_volume: f64,
    pub is_closed: bool,
    pub bar_count: usize,
    pub last_updated_ms: u64,
}

/// Converts timeframe string (e.g., "15m", "1h", "4h", "1d") to duration in seconds.
pub fn interval_to_seconds(interval: &str) -> u64 {
    let interval = interval.trim().to_lowercase();
    if interval.ends_with('s') {
        interval[..interval.len() - 1].parse::<u64>().unwrap_or(60)
    } else if interval.ends_with('m') {
        let mins = interval[..interval.len() - 1].parse::<u64>().unwrap_or(15);
        mins * 60
    } else if interval.ends_with('h') {
        let hours = interval[..interval.len() - 1].parse::<u64>().unwrap_or(1);
        hours * 3600
    } else if interval.ends_with('d') {
        let days = interval[..interval.len() - 1].parse::<u64>().unwrap_or(1);
        days * 86400
    } else if interval.ends_with('w') {
        let weeks = interval[..interval.len() - 1].parse::<u64>().unwrap_or(1);
        weeks * 604800
    } else {
        // Default to 15m (900s) if unparseable
        interval.parse::<u64>().unwrap_or(900)
    }
}

/// Aligns a timestamp in seconds to the start boundary of the interval bucket.
pub fn align_timestamp_sec(timestamp_sec: u64, step_sec: u64) -> u64 {
    if step_sec == 0 {
        return timestamp_sec;
    }
    (timestamp_sec / step_sec) * step_sec
}

/// Resamples a sequence of input bars (e.g., 1m bars) into target higher-timeframe candles.
pub fn resample_bars(symbol: &str, target_interval: &str, bars: &[Bar]) -> Vec<ResampledCandle> {
    if bars.is_empty() {
        return Vec::new();
    }

    let step_sec = interval_to_seconds(target_interval);
    let mut sorted_bars = bars.to_vec();
    // Normalize timestamps (convert ms to sec if needed)
    for b in sorted_bars.iter_mut() {
        if b.open_time > 10_000_000_000 {
            b.open_time /= 1000;
        }
    }
    sorted_bars.sort_by_key(|b| b.open_time);

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let mut result: Vec<ResampledCandle> = Vec::new();
    let mut current_candle: Option<ResampledCandle> = None;

    for bar in sorted_bars {
        let bucket_open = align_timestamp_sec(bar.open_time, step_sec);
        let bucket_close = bucket_open + step_sec - 1;

        match current_candle.as_mut() {
            Some(c) if c.open_time == bucket_open => {
                // Aggregate into active candle
                c.high = c.high.max(bar.high);
                c.low = c.low.min(bar.low);
                c.close = bar.close;
                c.volume += bar.volume;
                c.quote_volume += bar.quote_volume.unwrap_or(bar.close * bar.volume);
                c.trades_count += bar.trades_count.unwrap_or(1);
                c.buy_volume += bar.buy_volume.unwrap_or(0.0);
                c.sell_volume += bar.sell_volume.unwrap_or(0.0);
                c.bar_count += 1;
                c.last_updated_ms = now_ms;
            }
            _ => {
                // If previous candle existed, mark as closed and append
                if let Some(mut prev) = current_candle.take() {
                    prev.is_closed = true;
                    result.push(prev);
                }

                // Start new candle
                current_candle = Some(ResampledCandle {
                    symbol: symbol.to_string(),
                    target_interval: target_interval.to_string(),
                    open_time: bucket_open,
                    close_time: bucket_close,
                    open: bar.open,
                    high: bar.high,
                    low: bar.low,
                    close: bar.close,
                    volume: bar.volume,
                    quote_volume: bar.quote_volume.unwrap_or(bar.close * bar.volume),
                    trades_count: bar.trades_count.unwrap_or(1),
                    buy_volume: bar.buy_volume.unwrap_or(0.0),
                    sell_volume: bar.sell_volume.unwrap_or(0.0),
                    is_closed: false,
                    bar_count: 1,
                    last_updated_ms: now_ms,
                });
            }
        }
    }

    if let Some(c) = current_candle {
        result.push(c);
    }

    result
}

/// Core resampler engine for managing multiple symbols and target timeframes.
pub struct OhlcvResamplerEngine {
    pub history_limit: AtomicU64,
    // (Symbol, TargetInterval) -> Active Resampled Candle
    pub active_candles: Mutex<HashMap<(String, String), ResampledCandle>>,
    // (Symbol, TargetInterval) -> History of completed resampled candles
    pub completed_candles: Mutex<HashMap<(String, String), VecDeque<ResampledCandle>>>,
}

impl OhlcvResamplerEngine {
    pub fn new(history_limit: usize) -> Self {
        Self {
            history_limit: AtomicU64::new(history_limit as u64),
            active_candles: Mutex::new(HashMap::new()),
            completed_candles: Mutex::new(HashMap::new()),
        }
    }

    pub fn process_bar(&self, symbol: &str, bar: &Bar, target_intervals: &[String]) -> Vec<ResampledCandle> {
        let mut open_sec = bar.open_time;
        if open_sec > 10_000_000_000 {
            open_sec /= 1000;
        }

        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let limit = self.history_limit.load(Ordering::Relaxed) as usize;
        let mut updated_or_closed_candles = Vec::new();

        let mut active_guard = self.active_candles.lock().unwrap();
        let mut completed_guard = self.completed_candles.lock().unwrap();

        for interval in target_intervals {
            let key = (symbol.to_string(), interval.clone());
            let step_sec = interval_to_seconds(interval);
            let bucket_open = align_timestamp_sec(open_sec, step_sec);
            let bucket_close = bucket_open + step_sec - 1;

            if let Some(active) = active_guard.get_mut(&key) {
                if active.open_time == bucket_open {
                    // Update active candle
                    active.high = active.high.max(bar.high);
                    active.low = active.low.min(bar.low);
                    active.close = bar.close;
                    active.volume += bar.volume;
                    active.quote_volume += bar.quote_volume.unwrap_or(bar.close * bar.volume);
                    active.trades_count += bar.trades_count.unwrap_or(1);
                    active.buy_volume += bar.buy_volume.unwrap_or(0.0);
                    active.sell_volume += bar.sell_volume.unwrap_or(0.0);
                    active.bar_count += 1;
                    active.last_updated_ms = now_ms;

                    updated_or_closed_candles.push(active.clone());
                } else if open_sec > active.open_time {
                    // Previous candle is completed
                    let mut prev = active.clone();
                    prev.is_closed = true;
                    updated_or_closed_candles.push(prev.clone());

                    let history = completed_guard.entry(key.clone()).or_insert_with(VecDeque::new);
                    history.push_back(prev);
                    if history.len() > limit {
                        history.pop_front();
                    }

                    // Create new active candle
                    let new_active = ResampledCandle {
                        symbol: symbol.to_string(),
                        target_interval: interval.clone(),
                        open_time: bucket_open,
                        close_time: bucket_close,
                        open: bar.open,
                        high: bar.high,
                        low: bar.low,
                        close: bar.close,
                        volume: bar.volume,
                        quote_volume: bar.quote_volume.unwrap_or(bar.close * bar.volume),
                        trades_count: bar.trades_count.unwrap_or(1),
                        buy_volume: bar.buy_volume.unwrap_or(0.0),
                        sell_volume: bar.sell_volume.unwrap_or(0.0),
                        is_closed: false,
                        bar_count: 1,
                        last_updated_ms: now_ms,
                    };
                    *active = new_active.clone();
                    updated_or_closed_candles.push(new_active);
                }
            } else {
                // New active candle
                let new_active = ResampledCandle {
                    symbol: symbol.to_string(),
                    target_interval: interval.clone(),
                    open_time: bucket_open,
                    close_time: bucket_close,
                    open: bar.open,
                    high: bar.high,
                    low: bar.low,
                    close: bar.close,
                    volume: bar.volume,
                    quote_volume: bar.quote_volume.unwrap_or(bar.close * bar.volume),
                    trades_count: bar.trades_count.unwrap_or(1),
                    buy_volume: bar.buy_volume.unwrap_or(0.0),
                    sell_volume: bar.sell_volume.unwrap_or(0.0),
                    is_closed: false,
                    bar_count: 1,
                    last_updated_ms: now_ms,
                };
                active_guard.insert(key, new_active.clone());
                updated_or_closed_candles.push(new_active);
            }
        }

        updated_or_closed_candles
    }
}

// ============================================================================
// Cycle Orchestrator C-ABI Dynamic Plugin Implementation
// ============================================================================

struct PluginState {
    _runtime: tokio::runtime::Runtime,
    is_running: Arc<AtomicBool>,
    engine: Arc<OhlcvResamplerEngine>,
    outbox: Arc<Mutex<Vec<Value>>>,
    // stream_id -> (symbol, target_intervals)
    stream_configs: Arc<Mutex<HashMap<String, (String, Vec<String>)>>>,
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
        engine: Arc::new(OhlcvResamplerEngine::new(500)),
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
                                let target_intervals = if let Some(arr) = req.get("target_intervals").and_then(|v| v.as_array()) {
                                    arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                                } else {
                                    vec!["15m".to_string(), "1h".to_string(), "4h".to_string(), "1d".to_string()]
                                };

                                let mut configs = state.stream_configs.lock().unwrap();
                                configs.insert(stream_id.to_string(), (symbol, target_intervals));

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
        4 | 5 => { // DataMonitor & RawData (TUI output)
            let active_guard = state.engine.active_candles.lock().unwrap();
            let mut report = String::new();
            report.push_str("============================================================\n");
            report.push_str("📊 OHLCV TIMEFRAME RESAMPLER PLUGIN (1m -> 15m, 1h, 4h, 1d)\n");
            report.push_str("============================================================\n");

            if active_guard.is_empty() {
                report.push_str("Henüz yeniden örneklenmiş mum verisi yok (1m veri bekleniyor).\n");
            } else {
                for ((sym, tf), candle) in active_guard.iter() {
                    report.push_str(&format!(
                        "[{:<8} | {:<4}] O: {:<9.2} H: {:<9.2} L: {:<9.2} C: {:<9.2} Vol: {:<10.2} Bars: {}\n",
                        sym, tf, candle.open, candle.high, candle.low, candle.close, candle.volume, candle.bar_count
                    ));
                }
            }

            let bytes = report.as_bytes();
            let copy_len = bytes.len().min(out_max_len);
            if copy_len > 0 && !out_buf.is_null() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, copy_len);
            }
            copy_len
        }
        6 => { // Inbox: process incoming 1m bars or trade ticks
            if payload_len > 0 && !payload.is_null() {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(value) = serde_json::from_slice::<Value>(slice) {
                    if let Ok(bar) = serde_json::from_value::<Bar>(value.clone()) {
                        let symbol = value.get("symbol").and_then(|v| v.as_str()).unwrap_or("BTCUSDT");
                        let configs = state.stream_configs.lock().unwrap();
                        let default_tfs = vec!["15m".to_string(), "1h".to_string()];
                        let target_tfs = configs.values()
                            .find(|(s, _)| s == symbol)
                            .map(|(_, tfs)| tfs)
                            .unwrap_or(&default_tfs);

                        let resampled = state.engine.process_bar(symbol, &bar, target_tfs);
                        let mut out_guard = state.outbox.lock().unwrap();
                        for candle in resampled {
                            if let Ok(val) = serde_json::to_value(candle) {
                                out_guard.push(val);
                            }
                        }
                    }
                }
            }
            0
        }
        7 => { // Outbox: pop generated resampled candles
            let mut out_guard = state.outbox.lock().unwrap();
            if out_guard.is_empty() {
                return 0;
            }

            let payload_str = match serde_json::to_string(&*out_guard) {
                Ok(s) => s,
                Err(_) => return 0,
            };
            out_guard.clear();

            let bytes = payload_str.as_bytes();
            let copy_len = bytes.len().min(out_max_len);
            if copy_len > 0 && !out_buf.is_null() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, copy_len);
            }
            copy_len
        }
        8 => { // Schema
            let schema = serde_json::json!({
                "name": "plugin_ohlcv_resampler",
                "version": "0.1.0",
                "description": "High-performance OHLCV timeframe resampler for Cycle Orchestrator",
                "inputs": ["1m_ohlcv"],
                "outputs": ["resampled_ohlcv"],
                "supported_intervals": ["3m", "5m", "15m", "30m", "1h", "2h", "4h", "6h", "8h", "12h", "1d"]
            }).to_string();

            let bytes = schema.as_bytes();
            let copy_len = bytes.len().min(out_max_len);
            if copy_len > 0 && !out_buf.is_null() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, copy_len);
            }
            copy_len
        }
        9 => { // Destruct
            let _ = Box::from_raw(plugin_state as *mut PluginState);
            0
        }
        _ => 0,
    }
}
