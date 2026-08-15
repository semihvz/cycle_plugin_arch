use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AggregatedRawData {
    pub p_high: f64,
    pub p_low: f64,
    pub p_open: f64,
    pub p_close: f64,
    pub volume_current: f64,
    pub recent_volumes: Vec<f64>,
    pub recent_highs: Vec<f64>,
    pub recent_lows: Vec<f64>,
    pub recent_closes: Vec<f64>,
    pub r: f64,
    pub s: f64,
    pub t_cnt: f64,
    pub v_touch_avg: f64,
    pub oi: f64,
    pub oi_prev: f64,
    pub f_rate: f64,
    pub recent_f_rates: Vec<f64>,
    pub cvd_now: f64,
    pub cvd_prev_10: f64,
    pub recent_cvds: Vec<f64>,
    pub liq_current: f64,
    pub liq_avg: f64,
    pub mark: f64,
    pub last: f64,
}

#[derive(Debug, Serialize)]
struct BreakoutInput {
    p_high: f64,
    p_low: f64,
    p_open: f64,
    p_close: f64,
    volume_current: f64,
    sigma: f64, // ATR(14)
    v_avg: f64, // SMA(Volume, 20)
    high14: f64,
    low14: f64,
    r: f64,
    s: f64,
    t_cnt: f64,
    v_touch_avg: f64,
    oi: f64,
    oi_prev: f64,
    f_rate: f64,
    mu_20: f64, // Funding mean
    sigma_20: f64, // Funding stddev
    cvd_now: f64,
    cvd_prev_10: f64,
    sigma_cvd: f64,
    liq_current: f64,
    liq_avg: f64,
    mark: f64,
    last: f64,
}

fn std_dev(data: &[f64], mean: f64) -> f64 {
    if data.is_empty() { return 0.0; }
    let variance: f64 = data.iter().map(|value| {
        let diff = mean - *value;
        diff * diff
    }).sum::<f64>() / data.len() as f64;
    variance.sqrt()
}

fn mean(data: &[f64]) -> f64 {
    if data.is_empty() { return 0.0; }
    let sum: f64 = data.iter().sum();
    sum / data.len() as f64
}

struct PluginState {
    is_running: Arc<AtomicBool>,
    data: Arc<Mutex<Vec<u8>>>,
    outbox: Arc<Mutex<Vec<serde_json::Value>>>,
}

#[no_mangle]
pub unsafe extern "C" fn init_plugin(state_out: *mut *mut c_void) -> unsafe extern "C" fn(*mut c_void, u32, *const u8, usize, *mut u8, usize) -> usize {
    let state = Box::new(PluginState {
        is_running: Arc::new(AtomicBool::new(false)),
        data: Arc::new(Mutex::new(b"plugin_feature_engine hazir.".to_vec())),
        outbox: Arc::new(Mutex::new(Vec::new())),
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
        0 => { state.is_running.store(true, Ordering::Relaxed); 0 }
        1 => { state.is_running.store(false, Ordering::Relaxed); 0 }
        2 => { if state.is_running.load(Ordering::Relaxed) { 1 } else { 0 } }
        3 => { 1 }
        4 => {
            let guard = state.data.lock().unwrap();
            let len = guard.len().min(out_max_len);
            std::ptr::copy_nonoverlapping(guard.as_ptr(), out_buf, len);
            len
        }
        6 => {
            if payload_len > 0 {
                let slice = std::slice::from_raw_parts(payload, payload_len);
                if let Ok(msg) = serde_json::from_slice::<serde_json::Value>(slice) {
                    if msg["action"].as_str() == Some("process_raw_data") {
                        if let Ok(raw) = serde_json::from_value::<AggregatedRawData>(msg["data"].clone()) {
                            
                            // Calculate features
                            let v_avg = mean(&raw.recent_volumes);
                            let high14 = raw.recent_highs.iter().cloned().fold(f64::NEG_INFINITY, f64::max).max(raw.p_high);
                            let low14 = raw.recent_lows.iter().cloned().fold(f64::INFINITY, f64::min).min(raw.p_low);
                            
                            // Approximation of ATR(14)
                            let tr = (raw.p_high - raw.p_low).abs(); // Simplified TR
                            let sigma = tr; // Need full ATR array for real sigma, but simplifying for MVP
                            
                            let mu_20 = mean(&raw.recent_f_rates);
                            let sigma_20 = std_dev(&raw.recent_f_rates, mu_20);
                            
                            let cvd_mean = mean(&raw.recent_cvds);
                            let sigma_cvd = std_dev(&raw.recent_cvds, cvd_mean);
                            
                            let b_input = BreakoutInput {
                                p_high: raw.p_high,
                                p_low: raw.p_low,
                                p_open: raw.p_open,
                                p_close: raw.p_close,
                                volume_current: raw.volume_current,
                                sigma,
                                v_avg,
                                high14,
                                low14,
                                r: if raw.r == 0.0 { raw.p_high * 1.01 } else { raw.r }, // mock if 0
                                s: if raw.s == 0.0 { raw.p_low * 0.99 } else { raw.s }, // mock if 0
                                t_cnt: raw.t_cnt,
                                v_touch_avg: raw.v_touch_avg,
                                oi: raw.oi,
                                oi_prev: raw.oi_prev,
                                f_rate: raw.f_rate,
                                mu_20,
                                sigma_20,
                                cvd_now: raw.cvd_now,
                                cvd_prev_10: raw.cvd_prev_10,
                                sigma_cvd,
                                liq_current: raw.liq_current,
                                liq_avg: raw.liq_avg,
                                mark: raw.mark,
                                last: raw.last,
                            };
                            
                            let out_msg = serde_json::json!({
                                "to": "plugin_breakout",
                                "from": "plugin_feature_engine",
                                "action": "detect_breakout",
                                "data": b_input
                            });
                            
                            state.outbox.lock().unwrap().push(out_msg);
                            
                            let mut ds = state.data.lock().unwrap();
                            *ds = b"Features calculated and sent to breakout plugin.".to_vec();
                        }
                    }
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
