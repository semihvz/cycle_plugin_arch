use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Direct Level Evaluation: Calculates L - ATR_last for Support and Resistance levels
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LevelProximityMetrics {
    pub symbol: String,
    pub mid_price: f64,
    pub best_bid: f64,
    pub best_ask: f64,
    pub last_atr: f64,

    // Support Level & Direct Calculation (L_s - ATR_last)
    pub support_level: Option<f64>,
    pub support_level_minus_atr: Option<f64>,

    // Resistance Level & Direct Calculation (L_r - ATR_last)
    pub resistance_level: Option<f64>,
    pub resistance_level_minus_atr: Option<f64>,

    pub last_updated_ms: u64,
}

pub struct LevelProximityEngine {
    pub mid_prices: Mutex<HashMap<String, (f64, f64, f64, u64)>>, // (bid, ask, mid, ts)
    pub atr_values: Mutex<HashMap<String, (f64, u64)>>,           // (atr, ts)
    pub support_levels: Mutex<HashMap<String, (f64, u64)>>,       // (level, ts)
    pub resistance_levels: Mutex<HashMap<String, (f64, u64)>>,    // (level, ts)
    pub symbol_metrics: Mutex<HashMap<String, LevelProximityMetrics>>,
}

impl LevelProximityEngine {
    pub fn new() -> Self {
        Self {
            mid_prices: Mutex::new(HashMap::new()),
            atr_values: Mutex::new(HashMap::new()),
            support_levels: Mutex::new(HashMap::new()),
            resistance_levels: Mutex::new(HashMap::new()),
            symbol_metrics: Mutex::new(HashMap::new()),
        }
    }

    /// Process incoming payloads from binance_gateway, plugin_atr, or plugin_ms_analyzer
    pub fn process_payload(&self, _stream_id: &str, json_data: &Value, now_ms: u64) -> String {
        self.ingest_all(json_data, now_ms);
        self.generate_report(now_ms)
    }

    pub fn ingest_all(&self, json_data: &Value, now_ms: u64) {
        self.ingest_bestprice(json_data, now_ms);
        self.ingest_atr(json_data, now_ms);
        self.ingest_ms_analyzer(json_data, now_ms);

        if let Some(obj) = json_data.as_object() {
            for (_key, val) in obj.iter() {
                if val.is_object() || val.is_array() {
                    self.ingest_bestprice(val, now_ms);
                    self.ingest_atr(val, now_ms);
                    self.ingest_ms_analyzer(val, now_ms);
                }
            }
        }
    }

    pub fn ingest_bestprice(&self, json_data: &Value, now_ms: u64) {
        let mut mid_guard = self.mid_prices.lock().unwrap();

        let target_obj = if let Some(inner) = json_data.get("stream_bestprice").and_then(|v| v.as_object()) {
            inner
        } else if let Some(obj) = json_data.as_object() {
            obj
        } else {
            return;
        };

        for (symbol, item) in target_obj.iter() {
            let bid = parse_f64(&item["best_bid"]);
            let ask = parse_f64(&item["best_ask"]);
            if bid > 0.0 || ask > 0.0 {
                let mid = if bid > 0.0 && ask > 0.0 { (bid + ask) / 2.0 } else { bid.max(ask) };
                mid_guard.insert(symbol.clone(), (bid, ask, mid, now_ms));
            }
        }
    }

    pub fn ingest_atr(&self, json_data: &Value, now_ms: u64) {
        let mut atr_guard = self.atr_values.lock().unwrap();

        let process_val = |val: &Value, guard: &mut HashMap<String, (f64, u64)>| {
            let symbol = val.get("symbol").and_then(|s| s.as_str()).unwrap_or("");
            let atr = parse_f64(&val["latest_atr"])
                .max(parse_f64(&val["atr"]))
                .max(parse_f64(&val["latest_tr"]));
            if !symbol.is_empty() && atr > 0.0 {
                guard.insert(symbol.to_string(), (atr, now_ms));
            }
        };

        if let Some(metrics_map) = json_data.get("metrics").and_then(|m| m.as_object()) {
            for (_stream, val) in metrics_map.iter() {
                process_val(val, &mut atr_guard);
            }
        } else if let Some(arr) = json_data.as_array() {
            for val in arr {
                process_val(val, &mut atr_guard);
            }
        } else if let Some(val) = json_data.as_object() {
            process_val(json_data, &mut atr_guard);
            for (_k, v) in val.iter() {
                if v.is_object() {
                    process_val(v, &mut atr_guard);
                }
            }
        }
    }

    pub fn ingest_ms_analyzer(&self, json_data: &Value, now_ms: u64) {
        let mut sup_guard = self.support_levels.lock().unwrap();
        let mut res_guard = self.resistance_levels.lock().unwrap();

        let extract_from_obj = |obj: &Value, s_guard: &mut HashMap<String, (f64, u64)>, r_guard: &mut HashMap<String, (f64, u64)>| {
            let symbol = obj.get("symbol").and_then(|s| s.as_str()).unwrap_or("TACUSDT").to_string();
            let current_price = parse_f64(&obj["current_price"]);

            if let Some(s_val) = obj.get("support_level").map(parse_f64).filter(|&v| v > 0.0) {
                s_guard.insert(symbol.clone(), (s_val, now_ms));
            }
            if let Some(r_val) = obj.get("resistance_level").map(parse_f64).filter(|&v| v > 0.0) {
                r_guard.insert(symbol.clone(), (r_val, now_ms));
            }

            if let Some(levels_arr) = obj.get("levels").and_then(|l| l.as_array()) {
                let mut closest_sup: Option<f64> = None;
                let mut closest_res: Option<f64> = None;

                for level_obj in levels_arr {
                    let price = parse_f64(&level_obj["price"]);
                    let l_type = level_obj.get("level_type").and_then(|t| t.as_str()).unwrap_or("");

                    if price > 0.0 {
                        if l_type.contains("SUPPORT") || (current_price > 0.0 && price < current_price) {
                            if closest_sup.map_or(true, |existing| price > existing) {
                                closest_sup = Some(price);
                            }
                        } else if l_type.contains("RESISTANCE") || (current_price > 0.0 && price > current_price) {
                            if closest_res.map_or(true, |existing| price < existing) {
                                closest_res = Some(price);
                            }
                        }
                    }
                }

                if let Some(sup) = closest_sup {
                    s_guard.insert(symbol.clone(), (sup, now_ms));
                }
                if let Some(res) = closest_res {
                    r_guard.insert(symbol.clone(), (res, now_ms));
                }
            }
        };

        if let Some(obj) = json_data.as_object() {
            extract_from_obj(json_data, &mut sup_guard, &mut res_guard);
            for (_k, v) in obj.iter() {
                if v.is_object() {
                    extract_from_obj(v, &mut sup_guard, &mut res_guard);
                }
            }
        }
    }

    pub fn generate_report(&self, now_ms: u64) -> String {
        let mid_guard = self.mid_prices.lock().unwrap();
        let atr_guard = self.atr_values.lock().unwrap();
        let sup_guard = self.support_levels.lock().unwrap();
        let res_guard = self.resistance_levels.lock().unwrap();
        let mut metrics_guard = self.symbol_metrics.lock().unwrap();

        let mut symbols_set: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        symbols_set.insert("TACUSDT".to_string()); // Default target symbol
        for k_sym in mid_guard.keys() { symbols_set.insert(k_sym.clone()); }
        for k_sym in atr_guard.keys() { symbols_set.insert(k_sym.clone()); }
        for k_sym in sup_guard.keys() { symbols_set.insert(k_sym.clone()); }

        let mut report = String::new();
        report.push_str("============================================================\n");
        report.push_str("🎯 KEY LEVEL ATR CALCULATOR (L - ATR_last)\n");
        report.push_str("============================================================\n");

        for symbol in &symbols_set {
            let (best_bid, best_ask, mid_price) = mid_guard.get(symbol)
                .map(|&(b, a, m, _)| (b, a, m))
                .unwrap_or((0.0, 0.0, 0.0));

            let last_atr = atr_guard.get(symbol).map(|&(a, _)| a).unwrap_or(0.0);
            let support_lvl = sup_guard.get(symbol).map(|&(s, _)| s);
            let resistance_lvl = res_guard.get(symbol).map(|&(r, _)| r);

            // Direct L_s - ATR_last calculation
            let sup_minus_atr = support_lvl.and_then(|l_s| {
                if last_atr > 0.0 { Some(l_s - last_atr) } else { None }
            });

            // Direct L_r - ATR_last calculation
            let res_minus_atr = resistance_lvl.and_then(|l_r| {
                if last_atr > 0.0 { Some(l_r - last_atr) } else { None }
            });

            let metrics = LevelProximityMetrics {
                symbol: symbol.clone(),
                mid_price,
                best_bid,
                best_ask,
                last_atr,
                support_level: support_lvl,
                support_level_minus_atr: sup_minus_atr,
                resistance_level: resistance_lvl,
                resistance_level_minus_atr: res_minus_atr,
                last_updated_ms: now_ms,
            };

            metrics_guard.insert(symbol.clone(), metrics);

            report.push_str(&format!(
                "[{}]\n\
                 ├─► Şimdiki Mid Fiyat: {:.8} (Bid: {:.8} / Ask: {:.8})\n\
                 ├─► Son ATR (14)     : {:.8}\n\
                 │\n",
                symbol, mid_price, best_bid, best_ask, last_atr
            ));

            // Support Display
            report.push_str(" ├─► 🟢 DESTEK SEVİYESİ (SUPPORT)\n");
            if let Some(l_s) = support_lvl {
                report.push_str(&format!(" │    ├─► Seviye (L_s)       : {:.8}\n", l_s));
                if let Some(val) = sup_minus_atr {
                    report.push_str(&format!(" │    └─► L_s - ATR_last     : {:.8}\n", val));
                } else {
                    report.push_str(" │    └─► L_s - ATR_last     : ⏳ ATR Bekleniyor...\n");
                }
            } else {
                report.push_str(" │    └─► Seviye (L_s)       : ⏳ MS Analyzer Verisi Bekleniyor...\n");
            }
            report.push_str(" │\n");

            // Resistance Display
            report.push_str(" └─► 🔴 DİRENÇ SEVİYESİ (RESISTANCE)\n");
            if let Some(l_r) = resistance_lvl {
                report.push_str(&format!("      ├─► Seviye (L_r)       : {:.8}\n", l_r));
                if let Some(val) = res_minus_atr {
                    report.push_str(&format!("      └─► L_r - ATR_last     : {:.8}\n", val));
                } else {
                    report.push_str("      └─► L_r - ATR_last     : ⏳ ATR Bekleniyor...\n");
                }
            } else {
                report.push_str("      └─► Seviye (L_r)       : ⏳ MS Analyzer Verisi Bekleniyor...\n");
            }
            report.push('\n');
        }
        report
    }

    pub fn get_metrics_json(&self) -> String {
        let metrics_guard = self.symbol_metrics.lock().unwrap();
        serde_json::to_string_pretty(&*metrics_guard).unwrap_or_else(|_| "{}".to_string())
    }
}

fn parse_f64(val: &Value) -> f64 {
    match val {
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
        _ => 0.0,
    }
}

// C ABI Plugin State & Exported Endpoints
struct PluginState {
    is_running: Arc<AtomicBool>,
    engine: Arc<Mutex<LevelProximityEngine>>,
    data: Arc<Mutex<Vec<u8>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(
    state_out: *mut *mut c_void,
) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        engine: Arc::new(Mutex::new(LevelProximityEngine::new())),
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

                if let Ok(json_data) = serde_json::from_slice::<Value>(&slice[32..]) {
                    let now_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;

                    let engine = state.engine.lock().unwrap();
                    let report = engine.process_payload(&stream_id, &json_data, now_ms);

                    let mut data_guard = state.data.lock().unwrap();
                    *data_guard = report.into_bytes();
                }
            }
            0
        }
        _ => 0,
    }
}
