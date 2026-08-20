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
pub struct WallState {
    pub symbol: String,
    pub side: String, // "BID" (Alım) or "ASK" (Satış)
    pub price: f64,
    pub initial_qty: f64,
    pub initial_usdt: f64,
    pub current_qty: f64,
    pub current_usdt: f64,
    pub executed_qty: f64,
    pub executed_usdt: f64,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub wall_status: String, // "TESTING", "CONFIRMED_GENUINE", "BROKEN", "SPOOFED"
    pub is_alerted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsorptionEvent {
    pub id: u64,
    pub symbol: String,
    pub side: String, // "BID_ABSORPTION" (Boğa Emilimi/Destek Teyidi) or "ASK_ABSORPTION" (Ayı Emilimi/Direnç Teyidi)
    pub price: f64,
    pub absorbed_qty: f64,
    pub absorbed_usdt: f64,
    pub wall_status: String, // "CONFIRMED_GENUINE", "BROKEN", "SPOOFED", "TESTING"
    pub price_change_pct: f64,
    pub absorption_score: f64, // 0.0 to 1.0
    pub alert_level: String, // "INFO", "MEDIUM", "HIGH", "CRITICAL"
    pub event_time_ms: u64,
    pub description: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SymbolAbsorptionMetrics {
    pub symbol: String,
    pub active_walls_count: usize,
    pub confirmed_walls_count: usize,
    pub total_absorption_events: usize,
    pub total_bid_absorptions: usize,
    pub total_ask_absorptions: usize,
    pub total_absorbed_usdt: f64,
    pub last_event_time_ms: u64,
}

pub struct AbsorptionEngine {
    pub min_absorption_usdt: AtomicU64,   // Min executed USDT absorbed (e.g. $50,000)
    pub min_wall_usdt: AtomicU64,         // Min wall size USDT to track (e.g. $40,000)
    pub max_price_move_x1000: AtomicU64,  // Max price change % x1000 during absorption (e.g. 50 = 0.05%)
    pub window_ms: AtomicU64,             // Sliding tracking window (e.g. 60,000 ms)

    pub tracked_walls: Mutex<HashMap<String, HashMap<String, WallState>>>,
    pub trade_history: Mutex<HashMap<String, VecDeque<TradeRecord>>>,
    pub absorption_events: Mutex<VecDeque<AbsorptionEvent>>,
    pub symbol_metrics: Mutex<HashMap<String, SymbolAbsorptionMetrics>>,
    pub event_counter: AtomicU64,
}

impl AbsorptionEngine {
    pub fn new() -> Self {
        Self {
            min_absorption_usdt: AtomicU64::new(50000),   // $50,000 USDT default
            min_wall_usdt: AtomicU64::new(40000),         // $40,000 USDT wall size
            max_price_move_x1000: AtomicU64::new(50),     // 0.05% max price move
            window_ms: AtomicU64::new(60000),             // 60 seconds
            tracked_walls: Mutex::new(HashMap::new()),
            trade_history: Mutex::new(HashMap::new()),
            absorption_events: Mutex::new(VecDeque::new()),
            symbol_metrics: Mutex::new(HashMap::new()),
            event_counter: AtomicU64::new(1),
        }
    }

    pub fn configure(&self, min_abs_usdt: Option<u64>, min_wall: Option<u64>, max_move_x1000: Option<u64>, window_ms: Option<u64>) {
        if let Some(u) = min_abs_usdt {
            if u > 0 {
                self.min_absorption_usdt.store(u, Ordering::Relaxed);
            }
        }
        if let Some(w) = min_wall {
            if w > 0 {
                self.min_wall_usdt.store(w, Ordering::Relaxed);
            }
        }
        if let Some(m) = max_move_x1000 {
            self.max_price_move_x1000.store(m, Ordering::Relaxed);
        }
        if let Some(win) = window_ms {
            if win >= 5000 {
                self.window_ms.store(win, Ordering::Relaxed);
            }
        }
    }

    pub fn process_trade_payload(&self, json_data: &Value, now_ms: u64) {
        let mut trades_guard = self.trade_history.lock().unwrap();
        let mut walls_guard = self.tracked_walls.lock().unwrap();

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

                        // Attribute executed trade to matching wall price levels
                        if let Some(symbol_walls) = walls_guard.get_mut(symbol) {
                            let tolerance = price * 0.0005; // 0.05% band
                            for wall in symbol_walls.values_mut() {
                                if (wall.price - price).abs() <= tolerance {
                                    wall.executed_qty += quantity;
                                    wall.executed_usdt += usdt_val;
                                    wall.last_seen_ms = event_time;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn process_depth_payload(&self, json_data: &Value, now_ms: u64) -> String {
        let min_abs_usdt = self.min_absorption_usdt.load(Ordering::Relaxed) as f64;
        let min_wall_usdt = self.min_wall_usdt.load(Ordering::Relaxed) as f64;
        let max_move_pct = self.max_price_move_x1000.load(Ordering::Relaxed) as f64 / 1000.0;
        let window_ms = self.window_ms.load(Ordering::Relaxed);

        let mut walls_guard = self.tracked_walls.lock().unwrap();
        let mut events_guard = self.absorption_events.lock().unwrap();
        let mut metrics_guard = self.symbol_metrics.lock().unwrap();

        if let Some(obj) = json_data.as_object() {
            for (symbol, depth_item) in obj.iter() {
                let event_time = parse_u64(&depth_item["event_time"])
                    .or_else(|| parse_u64(&depth_item["local_recv_time_ms"]))
                    .unwrap_or(now_ms);

                let window_start_ms = event_time.saturating_sub(window_ms);
                let bids_arr = depth_item["bids"].as_array();
                let asks_arr = depth_item["asks"].as_array();

                let mut current_snapshot_keys = std::collections::HashSet::new();

                let symbol_walls = walls_guard.entry(symbol.clone()).or_insert_with(HashMap::new);

                // Prune old tracked walls
                symbol_walls.retain(|_, w| w.last_seen_ms >= window_start_ms);

                // Ingest Bids
                if let Some(bids) = bids_arr {
                    for item in bids {
                        if let Some((p, q)) = parse_price_qty(item) {
                            let usdt_val = p * q;
                            let key = format!("BID_{:.6}", p);
                            if usdt_val >= min_wall_usdt {
                                current_snapshot_keys.insert(key.clone());
                                update_or_insert_wall(
                                    symbol_walls,
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
                }

                // Ingest Asks
                if let Some(asks) = asks_arr {
                    for item in asks {
                        if let Some((p, q)) = parse_price_qty(item) {
                            let usdt_val = p * q;
                            let key = format!("ASK_{:.6}", p);
                            if usdt_val >= min_wall_usdt {
                                current_snapshot_keys.insert(key.clone());
                                update_or_insert_wall(
                                    symbol_walls,
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
                }

                // Evaluate Absorption & Wall Confirmation
                let mut evaluated_keys = Vec::new();

                for (key, wall) in symbol_walls.iter_mut() {
                    let is_present = current_snapshot_keys.contains(key);

                    // Confirmation Condition: Heavy trade volume executed against the wall (e.g. $50,000+)
                    if wall.executed_usdt >= min_abs_usdt {
                        if is_present && !wall.is_alerted {
                            // Wall survived market attacks and absorbed volume -> CONFIRMED GENUINE WALL
                            wall.wall_status = "CONFIRMED_GENUINE".to_string();
                            wall.is_alerted = true;

                            let side = if wall.side == "BID" {
                                "BID_ABSORPTION" // Market Sells absorbed by Bid (Bullish Support confirmed)
                            } else {
                                "ASK_ABSORPTION" // Market Buys absorbed by Ask (Bearish Resistance confirmed)
                            };

                            let score = calculate_absorption_score(wall.executed_usdt, wall.initial_usdt);
                            let alert_level = classify_alert_level(score, wall.executed_usdt);

                            let event_id = self.event_counter.fetch_add(1, Ordering::Relaxed);
                            let desc = format!(
                                "GENUINE WALL ABSORPTION: Aggressive trades hit the wall at price {:.4} with ${:.0} volume, but the wall WAS NOT BROKEN and was absorbed! ({})",
                                wall.price,
                                wall.executed_usdt,
                                side
                            );

                            let event = AbsorptionEvent {
                                id: event_id,
                                symbol: symbol.clone(),
                                side: side.to_string(),
                                price: wall.price,
                                absorbed_qty: wall.executed_qty,
                                absorbed_usdt: wall.executed_usdt,
                                wall_status: "CONFIRMED_GENUINE".to_string(),
                                price_change_pct: max_move_pct,
                                absorption_score: score,
                                alert_level,
                                event_time_ms: event_time,
                                description: desc,
                            };

                            events_guard.push_back(event);
                            if events_guard.len() > 50 {
                                events_guard.pop_front();
                            }

                            let m = metrics_guard.entry(symbol.clone()).or_insert_with(|| SymbolAbsorptionMetrics {
                                symbol: symbol.clone(),
                                ..Default::default()
                            });
                            m.total_absorption_events += 1;
                            m.confirmed_walls_count += 1;
                            if side == "BID_ABSORPTION" {
                                m.total_bid_absorptions += 1;
                            } else {
                                m.total_ask_absorptions += 1;
                            }
                            m.total_absorbed_usdt += wall.executed_usdt;
                            m.last_event_time_ms = event_time;
                        }
                    }

                    // Handles walls that disappeared/vanished
                    if !is_present {
                        evaluated_keys.push(key.clone());
                        if wall.executed_qty >= wall.initial_qty * 0.8 {
                            wall.wall_status = "BROKEN".to_string(); // Wall was eaten / broken by market orders
                        } else {
                            wall.wall_status = "SPOOFED".to_string(); // Wall was pulled before being executed
                        }
                    }
                }

                // Remove vanished walls
                for k in evaluated_keys {
                    symbol_walls.remove(&k);
                }
            }
        }

        // Update active walls metrics
        for (symbol, symbol_walls) in walls_guard.iter() {
            let active_count = symbol_walls.len();
            let confirmed_count = symbol_walls.values().filter(|w| w.wall_status == "CONFIRMED_GENUINE").count();
            let m = metrics_guard.entry(symbol.clone()).or_insert_with(|| SymbolAbsorptionMetrics {
                symbol: symbol.clone(),
                ..Default::default()
            });
            m.active_walls_count = active_count;
            m.confirmed_walls_count = confirmed_count;
        }

        // Safely drop all mutex locks before formatting report
        drop(metrics_guard);
        drop(events_guard);
        drop(walls_guard);

        self.get_formatted_report()
    }

    pub fn get_formatted_report(&self) -> String {
        let metrics_guard = self.symbol_metrics.lock().unwrap();
        let events_guard = self.absorption_events.lock().unwrap();
        let walls_guard = self.tracked_walls.lock().unwrap();

        let min_abs_usdt = self.min_absorption_usdt.load(Ordering::Relaxed);
        let min_wall_usdt = self.min_wall_usdt.load(Ordering::Relaxed);

        let mut sorted_symbols: Vec<String> = metrics_guard.keys().cloned().collect();
        sorted_symbols.sort();

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str(&format!(
            "🛡️ BINANCE FUTURES ABSORPTION & WALL CONFIRMATION PANEL\n\
             ├─ Min Absorption Vol: ${} USDT | Min Wall Size: ${} USDT\n",
            min_abs_usdt, min_wall_usdt
        ));
        report.push_str("============================================================\n");

        if sorted_symbols.is_empty() {
            report.push_str("No absorption or confirmed genuine walls detected yet.\n");
        } else {
            for symbol in &sorted_symbols {
                if let Some(m) = metrics_guard.get(symbol) {
                    let active_count = walls_guard.get(symbol).map(|w| w.len()).unwrap_or(0);
                    report.push_str(&format!(
                        "[{}]  Tracked Walls: {} (Confirmed Genuine: {}) | Total Absorption: {} (Bullish: {} | Bearish: {})\n\
                         ├─ Total Absorbed Vol: ${:.2}\n\n",
                        symbol,
                        active_count,
                        m.confirmed_walls_count,
                        m.total_absorption_events,
                        m.total_bid_absorptions,
                        m.total_ask_absorptions,
                        m.total_absorbed_usdt
                    ));
                }
            }
        }

        report.push_str("------------------------------------------------------------\n");
        report.push_str("🛡️ RECENT CONFIRMED ABSORPTION & WALL ALERTS (Last 5 Events):\n");
        report.push_str("------------------------------------------------------------\n");

        if events_guard.is_empty() {
            report.push_str("No confirmed wall absorption events recorded yet.\n");
        } else {
            for ev in events_guard.iter().rev().take(5) {
                let badge = match ev.alert_level.as_str() {
                    "CRITICAL" => "🛑 [CRITICAL]",
                    "HIGH" => "🔴 [HIGH]",
                    "MEDIUM" => "🟠 [MEDIUM]",
                    _ => "🟡 [INFO]",
                };

                let side_label = if ev.side == "BID_ABSORPTION" {
                    "🟢 BULLISH ABSORPTION (Buy Support Confirmed)"
                } else {
                    "🔴 BEARISH ABSORPTION (Sell Resistance Confirmed)"
                };

                report.push_str(&format!(
                    "{} #{} [{}] {} Price: {:.4}\n\
                     │   ├─ Absorbed Aggressive Vol: ${:.0} | Status: {}\n\
                     │   ├─ Absorption Score: {:.2}\n\
                     │   └─ Description: {}\n",
                    badge,
                    ev.id,
                    ev.symbol,
                    side_label,
                    ev.price,
                    ev.absorbed_usdt,
                    ev.wall_status,
                    ev.absorption_score,
                    ev.description
                ));
            }
        }

        report.push_str("============================================================\n");
        report
    }

    pub fn get_metrics_json(&self) -> String {
        let metrics_guard = self.symbol_metrics.lock().unwrap();
        let events_guard = self.absorption_events.lock().unwrap();

        let json_out = serde_json::json!({
            "metrics": *metrics_guard,
            "recent_events": *events_guard,
        });

        serde_json::to_string_pretty(&json_out).unwrap_or_else(|_| "{}".to_string())
    }
}

fn update_or_insert_wall(
    symbol_walls: &mut HashMap<String, WallState>,
    key: String,
    symbol: String,
    side: String,
    price: f64,
    visible_qty: f64,
    now_ms: u64,
) {
    let visible_usdt = price * visible_qty;

    symbol_walls.entry(key).and_modify(|wall| {
        wall.current_qty = visible_qty;
        wall.current_usdt = visible_usdt;
        wall.last_seen_ms = now_ms;
    }).or_insert(WallState {
        symbol,
        side,
        price,
        initial_qty: visible_qty,
        initial_usdt: visible_usdt,
        current_qty: visible_qty,
        current_usdt: visible_usdt,
        executed_qty: 0.0,
        executed_usdt: 0.0,
        first_seen_ms: now_ms,
        last_seen_ms: now_ms,
        wall_status: "TESTING".to_string(),
        is_alerted: false,
    });
}

fn calculate_absorption_score(executed_usdt: f64, initial_usdt: f64) -> f64 {
    let exec_factor = (executed_usdt / 200_000.0).clamp(0.2, 1.0);
    let size_factor = (initial_usdt / 100_000.0).clamp(0.2, 1.0);

    (exec_factor * 0.6 + size_factor * 0.4).clamp(0.0, 1.0)
}

fn classify_alert_level(score: f64, executed_usdt: f64) -> String {
    if score >= 0.85 || executed_usdt >= 400_000.0 {
        "CRITICAL".to_string()
    } else if score >= 0.65 || executed_usdt >= 150_000.0 {
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
    engine: Arc<AbsorptionEngine>,
    data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let engine = Arc::new(AbsorptionEngine::new());
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
                    let min_abs_usdt = params.get("min_absorption_usdt").and_then(|v| v.as_u64());
                    let min_wall = params.get("min_wall_usdt").and_then(|v| v.as_u64());
                    let max_move = params.get("max_price_move_x1000").and_then(|v| v.as_u64());
                    let window_ms = params.get("window_ms").and_then(|v| v.as_u64());

                    state.engine.configure(min_abs_usdt, min_wall, max_move, window_ms);
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
    fn test_absorption_and_wall_confirmation_simulation() {
        let engine = AbsorptionEngine::new();
        engine.configure(Some(50000), Some(40000), Some(50), Some(60000));

        let t0 = 1000000u64;

        // Step 1: Initial Depth Snapshot -> Bid Wall at 100.0 ($100,000 USDT)
        let depth_snap1 = serde_json::json!({
            "BTCUSDT": {
                "bids": [
                    ["100.0", "1000.0"] // $100,000 USDT wall
                ],
                "asks": [
                    ["100.5", "10.0"]
                ],
                "event_time": t0
            }
        });
        engine.process_depth_payload(&depth_snap1, t0);

        // Verify tracked wall status
        {
            let walls = engine.tracked_walls.lock().unwrap();
            let btc_walls = walls.get("BTCUSDT").unwrap();
            assert_eq!(btc_walls.len(), 1);
            assert_eq!(btc_walls.get("BID_100.000000").unwrap().wall_status, "TESTING");
        }

        // Step 2: Ingest aggressive Market Sells hitting 100.0 ($60,000 USDT executed)
        let trade_data = serde_json::json!({
            "BTCUSDT": {
                "trade_id": 301,
                "price": "100.0",
                "quantity": "600.0", // $60,000 executed >= $50,000 min
                "buyer_is_maker": true,
                "event_time": t0 + 100
            }
        });
        engine.process_trade_payload(&trade_data, t0 + 100);

        // Step 3: Ingest Depth Snapshot 2 -> Bid Wall at 100.0 STILL SURVIVES!
        let depth_snap2 = serde_json::json!({
            "BTCUSDT": {
                "bids": [
                    ["100.0", "400.0"] // Remaining $40,000 USDT wall
                ],
                "asks": [
                    ["100.5", "10.0"]
                ],
                "event_time": t0 + 200
            }
        });

        let report = engine.process_depth_payload(&depth_snap2, t0 + 200);

        assert!(report.contains("EMİLİM (ABSORPTION) VE DUVAR TEYİT PANELİ"));
        assert!(report.contains("BTCUSDT"));

        // Check absorption event and confirmed status
        let events = engine.absorption_events.lock().unwrap();
        assert_eq!(events.len(), 1);

        let event = &events[0];
        assert_eq!(event.symbol, "BTCUSDT");
        assert_eq!(event.side, "BID_ABSORPTION");
        assert_eq!(event.price, 100.0);
        assert_eq!(event.absorbed_usdt, 60000.0);
        assert_eq!(event.wall_status, "CONFIRMED_GENUINE");
        assert!(event.absorption_score > 0.5);
    }
}
