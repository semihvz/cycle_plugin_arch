use std::collections::{HashMap, VecDeque};
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub trade_id: i64,
    pub price: f64,
    pub quantity: f64,
    pub usdt_value: f64,
    pub is_buyer_maker: bool,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceLevelState {
    pub symbol: String,
    pub side: String, // "BID" (Alım/Buy) or "ASK" (Satış/Sell)
    pub price: f64,
    pub initial_visible_qty: f64,
    pub initial_visible_usdt: f64,
    pub last_visible_qty: f64,
    pub executed_qty: f64,
    pub executed_usdt: f64,
    pub last_refill_exec_qty: f64,
    pub refill_count: usize,
    pub first_seen_time_ms: u64,
    pub last_updated_time_ms: u64,
    pub is_alerted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IcebergEvent {
    pub id: u64,
    pub symbol: String,
    pub side: String, // "BUY_ICEBERG" (Alım Gizli Emri / Biriktirme) or "SELL_ICEBERG" (Satış Gizli Emri / Dağıtım)
    pub price: f64,
    pub visible_usdt: f64,
    pub executed_qty: f64,
    pub executed_usdt: f64,
    pub estimated_hidden_qty: f64,
    pub estimated_hidden_usdt: f64,
    pub refill_count: usize,
    pub execution_ratio: f64,
    pub iceberg_score: f64,
    pub alert_level: String, // "INFO", "MEDIUM", "HIGH", "CRITICAL"
    pub event_time_ms: u64,
    pub description: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SymbolIcebergMetrics {
    pub symbol: String,
    pub active_icebergs_count: usize,
    pub total_iceberg_events: usize,
    pub total_buy_icebergs: usize,
    pub total_sell_icebergs: usize,
    pub total_hidden_usdt_detected: f64,
    pub last_iceberg_time_ms: u64,
}

pub struct IcebergEngine {
    pub min_iceberg_usdt: AtomicU64,     // Min executed USDT value (e.g. $30,000)
    pub min_exec_ratio_x10: AtomicU64,   // Min execution ratio x10 (e.g. 25 = 2.5x)
    pub min_refill_count: AtomicU64,     // Min refills (e.g. 2)
    pub window_ms: AtomicU64,            // Sliding window (e.g. 60,000 ms)

    pub tracked_levels: Mutex<HashMap<String, HashMap<String, PriceLevelState>>>,
    pub trade_history: Mutex<HashMap<String, VecDeque<TradeRecord>>>,
    pub iceberg_events: Mutex<VecDeque<IcebergEvent>>,
    pub symbol_metrics: Mutex<HashMap<String, SymbolIcebergMetrics>>,
    pub event_counter: AtomicU64,
}

impl IcebergEngine {
    pub fn new() -> Self {
        Self {
            min_iceberg_usdt: AtomicU64::new(30000),    // $30,000 USDT minimum executed
            min_exec_ratio_x10: AtomicU64::new(25),     // 2.5x ratio
            min_refill_count: AtomicU64::new(2),       // At least 2 refills
            window_ms: AtomicU64::new(60000),          // 60 seconds tracking window
            tracked_levels: Mutex::new(HashMap::new()),
            trade_history: Mutex::new(HashMap::new()),
            iceberg_events: Mutex::new(VecDeque::new()),
            symbol_metrics: Mutex::new(HashMap::new()),
            event_counter: AtomicU64::new(1),
        }
    }

    pub fn configure(&self, min_usdt: Option<u64>, min_ratio_x10: Option<u64>, min_refills: Option<u64>, window_ms: Option<u64>) {
        if let Some(u) = min_usdt {
            if u > 0 {
                self.min_iceberg_usdt.store(u, Ordering::Relaxed);
            }
        }
        if let Some(r) = min_ratio_x10 {
            if r >= 10 {
                self.min_exec_ratio_x10.store(r, Ordering::Relaxed);
            }
        }
        if let Some(rf) = min_refills {
            self.min_refill_count.store(rf, Ordering::Relaxed);
        }
        if let Some(w) = window_ms {
            if w >= 5000 {
                self.window_ms.store(w, Ordering::Relaxed);
            }
        }
    }

    pub fn process_trade_payload(&self, json_data: &Value, now_ms: u64) {
        let mut trades_guard = self.trade_history.lock().unwrap();
        let mut levels_guard = self.tracked_levels.lock().unwrap();

        if let Some(obj) = json_data.as_object() {
            for (symbol, item) in obj.iter() {
                let trade_id = parse_i64(&item["trade_id"]);
                let price = parse_f64(&item["price"]);
                let quantity = parse_f64(&item["quantity"]);
                let is_buyer_maker = item["buyer_is_maker"].as_bool().unwrap_or(false);
                let event_time = parse_u64(&item["event_time"])
                    .or_else(|| parse_u64(&item["local_recv_time_ms"]))
                    .unwrap_or(now_ms);

                if price > 0.0 && quantity > 0.0 {
                    let usdt_val = price * quantity;
                    let deque = trades_guard.entry(symbol.clone()).or_insert_with(VecDeque::new);

                    if deque.back().map_or(true, |last| last.trade_id != trade_id || trade_id == 0) {
                        deque.push_back(TradeRecord {
                            trade_id,
                            price,
                            quantity,
                            usdt_value: usdt_val,
                            is_buyer_maker,
                            timestamp_ms: event_time,
                        });

                        if deque.len() > 1000 {
                            deque.pop_front();
                        }

                        // Attribute executed trade to tracked price levels around price
                        if let Some(symbol_levels) = levels_guard.get_mut(symbol) {
                            let tolerance = price * 0.0005; // 0.05% band
                            for level in symbol_levels.values_mut() {
                                if (level.price - price).abs() <= tolerance {
                                    level.executed_qty += quantity;
                                    level.executed_usdt += usdt_val;
                                    level.last_updated_time_ms = event_time;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn process_depth_payload(&self, json_data: &Value, now_ms: u64) -> String {
        let min_usdt = self.min_iceberg_usdt.load(Ordering::Relaxed) as f64;
        let min_ratio = self.min_exec_ratio_x10.load(Ordering::Relaxed) as f64 / 10.0;
        let min_refills = self.min_refill_count.load(Ordering::Relaxed) as usize;
        let window_ms = self.window_ms.load(Ordering::Relaxed);

        let mut levels_guard = self.tracked_levels.lock().unwrap();
        let mut events_guard = self.iceberg_events.lock().unwrap();
        let mut metrics_guard = self.symbol_metrics.lock().unwrap();

        if let Some(obj) = json_data.as_object() {
            for (symbol, depth_item) in obj.iter() {
                let event_time = parse_u64(&depth_item["event_time"])
                    .or_else(|| parse_u64(&depth_item["local_recv_time_ms"]))
                    .unwrap_or(now_ms);

                let window_start_ms = event_time.saturating_sub(window_ms);
                let bids_arr = depth_item["bids"].as_array();
                let asks_arr = depth_item["asks"].as_array();

                let symbol_levels = levels_guard.entry(symbol.clone()).or_insert_with(HashMap::new);

                // Prune old level entries past window_ms
                symbol_levels.retain(|_, lvl| lvl.last_updated_time_ms >= window_start_ms);

                // Process Bids
                if let Some(bids) = bids_arr {
                    for item in bids {
                        if let Some((p, q)) = parse_price_qty(item) {
                            let key = format!("BID_{:.6}", p);
                            update_or_insert_level(
                                symbol_levels,
                                key,
                                symbol.clone(),
                                "BID".to_string(),
                                p,
                                q,
                                event_time,
                            );
                        }
                    }
                }

                // Process Asks
                if let Some(asks) = asks_arr {
                    for item in asks {
                        if let Some((p, q)) = parse_price_qty(item) {
                            let key = format!("ASK_{:.6}", p);
                            update_or_insert_level(
                                symbol_levels,
                                key,
                                symbol.clone(),
                                "ASK".to_string(),
                                p,
                                q,
                                event_time,
                            );
                        }
                    }
                }

                // Evaluate tracked levels for Iceberg Patterns
                for level in symbol_levels.values_mut() {
                    let exec_ratio = if level.initial_visible_qty > 0.0 {
                        level.executed_qty / level.initial_visible_qty
                    } else {
                        0.0
                    };

                    let is_qualifying = level.executed_usdt >= min_usdt
                        && exec_ratio >= min_ratio
                        && level.refill_count >= min_refills;

                    if is_qualifying && !level.is_alerted {
                        level.is_alerted = true;

                        let estimated_hidden_qty = (level.executed_qty + level.last_visible_qty - level.initial_visible_qty).max(level.executed_qty);
                        let estimated_hidden_usdt = estimated_hidden_qty * level.price;

                        let iceberg_side = if level.side == "BID" {
                            "BUY_ICEBERG" // Market Sells are being absorbed by a hidden Bid (Accumulation)
                        } else {
                            "SELL_ICEBERG" // Market Buys are being absorbed by a hidden Ask (Distribution)
                        };

                        let iceberg_score = calculate_iceberg_score(exec_ratio, level.executed_usdt, level.refill_count);
                        let alert_level = classify_alert_level(iceberg_score, estimated_hidden_usdt);

                        let event_id = self.event_counter.fetch_add(1, Ordering::Relaxed);
                        let desc = format!(
                            "ICEBERG ORDER: {} executed ${:.0} vs visible ${:.0} at price {:.4} and REFILLED {} times! Estimated Hidden Vol: ${:.0}",
                            iceberg_side,
                            level.executed_usdt,
                            level.initial_visible_usdt,
                            level.price,
                            level.refill_count,
                            estimated_hidden_usdt
                        );

                        let event = IcebergEvent {
                            id: event_id,
                            symbol: symbol.clone(),
                            side: iceberg_side.to_string(),
                            price: level.price,
                            visible_usdt: level.initial_visible_usdt,
                            executed_qty: level.executed_qty,
                            executed_usdt: level.executed_usdt,
                            estimated_hidden_qty,
                            estimated_hidden_usdt,
                            refill_count: level.refill_count,
                            execution_ratio: exec_ratio,
                            iceberg_score,
                            alert_level,
                            event_time_ms: event_time,
                            description: desc,
                        };

                        events_guard.push_back(event);
                        if events_guard.len() > 50 {
                            events_guard.pop_front();
                        }

                        // Update symbol metrics
                        let m = metrics_guard.entry(symbol.clone()).or_insert_with(|| SymbolIcebergMetrics {
                            symbol: symbol.clone(),
                            ..Default::default()
                        });
                        m.total_iceberg_events += 1;
                        if iceberg_side == "BUY_ICEBERG" {
                            m.total_buy_icebergs += 1;
                        } else {
                            m.total_sell_icebergs += 1;
                        }
                        m.total_hidden_usdt_detected += estimated_hidden_usdt;
                        m.last_iceberg_time_ms = event_time;
                    }
                }
            }
        }

        // Update active icebergs count in metrics
        for (symbol, symbol_levels) in levels_guard.iter() {
            let active_count = symbol_levels.values().filter(|l| l.is_alerted).count();
            let m = metrics_guard.entry(symbol.clone()).or_insert_with(|| SymbolIcebergMetrics {
                symbol: symbol.clone(),
                ..Default::default()
            });
            m.active_icebergs_count = active_count;
        }

        // Safely drop all locks before producing the report
        drop(metrics_guard);
        drop(events_guard);
        drop(levels_guard);

        self.get_formatted_report()
    }

    pub fn get_formatted_report(&self) -> String {
        let metrics_guard = self.symbol_metrics.lock().unwrap();
        let events_guard = self.iceberg_events.lock().unwrap();
        let levels_guard = self.tracked_levels.lock().unwrap();

        let min_usdt = self.min_iceberg_usdt.load(Ordering::Relaxed);
        let min_ratio = self.min_exec_ratio_x10.load(Ordering::Relaxed) as f64 / 10.0;
        let window_ms = self.window_ms.load(Ordering::Relaxed);

        let mut sorted_symbols: Vec<String> = metrics_guard.keys().cloned().collect();
        sorted_symbols.sort();

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str(&format!(
            "🧊 BINANCE FUTURES ICEBERG ORDER DETECTION PANEL\n\
             ├─ Min Vol Threshold: ${} USDT | Min Ratio: {:.1}x | Window: {} ms\n",
            min_usdt, min_ratio, window_ms
        ));
        report.push_str("============================================================\n");

        if sorted_symbols.is_empty() {
            report.push_str("No iceberg orders detected yet.\n");
        } else {
            for symbol in &sorted_symbols {
                if let Some(m) = metrics_guard.get(symbol) {
                    let active_count = levels_guard.get(symbol).map(|lvls| lvls.values().filter(|l| l.is_alerted).count()).unwrap_or(0);
                    report.push_str(&format!(
                        "[{}]  Active Hidden Levels: {} | Total Icebergs: {} (Buy/Accumulation: {} | Sell/Distribution: {})\n\
                         ├─ Total Hidden Vol Detected: ${:.2}\n\n",
                        symbol,
                        active_count,
                        m.total_iceberg_events,
                        m.total_buy_icebergs,
                        m.total_sell_icebergs,
                        m.total_hidden_usdt_detected
                    ));
                }
            }
        }

        report.push_str("------------------------------------------------------------\n");
        report.push_str("🧊 RECENT ICEBERG ORDER ALERTS (Last 5 Events):\n");
        report.push_str("------------------------------------------------------------\n");

        if events_guard.is_empty() {
            report.push_str("No institutional hidden iceberg orders detected yet.\n");
        } else {
            for ev in events_guard.iter().rev().take(5) {
                let badge = match ev.alert_level.as_str() {
                    "CRITICAL" => "🛑 [CRITICAL]",
                    "HIGH" => "🔴 [HIGH]",
                    "MEDIUM" => "🟠 [MEDIUM]",
                    _ => "🟡 [INFO]",
                };

                let side_label = if ev.side == "BUY_ICEBERG" {
                    "🟢 BUY/ACCUMULATION (Buy Iceberg)"
                } else {
                    "🔴 SELL/DISTRIBUTION (Sell Iceberg)"
                };

                report.push_str(&format!(
                    "{} #{} [{}] {} Price: {:.8} | Refills: {} Times\n\
                     │   ├─ Visible Vol: ${:.8} -> Executed: ${:.8} (Ratio: {:.1}x)\n\
                     │   ├─ Estimated Total Hidden Vol: ${:.8} | Score: {:.8}\n\
                     │   └─ Description: {}\n",
                    badge,
                    ev.id,
                    ev.symbol,
                    side_label,
                    ev.price,
                    ev.refill_count,
                    ev.visible_usdt,
                    ev.executed_usdt,
                    ev.execution_ratio,
                    ev.estimated_hidden_usdt,
                    ev.iceberg_score,
                    ev.description
                ));
            }
        }

        report.push_str("============================================================\n");
        report
    }

    pub fn get_metrics_json(&self) -> String {
        let metrics_guard = self.symbol_metrics.lock().unwrap();
        let events_guard = self.iceberg_events.lock().unwrap();

        let json_out = serde_json::json!({
            "metrics": *metrics_guard,
            "recent_events": *events_guard,
        });

        serde_json::to_string_pretty(&json_out).unwrap_or_else(|_| "{}".to_string())
    }
}

fn update_or_insert_level(
    symbol_levels: &mut HashMap<String, PriceLevelState>,
    key: String,
    symbol: String,
    side: String,
    price: f64,
    visible_qty: f64,
    now_ms: u64,
) {
    let visible_usdt = price * visible_qty;

    symbol_levels.entry(key).and_modify(|lvl| {
        // Refill condition: Trades executed since last refill AND order book visible_qty is maintained/replenished
        if lvl.executed_qty > lvl.last_refill_exec_qty && visible_qty >= lvl.initial_visible_qty * 0.5 {
            lvl.refill_count += 1;
            lvl.last_refill_exec_qty = lvl.executed_qty;
        }
        lvl.last_visible_qty = visible_qty;
        lvl.last_updated_time_ms = now_ms;
    }).or_insert(PriceLevelState {
        symbol,
        side,
        price,
        initial_visible_qty: visible_qty,
        initial_visible_usdt: visible_usdt,
        last_visible_qty: visible_qty,
        executed_qty: 0.0,
        executed_usdt: 0.0,
        last_refill_exec_qty: 0.0,
        refill_count: 0,
        first_seen_time_ms: now_ms,
        last_updated_time_ms: now_ms,
        is_alerted: false,
    });
}

fn calculate_iceberg_score(exec_ratio: f64, executed_usdt: f64, refill_count: usize) -> f64 {
    let ratio_factor = (exec_ratio / 5.0).clamp(0.2, 1.0);
    let usdt_factor = (executed_usdt / 200_000.0).clamp(0.2, 1.0);
    let refill_factor = (refill_count as f64 / 5.0).clamp(0.2, 1.0);

    (ratio_factor * 0.4 + usdt_factor * 0.3 + refill_factor * 0.3).clamp(0.0, 1.0)
}

fn classify_alert_level(score: f64, estimated_hidden_usdt: f64) -> String {
    if score >= 0.85 || estimated_hidden_usdt >= 500_000.0 {
        "CRITICAL".to_string()
    } else if score >= 0.65 || estimated_hidden_usdt >= 150_000.0 {
        "HIGH".to_string()
    } else if score >= 0.45 {
        "MEDIUM".to_string()
    } else {
        "INFO".to_string()
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

fn parse_price_qty(item: &Value) -> Option<(f64, f64)> {
    if let Some(arr) = item.as_array() {
        if arr.len() >= 2 {
            let p = parse_f64(&arr[0]);
            let q = parse_f64(&arr[1]);
            if p > 0.0 && q > 0.0 {
                return Some((p, q));
            }
        }
    }
    None
}

// C ABI Plugin Integration
struct PluginState {
    is_running: Arc<AtomicBool>,
    engine: Arc<IcebergEngine>,
    data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let engine = Arc::new(IcebergEngine::new());
    let initial_report = engine.get_formatted_report();

    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        engine,
        data: Arc::new(Mutex::new(initial_report.into_bytes())),
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
                    let min_usdt = params.get("min_iceberg_usdt").and_then(|v| v.as_u64());
                    let min_ratio = params.get("min_exec_ratio_x10").and_then(|v| v.as_u64());
                    let min_refills = params.get("min_refill_count").and_then(|v| v.as_u64());
                    let window_ms = params.get("window_ms").and_then(|v| v.as_u64());

                    state.engine.configure(min_usdt, min_ratio, min_refills, window_ms);
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

                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                if stream_id.contains("trades") || stream_id.contains("aggtrades") {
                    if let Ok(json_data) = serde_json::from_slice::<Value>(&slice[32..]) {
                        state.engine.process_trade_payload(&json_data, now_ms);
                    }
                } else if stream_id.contains("depth") {
                    if let Ok(json_data) = serde_json::from_slice::<Value>(&slice[32..]) {
                        let report = state.engine.process_depth_payload(&json_data, now_ms);

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
    fn test_iceberg_detection_simulation() {
        let engine = IcebergEngine::new();
        engine.configure(Some(30000), Some(20), Some(2), Some(60000));

        let t0 = 1000000u64;

        // Step 1: Initial Depth Snapshot -> Bid at 100.0 has visible size $10,000 (100 BTC)
        let depth_snap1 = serde_json::json!({
            "BTCUSDT": {
                "bids": [
                    ["100.0", "100.0"] // $10,000 USDT visible
                ],
                "asks": [
                    ["100.5", "10.0"]
                ],
                "event_time": t0
            }
        });
        engine.process_depth_payload(&depth_snap1, t0);

        // Step 2: Trades hit 100.0 -> Market Sells hit $20,000 total executed
        let trade_data1 = serde_json::json!({
            "BTCUSDT": {
                "trade_id": 201,
                "price": "100.0",
                "quantity": "200.0", // $20,000 executed
                "buyer_is_maker": true,
                "event_time": t0 + 100
            }
        });
        engine.process_trade_payload(&trade_data1, t0 + 100);

        // Step 3: Depth Snapshot 2 -> Bid at 100.0 is REFILLED back up to 100.0 BTC ($10,000 USDT visible)!
        let depth_snap2 = serde_json::json!({
            "BTCUSDT": {
                "bids": [
                    ["100.0", "100.0"] // Refilled!
                ],
                "asks": [
                    ["100.5", "10.0"]
                ],
                "event_time": t0 + 200
            }
        });
        engine.process_depth_payload(&depth_snap2, t0 + 200);

        // Step 4: Trades hit 100.0 again -> Another $20,000 executed (Total Executed = $40,000 >= $30,000 min)
        let trade_data2 = serde_json::json!({
            "BTCUSDT": {
                "trade_id": 202,
                "price": "100.0",
                "quantity": "200.0", // Another $20,000 executed => Total $40,000
                "buyer_is_maker": true,
                "event_time": t0 + 300
            }
        });
        engine.process_trade_payload(&trade_data2, t0 + 300);

        // Step 5: Depth Snapshot 3 -> Bid at 100.0 is REFILLED AGAIN (refill_count = 2)
        let depth_snap3 = serde_json::json!({
            "BTCUSDT": {
                "bids": [
                    ["100.0", "100.0"] // Refilled second time!
                ],
                "asks": [
                    ["100.5", "10.0"]
                ],
                "event_time": t0 + 400
            }
        });

        let report = engine.process_depth_payload(&depth_snap3, t0 + 400);

        assert!(report.contains("BINANCE FUTURES ICEBERG ORDER DETECTION PANEL"));
        assert!(report.contains("BTCUSDT"));

        // Check metrics & events
        let events = engine.iceberg_events.lock().unwrap();
        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.symbol, "BTCUSDT");
        assert_eq!(event.side, "BUY_ICEBERG");
        assert_eq!(event.price, 100.0);
        assert_eq!(event.visible_usdt, 10000.0);
        assert_eq!(event.executed_usdt, 40000.0);
        assert_eq!(event.execution_ratio, 4.0); // 4x visible size
        assert_eq!(event.refill_count, 2);
        assert!(event.estimated_hidden_usdt >= 40000.0);
    }
}
