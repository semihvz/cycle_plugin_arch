use plugin_price_impact::{PriceImpactEngine, init_plugin};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_multi_window_price_impact_and_trade_summary() {
    let engine = PriceImpactEngine::new();
    engine.configure(None, Some(0.0));

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let t_50s = now.saturating_sub(50000); // 50s ago (within 60s window)
    let t_25s = now.saturating_sub(25000); // 25s ago (within 30s & 60s windows)
    let t_8s  = now.saturating_sub(8000);  // 8s ago (within 10s, 30s & 60s windows)
    let t_2s  = now.saturating_sub(2000);  // 2s ago (within 5s, 10s, 30s & 60s windows)
    let t_now = now;

    // Snapshot 50s ago (Mid: 50000.0)
    engine.ingest_bestprice(&json!({ "BTCUSDT": { "best_bid": "49999.0", "best_ask": "50001.0", "event_time": t_50s } }), t_50s);
    engine.ingest_trades(&json!({ "BTCUSDT": { "trade_id": 100, "price": "50001.0", "quantity": "1.0", "buyer_is_maker": false, "event_time": t_50s } }), t_50s, 0.0);

    // Snapshot 25s ago (Mid: 50010.0)
    engine.ingest_bestprice(&json!({ "BTCUSDT": { "best_bid": "50009.0", "best_ask": "50011.0", "event_time": t_25s } }), t_25s);
    engine.ingest_trades(&json!({ "BTCUSDT": { "trade_id": 101, "price": "50011.0", "quantity": "1.0", "buyer_is_maker": false, "event_time": t_25s } }), t_25s, 0.0);

    // Snapshot 8s ago (Mid: 50020.0)
    engine.ingest_bestprice(&json!({ "BTCUSDT": { "best_bid": "50019.0", "best_ask": "50021.0", "event_time": t_8s } }), t_8s);
    engine.ingest_trades(&json!({ "BTCUSDT": { "trade_id": 102, "price": "50021.0", "quantity": "1.0", "buyer_is_maker": false, "event_time": t_8s } }), t_8s, 0.0);

    // Snapshot 2s ago (Mid: 50030.0)
    engine.ingest_bestprice(&json!({ "BTCUSDT": { "best_bid": "50029.0", "best_ask": "50031.0", "event_time": t_2s } }), t_2s);
    engine.ingest_trades(&json!({ "BTCUSDT": { "trade_id": 103, "price": "50031.0", "quantity": "1.0", "buyer_is_maker": false, "event_time": t_2s } }), t_2s, 0.0);

    // Snapshot Now (Mid: 50050.0)
    engine.ingest_bestprice(&json!({ "BTCUSDT": { "best_bid": "50049.0", "best_ask": "50051.0", "event_time": t_now } }), t_now);

    let report = engine.generate_report(t_now);
    assert!(report.contains("MULTI-WINDOW PRICE IMPACT & TRADE ANALİZİ"));
    assert!(report.contains("100ms"));
    assert!(report.contains("500ms"));
    assert!(report.contains("1s"));
    assert!(report.contains("5s"));
    assert!(report.contains("10s"));
    assert!(report.contains("30s"));
    assert!(report.contains("60s"));
    assert!(report.contains("BTCUSDT"));

    let metrics_guard = engine.symbol_metrics.lock().unwrap();
    let btc = metrics_guard.get("BTCUSDT").expect("BTCUSDT metrics missing");

    assert_eq!(btc.mid_price_now, 50050.0);
    assert_eq!(btc.windows.len(), 7); // 100ms, 500ms, 1s, 5s, 10s, 30s, 60s

    // Check window labels
    assert_eq!(btc.windows[0].window_label, "100ms");
    assert_eq!(btc.windows[1].window_label, "500ms");
    assert_eq!(btc.windows[2].window_label, "1s");
    assert_eq!(btc.windows[3].window_label, "5s");
    assert_eq!(btc.windows[4].window_label, "10s");
    assert_eq!(btc.windows[5].window_label, "30s");
    assert_eq!(btc.windows[6].window_label, "60s");

    // 60s window should see 4 trades
    assert_eq!(btc.windows[6].total_trades, 4);
    // 30s window should see 3 trades
    assert_eq!(btc.windows[5].total_trades, 3);
    // 10s window should see 2 trades
    assert_eq!(btc.windows[4].total_trades, 2);
    // 5s window should see 1 trade
    assert_eq!(btc.windows[3].total_trades, 1);
}

#[test]
fn test_c_abi_handle_endpoint() {
    unsafe {
        let mut state_ptr: *mut std::ffi::c_void = std::ptr::null_mut();
        let endpoint_fn = init_plugin(&mut state_ptr);

        assert!(!state_ptr.is_null());

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        // 1. Start plugin
        let start_config = json!({
            "plugin_params": {
                "min_trade_usdt": 0.0
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

        // 3. Send Inbox payload with 32-byte header stream_bestprice
        let mut payload = Vec::new();
        let mut header = [0u8; 32];
        let sname = b"stream_bestprice";
        header[..sname.len()].copy_from_slice(sname);
        payload.extend_from_slice(&header);

        let data_body = json!({
            "stream_bestprice": {
                "SOLUSDT": {
                    "best_bid": "150.0",
                    "best_ask": "150.1",
                    "event_time": now
                }
            },
            "stream_trades": {
                "SOLUSDT": {
                    "trade_id": 505,
                    "price": "150.1",
                    "quantity": "10.0",
                    "buyer_is_maker": false,
                    "event_time": now
                }
            }
        });
        payload.extend_from_slice(&serde_json::to_vec(&data_body).unwrap());

        let inbox_res = endpoint_fn(state_ptr, 6, payload.as_ptr(), payload.len(), std::ptr::null_mut(), 0);
        assert_eq!(inbox_res, 0);

        // 4. Read RawData / DataMonitor (endpoint 5)
        let mut out_buf = vec![0u8; 8192];
        let bytes_read = endpoint_fn(state_ptr, 5, std::ptr::null(), 0, out_buf.as_mut_ptr(), out_buf.len());
        assert!(bytes_read > 0);

        let report_str = String::from_utf8_lossy(&out_buf[..bytes_read]);
        assert!(report_str.contains("MULTI-WINDOW PRICE IMPACT & TRADE ANALİZİ"));
        assert!(report_str.contains("SOLUSDT"));

        // 5. Stop plugin
        let stop_res = endpoint_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(stop_res, 0);
    }
}
