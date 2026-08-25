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
pub struct ActiveWall {
    pub symbol: String,
    pub side: String, // "BID" or "ASK"
    pub price: f64,
    pub initial_qty: f64,
    pub initial_usdt: f64,
    pub current_qty: f64,
    pub current_usdt: f64,
    pub appear_time_ms: u64,
    pub last_seen_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpoofEvent {
    pub id: u64,
    pub symbol: String,
    pub side: String, // "BID" (Bullish/Long Bait) or "ASK" (Bearish/Short Bait)
    pub price: f64,
    pub initial_usdt: f64,
    pub canceled_qty: f64,
    pub canceled_usdt: f64,
    pub executed_qty: f64,
    pub executed_usdt: f64,
    pub lifespan_ms: u64,
    pub cancel_ratio: f64,
    pub spoof_score: f64,
    pub alert_level: String, // "INFO", "MEDIUM", "HIGH", "CRITICAL"
    pub event_time_ms: u64,
    pub description: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SymbolSpoofMetrics {
    pub symbol: String,
    pub active_walls_count: usize,
    pub total_spoof_events: usize,
    pub total_bid_spoofs: usize,
    pub total_ask_spoofs: usize,
    pub total_phantom_usdt: f64,
    pub last_spoof_time_ms: u64,
}

pub struct SpoofDetectorEngine {
    pub min_wall_usdt: AtomicU64,        // Min USDT value for a wall (e.g. 50,000)
    pub max_lifespan_ms: AtomicU64,      // Max wall lifespan to count as spoofing (e.g. 15,000 ms)
    pub min_cancel_ratio_pct: AtomicU64, // Min canceled % without fill (e.g. 70%)

    // active_walls: symbol -> price_key -> ActiveWall
    pub active_walls: Mutex<HashMap<String, HashMap<String, ActiveWall>>>,
    pub trade_history: Mutex<HashMap<String, VecDeque<TradeRecord>>>,
    pub spoof_events: Mutex<VecDeque<SpoofEvent>>,
    pub symbol_metrics: Mutex<HashMap<String, SymbolSpoofMetrics>>,
    pub event_counter: AtomicU64,
}

impl SpoofDetectorEngine {
    pub fn new() -> Self {
        Self {
            min_wall_usdt: AtomicU64::new(50000),      // $50,000 default threshold
            max_lifespan_ms: AtomicU64::new(15000),    // 15 seconds
            min_cancel_ratio_pct: AtomicU64::new(70),  // 70% canceled
            active_walls: Mutex::new(HashMap::new()),
            trade_history: Mutex::new(HashMap::new()),
            spoof_events: Mutex::new(VecDeque::new()),
            symbol_metrics: Mutex::new(HashMap::new()),
            event_counter: AtomicU64::new(1),
        }
    }

    pub fn configure(&self, min_usdt: Option<u64>, max_lifespan: Option<u64>, min_cancel_pct: Option<u64>) {
        if let Some(u) = min_usdt {
            if u > 0 {
                self.min_wall_usdt.store(u, Ordering::Relaxed);
            }
        }
        if let Some(l) = max_lifespan {
            if l >= 500 {
                self.max_lifespan_ms.store(l, Ordering::Relaxed);
            }
        }
        if let Some(c) = min_cancel_pct {
            if c <= 100 {
                self.min_cancel_ratio_pct.store(c, Ordering::Relaxed);
            }
        }
    }

    pub fn process_trade_payload(&self, json_data: &Value, now_ms: u64) {
        let mut trades_guard = self.trade_history.lock().unwrap();

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

                    // Prevent duplicates
                    if deque.back().map_or(true, |last| last.trade_id != trade_id || trade_id == 0) {
                        deque.push_back(TradeRecord {
                            trade_id,
                            price,
                            quantity,
                            usdt_value: usdt_val,
                            is_buyer_maker,
                            timestamp_ms: event_time,
                        });

                        // Keep up to 1,000 trades per symbol
                        if deque.len() > 1000 {
                            deque.pop_front();
                        }
                    }
                }
            }
        }
    }

    pub fn process_depth_payload(&self, json_data: &Value, now_ms: u64) -> String {
        let min_usdt = self.min_wall_usdt.load(Ordering::Relaxed) as f64;
        let max_lifespan = self.max_lifespan_ms.load(Ordering::Relaxed);
        let min_cancel_pct = self.min_cancel_ratio_pct.load(Ordering::Relaxed) as f64 / 100.0;

        let mut walls_guard = self.active_walls.lock().unwrap();
        let trades_guard = self.trade_history.lock().unwrap();
        let mut events_guard = self.spoof_events.lock().unwrap();
        let mut metrics_guard = self.symbol_metrics.lock().unwrap();

        if let Some(obj) = json_data.as_object() {
            for (symbol, depth_item) in obj.iter() {
                let event_time = parse_u64(&depth_item["event_time"])
                    .or_else(|| parse_u64(&depth_item["local_recv_time_ms"]))
                    .unwrap_or(now_ms);

                let bids_arr = depth_item["bids"].as_array();
                let asks_arr = depth_item["asks"].as_array();

                let mut current_snapshot_keys = std::collections::HashSet::new();

                // Process Bids (Alım Duvarları)
                if let Some(bids) = bids_arr {
                    let avg_qty = compute_avg_qty(bids);
                    for item in bids {
                        if let Some((p, q)) = parse_price_qty(item) {
                            let usdt_val = p * q;
                            let price_key = format!("BID_{:.6}", p);
                            // Wall condition: exceeds min_usdt and is at least 2.5x avg quantity
                            if usdt_val >= min_usdt || (avg_qty > 0.0 && q >= avg_qty * 2.5) {
                                current_snapshot_keys.insert(price_key.clone());
                                let symbol_walls = walls_guard.entry(symbol.clone()).or_insert_with(HashMap::new);

                                symbol_walls.entry(price_key).and_modify(|w| {
                                    w.current_qty = q;
                                    w.current_usdt = usdt_val;
                                    w.last_seen_time_ms = event_time;
                                }).or_insert(ActiveWall {
                                    symbol: symbol.clone(),
                                    side: "BID".to_string(),
                                    price: p,
                                    initial_qty: q,
                                    initial_usdt: usdt_val,
                                    current_qty: q,
                                    current_usdt: usdt_val,
                                    appear_time_ms: event_time,
                                    last_seen_time_ms: event_time,
                                });
                            }
                        }
                    }
                }

                // Process Asks (Satış Duvarları)
                if let Some(asks) = asks_arr {
                    let avg_qty = compute_avg_qty(asks);
                    for item in asks {
                        if let Some((p, q)) = parse_price_qty(item) {
                            let usdt_val = p * q;
                            let price_key = format!("ASK_{:.6}", p);
                            if usdt_val >= min_usdt || (avg_qty > 0.0 && q >= avg_qty * 2.5) {
                                current_snapshot_keys.insert(price_key.clone());
                                let symbol_walls = walls_guard.entry(symbol.clone()).or_insert_with(HashMap::new);

                                symbol_walls.entry(price_key).and_modify(|w| {
                                    w.current_qty = q;
                                    w.current_usdt = usdt_val;
                                    w.last_seen_time_ms = event_time;
                                }).or_insert(ActiveWall {
                                    symbol: symbol.clone(),
                                    side: "ASK".to_string(),
                                    price: p,
                                    initial_qty: q,
                                    initial_usdt: usdt_val,
                                    current_qty: q,
                                    current_usdt: usdt_val,
                                    appear_time_ms: event_time,
                                    last_seen_time_ms: event_time,
                                });
                            }
                        }
                    }
                }

                // Inspect missing or significantly reduced walls for Spoofing Detection
                if let Some(symbol_walls) = walls_guard.get_mut(symbol) {
                    let mut evaluated_keys = Vec::new();

                    for (key, wall) in symbol_walls.iter() {
                        let is_missing = !current_snapshot_keys.contains(key);
                        let qty_dropped = wall.current_qty <= wall.initial_qty * 0.30; // Dropped by 70%+

                        if is_missing || qty_dropped {
                            evaluated_keys.push(key.clone());

                            let effective_current_qty = if is_missing { 0.0 } else { wall.current_qty };
                            let lifespan = event_time.saturating_sub(wall.appear_time_ms);
                            if lifespan <= max_lifespan && lifespan >= 100 {
                                // Calculate executed volume around this price level during wall lifespan
                                let executed_qty = calculate_executed_qty_at_price(
                                    trades_guard.get(symbol),
                                    wall.price,
                                    wall.appear_time_ms,
                                    event_time,
                                );

                                let raw_canceled = (wall.initial_qty - effective_current_qty) - executed_qty;
                                let canceled_qty = raw_canceled.max(0.0);
                                let cancel_ratio = if wall.initial_qty > 0.0 {
                                    canceled_qty / wall.initial_qty
                                } else {
                                    0.0
                                };

                                if cancel_ratio >= min_cancel_pct {
                                    let canceled_usdt = canceled_qty * wall.price;
                                    let executed_usdt = executed_qty * wall.price;

                                    let spoof_score = calculate_spoof_score(
                                        wall.initial_usdt,
                                        lifespan,
                                        cancel_ratio,
                                    );
                                    let alert_level = classify_alert_level(spoof_score, wall.initial_usdt, lifespan);

                                    let event_id = self.event_counter.fetch_add(1, Ordering::Relaxed);
                                    let desc = format!(
                                        "SPOOF ORDER (Fake Wall): {} wall at price {:.4} with ${:.0} size was CANCELED within {} ms before {:.1}% was filled!",
                                        wall.side,
                                        wall.price,
                                        wall.initial_usdt,
                                        lifespan,
                                        cancel_ratio * 100.0
                                    );

                                    let spoof_event = SpoofEvent {
                                        id: event_id,
                                        symbol: symbol.clone(),
                                        side: wall.side.clone(),
                                        price: wall.price,
                                        initial_usdt: wall.initial_usdt,
                                        canceled_qty,
                                        canceled_usdt,
                                        executed_qty,
                                        executed_usdt,
                                        lifespan_ms: lifespan,
                                        cancel_ratio,
                                        spoof_score,
                                        alert_level,
                                        event_time_ms: event_time,
                                        description: desc,
                                    };

                                    events_guard.push_back(spoof_event);
                                    if events_guard.len() > 50 {
                                        events_guard.pop_front();
                                    }

                                    // Update metrics
                                    let m = metrics_guard.entry(symbol.clone()).or_insert_with(|| SymbolSpoofMetrics {
                                        symbol: symbol.clone(),
                                        ..Default::default()
                                    });
                                    m.total_spoof_events += 1;
                                    if wall.side == "BID" {
                                        m.total_bid_spoofs += 1;
                                    } else {
                                        m.total_ask_spoofs += 1;
                                    }
                                    m.total_phantom_usdt += canceled_usdt;
                                    m.last_spoof_time_ms = event_time;
                                }
                            }
                        }
                    }

                    // Remove processed walls
                    for k in evaluated_keys {
                        symbol_walls.remove(&k);
                    }
                }
            }
        }

        // Update active walls count in metrics
        for (symbol, symbol_walls) in walls_guard.iter() {
            let m = metrics_guard.entry(symbol.clone()).or_insert_with(|| SymbolSpoofMetrics {
                symbol: symbol.clone(),
                ..Default::default()
            });
            m.active_walls_count = symbol_walls.len();
        }

        drop(metrics_guard);
        drop(events_guard);
        drop(trades_guard);
        drop(walls_guard);

        self.get_formatted_report()
    }

    pub fn get_formatted_report(&self) -> String {
        let metrics_guard = self.symbol_metrics.lock().unwrap();
        let events_guard = self.spoof_events.lock().unwrap();
        let walls_guard = self.active_walls.lock().unwrap();

        let min_usdt = self.min_wall_usdt.load(Ordering::Relaxed);
        let max_lifespan = self.max_lifespan_ms.load(Ordering::Relaxed);

        let mut sorted_symbols: Vec<String> = metrics_guard.keys().cloned().collect();
        sorted_symbols.sort();

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str(&format!(
            "🕵️ BINANCE FUTURES SPOOFING (FAKE ORDER) DETECTION PANEL\n\
             ├─ Wall Threshold: ${} USDT | Max Lifespan: {} ms\n",
            min_usdt, max_lifespan
        ));
        report.push_str("============================================================\n");

        if sorted_symbols.is_empty() {
            report.push_str("No spoofing (fake orders) or active walls detected yet.\n");
        } else {
            for symbol in &sorted_symbols {
                if let Some(m) = metrics_guard.get(symbol) {
                    let wall_count = walls_guard.get(symbol).map(|w| w.len()).unwrap_or(0);
                    report.push_str(&format!(
                        "[{}]  Active Walls: {} | Total Spoof Events: {} (Buy/Long Bait: {} | Sell/Short Bait: {})\n\
                         ├─ Total Phantom/Pulled Liquidity: ${:.2}\n\n",
                        symbol,
                        wall_count,
                        m.total_spoof_events,
                        m.total_bid_spoofs,
                        m.total_ask_spoofs,
                        m.total_phantom_usdt
                    ));
                }
            }
        }

        report.push_str("------------------------------------------------------------\n");
        report.push_str("🚨 RECENT SPOOFING ALERTS (Last 5 Events):\n");
        report.push_str("------------------------------------------------------------\n");

        if events_guard.is_empty() {
            report.push_str("No suspicious spoof order cancellations detected yet.\n");
        } else {
            for ev in events_guard.iter().rev().take(5) {
                let badge = match ev.alert_level.as_str() {
                    "CRITICAL" => "🛑 [CRITICAL]",
                    "HIGH" => "🔴 [HIGH]",
                    "MEDIUM" => "🟠 [MEDIUM]",
                    _ => "🟡 [INFO]",
                };

                report.push_str(&format!(
                    "{} #{} [{}] {} Price: {:.4} | Fake Vol: ${:.0} (Canceled: {:.1}%)\n\
                     │   ├─ Lifespan: {} ms | Executed: ${:.0} | Score: {:.2}\n\
                     │   └─ Description: {}\n",
                    badge,
                    ev.id,
                    ev.symbol,
                    ev.side,
                    ev.price,
                    ev.initial_usdt,
                    ev.cancel_ratio * 100.0,
                    ev.lifespan_ms,
                    ev.executed_usdt,
                    ev.spoof_score,
                    ev.description
                ));
            }
        }

        report.push_str("============================================================\n");
        report
    }

    pub fn get_metrics_json(&self) -> String {
        let metrics_guard = self.symbol_metrics.lock().unwrap();
        let events_guard = self.spoof_events.lock().unwrap();

        let json_out = serde_json::json!({
            "metrics": *metrics_guard,
            "recent_events": *events_guard,
        });

        serde_json::to_string_pretty(&json_out).unwrap_or_else(|_| "{}".to_string())
    }
}

fn compute_avg_qty(levels: &[Value]) -> f64 {
    if levels.is_empty() {
        return 0.0;
    }
    let mut total = 0.0;
    let mut count = 0;
    for item in levels {
        if let Some((_, q)) = parse_price_qty(item) {
            total += q;
            count += 1;
        }
    }
    if count > 0 { total / count as f64 } else { 0.0 }
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

fn calculate_executed_qty_at_price(
    trades: Option<&VecDeque<TradeRecord>>,
    price: f64,
    start_ms: u64,
    end_ms: u64,
) -> f64 {
    let mut total_qty = 0.0;
    if let Some(deque) = trades {
        let tolerance = price * 0.0005; // 0.05% price band tolerance for market order match
        for t in deque.iter().rev() {
            if t.timestamp_ms < start_ms {
                break;
            }
            if t.timestamp_ms <= end_ms && (t.price - price).abs() <= tolerance {
                total_qty += t.quantity;
            }
        }
    }
    total_qty
}

fn calculate_spoof_score(initial_usdt: f64, lifespan_ms: u64, cancel_ratio: f64) -> f64 {
    // Size score: $50k = 0.5, $500k+ = 1.0
    let size_factor = (initial_usdt / 250_000.0).clamp(0.2, 1.0);
    // Speed score: < 1000 ms = 1.0, 15000 ms = 0.2
    let speed_factor = (1.0 - (lifespan_ms as f64 / 15_000.0)).clamp(0.2, 1.0);
    // Cancel score: cancel_ratio
    let cancel_factor = cancel_ratio.clamp(0.0, 1.0);

    (size_factor * 0.4 + speed_factor * 0.3 + cancel_factor * 0.3).clamp(0.0, 1.0)
}

fn classify_alert_level(score: f64, initial_usdt: f64, lifespan_ms: u64) -> String {
    if score >= 0.85 || (initial_usdt >= 500_000.0 && lifespan_ms <= 2000) {
        "CRITICAL".to_string()
    } else if score >= 0.65 || (initial_usdt >= 200_000.0 && lifespan_ms <= 5000) {
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

// C ABI Plugin Integration
struct PluginState {
    is_running: Arc<AtomicBool>,
    engine: Arc<SpoofDetectorEngine>,
    data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let engine = Arc::new(SpoofDetectorEngine::new());
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
                    let min_usdt = params.get("min_wall_usdt").and_then(|v| v.as_u64());
                    let max_lifespan = params.get("max_lifespan_ms").and_then(|v| v.as_u64());
                    let min_cancel = params.get("min_cancel_ratio_pct").and_then(|v| v.as_u64());

                    state.engine.configure(min_usdt, max_lifespan, min_cancel);
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
    fn test_spoofing_detection_simulation() {
        let engine = SpoofDetectorEngine::new();
        engine.configure(Some(50000), Some(10000), Some(70));

        let t0 = 1000000u64;
        let t1 = 1001500u64; // 1500 ms later

        // Step 1: Ingest depth snapshot with a massive BID wall ($200,000 at price 100.0)
        let depth_snap1 = serde_json::json!({
            "BTCUSDT": {
                "bids": [
                    ["100.0", "2000.0"], // $200,000 USDT wall
                    ["99.5", "10.0"]
                ],
                "asks": [
                    ["100.5", "10.0"]
                ],
                "event_time": t0
            }
        });

        engine.process_depth_payload(&depth_snap1, t0);

        // Verify active wall was registered
        {
            let walls = engine.active_walls.lock().unwrap();
            let btc_walls = walls.get("BTCUSDT").unwrap();
            assert_eq!(btc_walls.len(), 1);
            assert!(btc_walls.contains_key("BID_100.000000"));
        }

        // Step 2: Ingest a small trade at 100.0 ($5,000 filled)
        let trade_data = serde_json::json!({
            "BTCUSDT": {
                "trade_id": 101,
                "price": "100.0",
                "quantity": "50.0", // $5,000 executed
                "buyer_is_maker": true,
                "event_time": t0 + 500
            }
        });
        engine.process_trade_payload(&trade_data, t0 + 500);

        // Step 3: Ingest depth snapshot where the $200,000 BID wall completely disappears (canceled without fill)
        let depth_snap2 = serde_json::json!({
            "BTCUSDT": {
                "bids": [
                    ["99.5", "10.0"]
                ],
                "asks": [
                    ["100.5", "10.0"]
                ],
                "event_time": t1
            }
        });

        let report = engine.process_depth_payload(&depth_snap2, t1);

        assert!(report.contains("BINANCE FUTURES SPOOFING (FAKE ORDER) DETECTION PANEL"));
        assert!(report.contains("BTCUSDT"));

        // Check metrics & events
        let events = engine.spoof_events.lock().unwrap();
        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.symbol, "BTCUSDT");
        assert_eq!(event.side, "BID");
        assert_eq!(event.price, 100.0);
        assert_eq!(event.initial_usdt, 200000.0);
        assert_eq!(event.executed_qty, 50.0);
        assert_eq!(event.canceled_qty, 1950.0);
        assert!((event.cancel_ratio - 0.975).abs() < 1e-5);
        assert_eq!(event.lifespan_ms, 1500);
        assert!(event.spoof_score > 0.6);
        assert!(event.alert_level == "HIGH" || event.alert_level == "CRITICAL");
    }
}
