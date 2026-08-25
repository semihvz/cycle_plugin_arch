use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const WINDOW_SPECS: &[(u64, &str)] = &[
    (100, "100ms"),
    (500, "500ms"),
    (1000, "1s"),
    (5000, "5s"),
    (10000, "10s"),
    (30000, "30s"),
    (60000, "60s"),
];

/// Top-of-Book Mid Price Snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestPriceSnapshot {
    pub symbol: String,
    pub best_bid: f64,
    pub best_ask: f64,
    pub mid_price: f64,
    pub timestamp_ms: u64,
}

/// Trade Record from Exchange Feed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub trade_id: i64,
    pub symbol: String,
    pub price: f64,
    pub quantity: f64,
    pub usdt_value: f64,
    pub buyer_is_maker: bool, // true = seller taker (market sell), false = buyer taker (market buy)
    pub timestamp_ms: u64,
}

/// Single Window Metrics (e.g. 100ms, 500ms, 1s, 5s, 10s, 30s, 60s)
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WindowImpactMetrics {
    pub window_label: String,
    pub window_ms: u64,
    pub mid_price_ago: f64,
    pub price_impact_pct: f64,
    pub total_trades: usize,
    pub total_volume_usdt: f64,
    pub buy_volume_usdt: f64,
    pub buy_trades_count: usize,
    pub sell_volume_usdt: f64,
    pub sell_trades_count: usize,
}

/// Multi-Window Rolling Price Impact & Trade Summary Metrics per Symbol
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SymbolPriceImpactMetrics {
    pub symbol: String,
    pub mid_price_now: f64,
    pub windows: Vec<WindowImpactMetrics>,
    pub latest_trade_price: f64,
    pub last_updated_ms: u64,
}

pub struct PriceImpactEngine {
    pub min_trade_usdt_x100: AtomicU64,

    pub best_price_history: Mutex<HashMap<String, VecDeque<BestPriceSnapshot>>>,
    pub trade_history: Mutex<HashMap<String, VecDeque<TradeRecord>>>,
    pub symbol_metrics: Mutex<HashMap<String, SymbolPriceImpactMetrics>>,
    pub last_seen_trade_id: Mutex<HashMap<String, i64>>,
}

impl PriceImpactEngine {
    pub fn new() -> Self {
        Self {
            min_trade_usdt_x100: AtomicU64::new(0),
            best_price_history: Mutex::new(HashMap::new()),
            trade_history: Mutex::new(HashMap::new()),
            symbol_metrics: Mutex::new(HashMap::new()),
            last_seen_trade_id: Mutex::new(HashMap::new()),
        }
    }

    pub fn configure(&self, _window_ms: Option<u64>, min_trade_usdt: Option<f64>) {
        if let Some(min_usdt) = min_trade_usdt {
            let u_fixed = (min_usdt * 100.0).max(0.0) as u64;
            self.min_trade_usdt_x100.store(u_fixed, Ordering::Relaxed);
        }
    }

    /// Process incoming payload (bestprice, trades/aggtrades, or combined gateway dump)
    pub fn process_payload(&self, json_data: &Value, now_ms: u64) -> String {
        let min_usdt = self.min_trade_usdt_x100.load(Ordering::Relaxed) as f64 / 100.0;

        if let Some(obj) = json_data.as_object() {
            if obj.contains_key("stream_bestprice") || obj.contains_key("stream_trades") || obj.contains_key("stream_aggtrades") {
                if let Some(bp) = obj.get("stream_bestprice") {
                    self.ingest_bestprice(bp, now_ms);
                }
                if let Some(tr) = obj.get("stream_trades") {
                    self.ingest_trades(tr, now_ms, min_usdt);
                }
                if let Some(ag) = obj.get("stream_aggtrades") {
                    self.ingest_trades(ag, now_ms, min_usdt);
                }
            } else {
                let sample_item = obj.values().next();
                if let Some(item) = sample_item {
                    if item.get("best_bid").is_some() || item.get("best_ask").is_some() {
                        self.ingest_bestprice(json_data, now_ms);
                    } else if item.get("price").is_some() && (item.get("quantity").is_some() || item.get("trade_id").is_some()) {
                        self.ingest_trades(json_data, now_ms, min_usdt);
                    }
                }
            }
        }

        self.generate_report(now_ms)
    }

    /// Ingest Top-of-Book Best Prices update into mid price history queue
    pub fn ingest_bestprice(&self, json_data: &Value, now_ms: u64) {
        let mut price_guard = self.best_price_history.lock().unwrap();

        if let Some(obj) = json_data.as_object() {
            for (symbol, item) in obj.iter() {
                let best_bid = parse_f64(&item["best_bid"]);
                let best_ask = parse_f64(&item["best_ask"]);
                let event_time = parse_u64(&item["local_recv_time_ms"])
                    .filter(|&t| t > 0)
                    .or_else(|| parse_u64(&item["event_time"]).filter(|&t| t > 0))
                    .unwrap_or(now_ms);

                if best_bid > 0.0 || best_ask > 0.0 {
                    let mid_price = if best_bid > 0.0 && best_ask > 0.0 {
                        (best_bid + best_ask) / 2.0
                    } else {
                        best_bid.max(best_ask)
                    };

                    let snap = BestPriceSnapshot {
                        symbol: symbol.clone(),
                        best_bid,
                        best_ask,
                        mid_price,
                        timestamp_ms: event_time,
                    };

                    let queue = price_guard.entry(symbol.clone()).or_insert_with(VecDeque::new);
                    // Avoid inserting exact duplicate timestamps
                    if queue.back().map_or(true, |last| last.timestamp_ms < event_time) {
                        queue.push_back(snap);
                    }

                    // Keep up to 65 seconds of price history for 60s window evaluation
                    let max_history_ms = 65000;
                    let cutoff_ts = event_time.saturating_sub(max_history_ms);
                    while queue.len() > 1 {
                        if let Some(front) = queue.front() {
                            if front.timestamp_ms < cutoff_ts {
                                queue.pop_front();
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    if queue.len() > 10000 {
                        queue.pop_front();
                    }
                }
            }
        }
    }

    /// Ingest Trade feed update into trade history queue
    pub fn ingest_trades(&self, json_data: &Value, now_ms: u64, min_usdt: f64) {
        let mut trade_guard = self.trade_history.lock().unwrap();
        let mut last_seen_guard = self.last_seen_trade_id.lock().unwrap();

        if let Some(obj) = json_data.as_object() {
            for (symbol, item) in obj.iter() {
                let trade_id = parse_i64(&item["trade_id"]);
                if trade_id > 0 {
                    if let Some(&last_id) = last_seen_guard.get(symbol) {
                        if last_id == trade_id {
                            continue; // Skip duplicate trade payload
                        }
                    }
                    last_seen_guard.insert(symbol.clone(), trade_id);
                }

                let price = parse_f64(&item["price"]);
                let quantity = parse_f64(&item["quantity"]);
                let buyer_is_maker = item["buyer_is_maker"].as_bool().unwrap_or(false);
                let event_time = parse_u64(&item["local_recv_time_ms"])
                    .filter(|&t| t > 0)
                    .or_else(|| parse_u64(&item["event_time"]).filter(|&t| t > 0))
                    .unwrap_or(now_ms);

                let usdt_value = price * quantity;
                if price <= 0.0 || usdt_value < min_usdt {
                    continue;
                }

                let trade = TradeRecord {
                    trade_id,
                    symbol: symbol.clone(),
                    price,
                    quantity,
                    usdt_value,
                    buyer_is_maker,
                    timestamp_ms: event_time,
                };

                let queue = trade_guard.entry(symbol.clone()).or_insert_with(VecDeque::new);
                queue.push_back(trade);

                // Keep up to 65 seconds of trade history for 60s window evaluation
                let max_history_ms = 65000;
                let cutoff_ts = event_time.saturating_sub(max_history_ms);
                while let Some(front) = queue.front() {
                    if front.timestamp_ms < cutoff_ts {
                        queue.pop_front();
                    } else {
                        break;
                    }
                }
                if queue.len() > 10000 {
                    queue.pop_front();
                }
            }
        }
    }

    /// Generate multi-window (100ms, 500ms, 1s, 5s, 10s, 30s, 60s) price impact & trade summary report
    pub fn generate_report(&self, now_ms: u64) -> String {
        let price_guard = self.best_price_history.lock().unwrap();
        let trade_guard = self.trade_history.lock().unwrap();
        let mut metrics_guard = self.symbol_metrics.lock().unwrap();

        let mut symbols_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for k in price_guard.keys() { symbols_set.insert(k.clone()); }
        for k in trade_guard.keys() { symbols_set.insert(k.clone()); }

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str("⚡ MULTI-WINDOW PRICE IMPACT & TRADE ANALİZİ (100ms..60s)\n");
        report.push_str("============================================================\n");

        if symbols_set.is_empty() {
            report.push_str("Henüz Binance Gateway trade veya bestprice verisi alınmadı.\n");
        } else {
            for symbol in &symbols_set {
                let empty_p_queue = VecDeque::new();
                let empty_t_queue = VecDeque::new();

                let p_queue = price_guard.get(symbol).unwrap_or(&empty_p_queue);
                let t_queue = trade_guard.get(symbol).unwrap_or(&empty_t_queue);

                let mid_price_now = p_queue.back().map(|s| s.mid_price).unwrap_or(0.0);
                let last_price_ts = p_queue.back().map(|s| s.timestamp_ms).unwrap_or(now_ms);
                let latest_trade_price = t_queue.back().map(|t| t.price).unwrap_or(0.0);

                let mut window_metrics_list = Vec::new();
                for &(win_ms, win_label) in WINDOW_SPECS {
                    let w_metric = calc_window_metrics(p_queue, t_queue, now_ms, win_ms, win_label);
                    window_metrics_list.push(w_metric);
                }

                let metrics_item = SymbolPriceImpactMetrics {
                    symbol: symbol.clone(),
                    mid_price_now,
                    windows: window_metrics_list.clone(),
                    latest_trade_price,
                    last_updated_ms: last_price_ts,
                };

                metrics_guard.insert(symbol.clone(), metrics_item);

                report.push_str(&format!(
                    "[{}]  Şimdiki Mid Fiyat: {:.8} (Son Trade: {:.8})\n",
                    symbol, mid_price_now, latest_trade_price
                ));

                let total_wins = window_metrics_list.len();
                for (idx, w) in window_metrics_list.iter().enumerate() {
                    let prefix = if idx + 1 == total_wins { " └─►" } else { " ├─►" };
                    report.push_str(&format!(
                        "{} {:>6} | Mid: {:.8} | Impact: {:+.8}% | Hacim: ${:.8} USDT ({} İşlem) [Buy: ${:.2} ({}) | Sell: ${:.2} ({})]\n",
                        prefix,
                        w.window_label,
                        w.mid_price_ago,
                        w.price_impact_pct,
                        w.total_volume_usdt,
                        w.total_trades,
                        w.buy_volume_usdt,
                        w.buy_trades_count,
                        w.sell_volume_usdt,
                        w.sell_trades_count
                    ));
                }
                report.push('\n');
            }
        }
        report
    }

    pub fn get_metrics_json(&self) -> String {
        let metrics_guard = self.symbol_metrics.lock().unwrap();
        serde_json::to_string_pretty(&*metrics_guard).unwrap_or_else(|_| "{}".to_string())
    }
}

fn calc_window_metrics(
    price_queue: &VecDeque<BestPriceSnapshot>,
    trade_queue: &VecDeque<TradeRecord>,
    now_ms: u64,
    window_ms: u64,
    window_label: &str,
) -> WindowImpactMetrics {
    let mut metrics = WindowImpactMetrics {
        window_label: window_label.to_string(),
        window_ms,
        ..Default::default()
    };

    if price_queue.is_empty() {
        return metrics;
    }

    let latest_ts = price_queue.back().map(|s| s.timestamp_ms).unwrap_or(now_ms);
    let window_start_ms = latest_ts.saturating_sub(window_ms);
    let mid_price_now = price_queue.back().map(|s| s.mid_price).unwrap_or(0.0);

    // Find snapshot closest to window_start_ms
    let oldest_snap_in_window = price_queue
        .iter()
        .find(|s| s.timestamp_ms >= window_start_ms)
        .or_else(|| price_queue.front());

    if let Some(snap) = oldest_snap_in_window {
        metrics.mid_price_ago = snap.mid_price;
        if snap.mid_price > 0.0 {
            metrics.price_impact_pct = (mid_price_now - snap.mid_price) / snap.mid_price * 100.0;
        }
    }

    // Aggregate trades within window_start_ms
    for t in trade_queue.iter().rev() {
        if t.timestamp_ms < window_start_ms {
            break;
        }
        metrics.total_trades += 1;
        metrics.total_volume_usdt += t.usdt_value;
        if t.buyer_is_maker {
            // Taker Sell (Market Sell)
            metrics.sell_volume_usdt += t.usdt_value;
            metrics.sell_trades_count += 1;
        } else {
            // Taker Buy (Market Buy)
            metrics.buy_volume_usdt += t.usdt_value;
            metrics.buy_trades_count += 1;
        }
    }

    metrics
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

// C ABI Plugin State & Exported Endpoints
struct PluginState {
    is_running: Arc<AtomicBool>,
    engine: Arc<PriceImpactEngine>,
    data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        engine: Arc::new(PriceImpactEngine::new()),
        data: Arc::new(Mutex::new(Vec::new())),
    });

    *state_out = Box::into_raw(state) as *mut c_void;
    handle_endpoint
}

pub unsafe extern "C" fn handle_endpoint(
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
                    let win = params.get("window_ms").and_then(|v| v.as_u64())
                        .or_else(|| params.get("window_secs").and_then(|v| v.as_u64()).map(|s| s * 1000));
                    let min_u = params.get("min_trade_usdt").and_then(|v| v.as_f64());
                    state.engine.configure(win, min_u);
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

                if stream_id.contains("bestprice")
                    || stream_id.contains("trades")
                    || stream_id.contains("aggtrades")
                    || stream_id.contains("gateway")
                {
                    if let Ok(json_data) = serde_json::from_slice::<Value>(&slice[32..]) {
                        let now_ms = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64;

                        let report = state.engine.process_payload(&json_data, now_ms);

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
