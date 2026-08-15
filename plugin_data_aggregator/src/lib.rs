use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AggregatedRawData {
    // Klines
    pub p_high: f64,
    pub p_low: f64,
    pub p_open: f64,
    pub p_close: f64,
    pub volume_current: f64,
    
    // Arrays for indicators (just keeping a history to pass along, or just the raw latest)
    pub recent_volumes: Vec<f64>,
    pub recent_highs: Vec<f64>,
    pub recent_lows: Vec<f64>,
    pub recent_closes: Vec<f64>,
    
    // MS pivots (if provided directly)
    pub r: f64,
    pub s: f64,
    pub t_cnt: f64,
    pub v_touch_avg: f64,
    
    // Flow Rings
    pub oi: f64,
    pub oi_prev: f64,
    pub f_rate: f64,
    pub recent_f_rates: Vec<f64>, // to calculate mu_20, sigma_20
    
    // CVD
    pub cvd_now: f64,
    pub cvd_prev_10: f64,
    pub recent_cvds: Vec<f64>, // to calculate sigma_cvd
    
    // Liq & Price
    pub liq_current: f64,
    pub liq_avg: f64,
    pub mark: f64,
    pub last: f64,
}

struct PluginState {
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    outbox: Arc<Mutex<Vec<serde_json::Value>>>,
    aggregated_data: Arc<Mutex<AggregatedRawData>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(b"plugin_data_aggregator hazir.".to_vec())),
        outbox: Arc::new(Mutex::new(Vec::new())),
        aggregated_data: Arc::new(Mutex::new(AggregatedRawData::default())),
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
            if state.is_running.load(Ordering::Relaxed) { 1 } else { 0 }
        }
        3 => { // DataValid
            1
        }
        4 => { // DataMonitor
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
            len
        }
        6 => { // Inbox
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(slice) {
                    let mut ag_data = state.aggregated_data.lock().unwrap();
                    let action = msg["action"].as_str().unwrap_or("");
                    
                    match action {
                        "update_klines" => {
                            if let Some(klines) = msg["data"].as_array() {
                                if let Some(last) = klines.last() {
                                    ag_data.p_open = last["open"].as_f64().unwrap_or(0.0);
                                    ag_data.p_high = last["high"].as_f64().unwrap_or(0.0);
                                    ag_data.p_low = last["low"].as_f64().unwrap_or(0.0);
                                    ag_data.p_close = last["close"].as_f64().unwrap_or(0.0);
                                    ag_data.volume_current = last["volume"].as_f64().unwrap_or(0.0);
                                    
                                    let vc = ag_data.volume_current;
                                    ag_data.recent_volumes.push(vc);
                                    let ph = ag_data.p_high;
                                    ag_data.recent_highs.push(ph);
                                    let pl = ag_data.p_low;
                                    ag_data.recent_lows.push(pl);
                                    let pc = ag_data.p_close;
                                    ag_data.recent_closes.push(pc);
                                    
                                    if ag_data.recent_volumes.len() > 30 { ag_data.recent_volumes.remove(0); }
                                    if ag_data.recent_highs.len() > 30 { ag_data.recent_highs.remove(0); }
                                    if ag_data.recent_lows.len() > 30 { ag_data.recent_lows.remove(0); }
                                    if ag_data.recent_closes.len() > 30 { ag_data.recent_closes.remove(0); }
                                }
                            }
                        },
                        "update_markprice" => {
                            if let Some(data) = msg.get("data") {
                                ag_data.f_rate = data["funding_rate"].as_f64().unwrap_or(ag_data.f_rate);
                                ag_data.mark = data["mark_price"].as_f64().unwrap_or(ag_data.mark);
                                let f_rate = ag_data.f_rate;
                                ag_data.recent_f_rates.push(f_rate);
                                if ag_data.recent_f_rates.len() > 30 { ag_data.recent_f_rates.remove(0); }
                            }
                        },
                        "update_oi" => {
                            if let Some(data) = msg.get("data") {
                                ag_data.oi_prev = ag_data.oi;
                                ag_data.oi = data["oi"].as_f64().unwrap_or(ag_data.oi);
                            }
                        },
                        "update_aggtrade" => {
                            if let Some(data) = msg.get("data") {
                                let cvd_delta = data["cvd_delta"].as_f64().unwrap_or(0.0);
                                ag_data.last = data["price"].as_f64().unwrap_or(ag_data.last);
                                ag_data.cvd_prev_10 = if ag_data.recent_cvds.len() >= 10 {
                                    ag_data.recent_cvds[ag_data.recent_cvds.len() - 10]
                                } else {
                                    ag_data.cvd_now
                                };
                                ag_data.cvd_now += cvd_delta;
                                let cvd_now = ag_data.cvd_now;
                                ag_data.recent_cvds.push(cvd_now);
                                if ag_data.recent_cvds.len() > 30 { ag_data.recent_cvds.remove(0); }
                            }
                        },
                        "update_ms" => {
                            if let Some(data) = msg.get("data") {
                                ag_data.r = data["r"].as_f64().unwrap_or(ag_data.r);
                                ag_data.s = data["s"].as_f64().unwrap_or(ag_data.s);
                                ag_data.t_cnt = data["t_cnt"].as_f64().unwrap_or(ag_data.t_cnt);
                                ag_data.v_touch_avg = data["v_touch_avg"].as_f64().unwrap_or(ag_data.v_touch_avg);
                            }
                        },
                        "update_liq" => {
                            if let Some(data) = msg.get("data") {
                                ag_data.liq_current = data["liq_current"].as_f64().unwrap_or(ag_data.liq_current);
                                ag_data.liq_avg = data["liq_avg"].as_f64().unwrap_or(ag_data.liq_avg);
                            }
                        },
                        _ => {}
                    }
                    
                    // Otomatik olarak tüm güncellemelerde Feature Engine'i tetikle!
                    if action.starts_with("update_") || action == "trigger_feature_engine" {
                        let out_msg = serde_json::json!({
                            "to": "plugin_feature_engine",
                            "from": "plugin_data_aggregator",
                            "action": "process_raw_data",
                            "data": *ag_data
                        });
                        state.outbox.lock().unwrap().push(out_msg);
                    }
                    
                    let mut ds = state.data.lock().unwrap();
                    *ds = serde_json::to_vec_pretty(&*ag_data).unwrap_or_default();
                }
            }
            0
        }
        7 => { // Outbox Check
            let mut out = state.outbox.lock().unwrap();
            if out.is_empty() {
                0
            } else {
                let msg = out.remove(0);
                if let Ok(json_str) = serde_json::to_string(&msg) {
                    let bytes = json_str.as_bytes();
                    let len = bytes.len().min(out_max_len);
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buf, len);
                    len
                } else {
                    0
                }
            }
        }
        _ => 0,
    }
}
