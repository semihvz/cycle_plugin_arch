use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct BreakoutInput {
    // Klines
    p_high: f64,
    p_low: f64,
    p_open: f64,
    p_close: f64,
    volume_current: f64,
    
    // Indicators
    sigma: f64, // ATR(14)
    v_avg: f64, // SMA(Volume, 20)
    high14: f64,
    low14: f64,
    
    // detect-ms
    r: f64,
    s: f64,
    
    // Touches
    t_cnt: f64,
    v_touch_avg: f64,
    
    // Flow Rings
    oi: f64,
    oi_prev: f64,
    f_rate: f64,
    mu_20: f64, // Funding mean
    sigma_20: f64, // Funding stddev
    
    // CVD
    cvd_now: f64,
    cvd_prev_10: f64,
    sigma_cvd: f64,
    
    // Liq & Price
    liq_current: f64,
    liq_avg: f64,
    mark: f64,
    last: f64,
}

#[derive(Debug, serde::Serialize)]
struct BreakoutOutput {
    direction: String,
    broken_level: f64,
    breakout_quality: f64,
    fake_percentage: f64,
    certainty_percentage: f64,
}

fn calculate_breakout(input: &BreakoutInput) -> BreakoutOutput {
    let epsilon = 1e-9;
    
    // 1. Seviye Sağlamlık Skoru (S_level)
    let touch_score = (input.t_cnt / 15.0).min(1.0);
    let vol_touch_score = if input.v_avg > 0.0 { (input.v_touch_avg / input.v_avg).min(1.0) } else { 0.0 };
    let narrow_score = ( (2.0 * input.sigma) / ((input.r - input.s).abs() + epsilon) ).min(1.0);
    
    let s_level = (touch_score * 0.40) + (vol_touch_score * 0.40) + (narrow_score * 0.20);
    
    // 2. Kırılım Tetikleyici
    let mut direction = "NONE".to_string();
    let mut broken_level = 0.0;
    
    if input.p_close >= input.r + 0.25 * input.sigma {
        direction = "UP".to_string();
        broken_level = input.r;
    } else if input.p_close <= input.s - 0.25 * input.sigma {
        direction = "DOWN".to_string();
        broken_level = input.s;
    }
    
    if direction == "NONE" {
        return BreakoutOutput {
            direction,
            broken_level: 0.0,
            breakout_quality: 0.0,
            fake_percentage: 0.0,
            certainty_percentage: 0.0,
        };
    }
    
    // 3. Kırılım Kalitesi (Q)
    let v_score = if input.v_avg > 0.0 { (input.volume_current / input.v_avg).min(1.0) } else { 0.0 };
    let hl_range = input.high14 - input.low14;
    let m_score = if hl_range > 0.0 {
        if direction == "UP" {
            (input.p_close - input.low14) / hl_range
        } else {
            (input.high14 - input.p_close) / hl_range
        }
    } else {
        0.0
    };
    
    let current_hl = input.p_high - input.p_low;
    let body_score = if current_hl > 0.0 {
        (input.p_close - input.p_open).abs() / current_hl
    } else {
        0.0
    };
    
    let q = (v_score * 0.40 + m_score * 0.35 + body_score * 0.25) * 100.0;
    
    // 4. Sahte Olasılığı (F)
    let w_score = if current_hl > 0.0 {
        if direction == "UP" {
            ((input.p_high - input.p_close.max(input.p_open)) / current_hl) * 2.0
        } else {
            ((input.p_close.min(input.p_open) - input.p_low) / current_hl) * 2.0
        }
    } else {
        0.0
    };
    
    let delta_oi_norm = (input.oi - input.oi_prev) / (input.oi_prev + epsilon);
    let oi_score = (-delta_oi_norm).max(0.0);
    
    let z_funding = if input.sigma_20 > 0.0 {
        (input.f_rate - input.mu_20) / input.sigma_20
    } else {
        0.0
    };
    let fz_score = (z_funding / 3.0).max(0.0).min(1.0);
    
    let liq_score = if input.liq_avg > 0.0 { (input.liq_current / input.liq_avg).min(1.0) } else { 0.0 };
    
    let mut f = (w_score * 0.30 + oi_score * 0.30 + fz_score * 0.20 + liq_score * 0.20) * 100.0;
    
    // 5. Kırılım Kesinliği (C)
    let cvd_score = if input.sigma_cvd > 0.0 {
        ((input.cvd_now - input.cvd_prev_10) / (input.sigma_cvd * 10.0)).max(0.0).min(1.0)
    } else {
        0.0
    };
    
    let mp_score = if direction == "UP" && input.mark > input.last {
        1.0 // Contango
    } else if direction == "DOWN" && input.mark < input.last {
        1.0 // Backwardation
    } else {
        0.5
    };
    
    let mut c = (s_level * 0.40 + cvd_score * 0.40 + mp_score * 0.20) * 100.0;
    
    // 6. Acımasız Kurallar (Hard Rules)
    if input.liq_avg > 0.0 && input.liq_current > 5.0 * input.liq_avg {
        direction = "NONE".to_string(); // Likidasyon avı (Stop-hunt)
    }
    
    if z_funding > 3.0 {
        c = c.min(30.0); // Aşırı funding, kesinlik maks %30
    }
    
    // Fitil tuzağı (Wick broke the level but close didn't)
    // Actually the close broke the level by 0.25 sigma, but maybe there's a huge wick?
    // "Fitil seviyeyi deldi ama kapanış eşik altında -> Fake +%15"
    // In step 2, if close didn't break threshold, direction is NONE.
    // If direction is NONE, we don't output fake_percentage > 0 typically, but let's implement the rule if we check potential wicks:
    // If direction == NONE and ((p_high > r && p_close < r) or (p_low < s && p_close > s))
    if direction == "NONE" {
        if (input.p_high > input.r && input.p_close < input.r + 0.25 * input.sigma) || 
           (input.p_low < input.s && input.p_close > input.s - 0.25 * input.sigma) {
            f += 15.0; // Fitil tuzağı
        }
    }
    
    BreakoutOutput {
        direction,
        broken_level,
        breakout_quality: q,
        fake_percentage: f,
        certainty_percentage: c,
    }
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
        data: Arc::new(Mutex::new(b"plugin_breakout hazir.".to_vec())),
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
                    if msg["action"].as_str() == Some("detect_breakout") {
                        if let Ok(input) = serde_json::from_value::<BreakoutInput>(msg["data"].clone()) {
                            let result = calculate_breakout(&input);
                            
                            // Send result back
                            if let Some(from) = msg["from"].as_str() {
                                let mut out = state.outbox.lock().unwrap();
                                out.push(serde_json::json!({
                                    "to": from,
                                    "from": "plugin_breakout",
                                    "action": "breakout_result",
                                    "data": result
                                }));
                                
                                // Update internal data for monitoring
                                let mut data = state.data.lock().unwrap();
                                let report = if result.direction == "NONE" {
                                    format!("Durum: Beklemede (Kirilim Yok)")
                                } else {
                                    let dir_icon = if result.direction == "UP" { "🚀 YUKARI" } else { "💥 ASAGI" };
                                    format!(
                                        "=========================================\n\
                                         🔥 KIRILIM TESPIT RAPORU 🔥\n\
                                         =========================================\n\
                                         Yön: {}\n\
                                         Kirilan Seviye: {:.2}\n\
                                         Kalite Skoru (Q): %{:.2}\n\
                                         Kesinlik Skoru (C): %{:.2}\n\
                                         Sahte/Tuzak Ihtimali (F): %{:.2}\n\
                                         =========================================",
                                         dir_icon,
                                         result.broken_level,
                                         result.breakout_quality,
                                         result.certainty_percentage,
                                         result.fake_percentage
                                    )
                                };
                                *data = report.into_bytes();
                            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_breakout_report() {
        let mut state_ptr: *mut c_void = std::ptr::null_mut();
        unsafe {
            let endpoint_fn = init_plugin(&mut state_ptr);
            
            // Start
            endpoint_fn(state_ptr, 0, std::ptr::null(), 0, std::ptr::null_mut(), 0);
            
            // Inbox msg
            let input = BreakoutInput {
                p_high: 66000.0,
                p_low: 65000.0,
                p_open: 65100.0,
                p_close: 65900.0,
                volume_current: 100.0,
                sigma: 200.0,
                v_avg: 50.0,
                high14: 66000.0,
                low14: 64000.0,
                r: 65800.0, // Close > R + 0.25*sigma (65900 > 65800 + 50 = 65850)
                s: 64500.0,
                t_cnt: 5.0,
                v_touch_avg: 80.0,
                oi: 1000.0,
                oi_prev: 900.0,
                f_rate: 0.01,
                mu_20: 0.01,
                sigma_20: 0.005,
                cvd_now: 50.0,
                cvd_prev_10: 10.0,
                sigma_cvd: 20.0,
                liq_current: 10000.0,
                liq_avg: 5000.0,
                mark: 65950.0,
                last: 65900.0,
            };
            
            let payload = serde_json::json!({
                "from": "test",
                "action": "detect_breakout",
                "data": input
            });
            let payload_bytes = serde_json::to_vec(&payload).unwrap();
            
            endpoint_fn(state_ptr, 6, payload_bytes.as_ptr(), payload_bytes.len(), std::ptr::null_mut(), 0);
            
            // Read DataMonitor
            let mut buf = vec![0u8; 1024];
            let len = endpoint_fn(state_ptr, 4, std::ptr::null(), 0, buf.as_mut_ptr(), buf.len());
            
            let output = String::from_utf8_lossy(&buf[..len]);
            println!("Report:\n{}", output);
            assert!(output.contains("KIRILIM TESPIT RAPORU"));
        }
    }
}
