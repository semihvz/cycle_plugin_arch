use plugin_level_proximity::{LevelProximityEngine, init_plugin};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_proximity_formula_calculation() {
    let mut engine = LevelProximityEngine::new();
    engine.set_k_threshold(0.5);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    // 1. Ingest Mid Price for TACUSDT (Bid: 0.1234, Ask: 0.1236 => Mid: 0.1235)
    let bestprice_payload = json!({
        "TACUSDT": {
            "best_bid": "0.12340000",
            "best_ask": "0.12360000"
        }
    });
    engine.ingest_bestprice(&bestprice_payload, now);

    // 2. Ingest ATR for TACUSDT (latest_atr: 0.0020)
    let atr_payload = json!([
        {
            "symbol": "TACUSDT",
            "latest_atr": "0.00200000"
        }
    ]);
    engine.ingest_atr(&atr_payload, now);

    // 3. Ingest MS Analyzer Levels for TACUSDT
    // Support: 0.1230, Resistance: 0.1250
    let ms_payload = json!({
        "symbol": "TACUSDT",
        "current_price": "0.12350000",
        "support_level": "0.12300000",
        "resistance_level": "0.12500000"
    });
    engine.ingest_ms_analyzer(&ms_payload, now);

    let report = engine.generate_report(now);
    assert!(report.contains("KEY LEVEL PROXIMITY PANEL"));
    assert!(report.contains("TACUSDT"));

    let metrics_guard = engine.symbol_metrics.lock().unwrap();
    let tac = metrics_guard.get("TACUSDT").expect("TACUSDT metrics missing");

    assert_eq!(tac.mid_price, 0.1235);
    assert_eq!(tac.last_atr, 0.0020);
    assert_eq!(tac.support_level, Some(0.1230));
    assert_eq!(tac.resistance_level, Some(0.1250));

    // Support Distance: D_s = |0.1235 - 0.1230| / 0.0020 = 0.0005 / 0.0020 = 0.25 ATR
    // Since D_s (0.25) < k (0.5) => Alert Active! Formula: L_s - ATR_last = 0.1230 - 0.0020 = 0.1210
    let expected_sup_d = 0.25;
    assert!((tac.support_distance_d.unwrap() - expected_sup_d).abs() < 1e-5);
    assert!(tac.support_alert_active);
    let expected_sig = 0.1230 - 0.0020;
    assert!((tac.support_signal_val.unwrap() - expected_sig).abs() < 1e-5);

    // Resistance Distance: D_r = |0.1235 - 0.1250| / 0.0020 = 0.0015 / 0.0020 = 0.75 ATR
    // Since D_r (0.75) >= k (0.5) => Alert Inactive!
    let expected_res_d = 0.75;
    assert!((tac.resistance_distance_d.unwrap() - expected_res_d).abs() < 1e-5);
    assert!(!tac.resistance_alert_active);
    assert_eq!(tac.resistance_signal_val, None);
}

#[test]
fn test_c_abi_handle_endpoint() {
    unsafe {
        let mut state_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let endpoint_fn = init_plugin(&mut state_ptr);

        assert!(!state_ptr.is_null());

        let _now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // 1. Start plugin with k = 0.5
        let start_config = json!({
            "plugin_params": {
                "k": 0.5
            }
        });
        let config_bytes = serde_json::to_vec(&start_config).unwrap();
        let res = endpoint_fn(state_ptr, 0, config_bytes.as_ptr(), config_bytes.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // 2. Check IsWorking
        let mut working_byte: u8 = 0;
        let is_working = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, &mut working_byte, 1);
        assert_eq!(is_working, 1);
        assert_eq!(working_byte, 1);

        // 3. Send Inbox payload stream_bestprice
        let mut payload = Vec::new();
        let mut header = [0u8; 32];
        let sname = b"stream_bestprice";
        header[..sname.len()].copy_from_slice(sname);
        payload.extend_from_slice(&header);

        let data_body = json!({
            "stream_bestprice": {
                "TACUSDT": {
                    "best_bid": "0.12340000",
                    "best_ask": "0.12360000"
                }
            }
        });
        payload.extend_from_slice(&serde_json::to_vec(&data_body).unwrap());

        let inbox_res = endpoint_fn(state_ptr, 6, payload.as_ptr(), payload.len(), std::ptr::null_mut(), 0);
        assert_eq!(inbox_res, 0);

        // 4. Read RawData / DataMonitor (endpoint 5)
        let mut out_buf = vec![0u8; 4096];
        let bytes_read = endpoint_fn(state_ptr, 5, std::ptr::null(), 0, out_buf.as_mut_ptr(), out_buf.len());
        assert!(bytes_read > 0);

        let report_str = String::from_utf8_lossy(&out_buf[..bytes_read]);
        assert!(report_str.contains("KEY LEVEL PROXIMITY PANEL"));
        assert!(report_str.contains("TACUSDT"));

        // 5. Stop plugin
        let stop_res = endpoint_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(stop_res, 0);
    }
}
