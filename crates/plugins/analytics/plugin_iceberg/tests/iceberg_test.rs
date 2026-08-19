use plugin_iceberg::init_plugin;
use std::ffi::c_void;

#[test]
fn test_cabi_plugin_lifecycle_and_inbox() {
    unsafe {
        let mut state_ptr: *mut c_void = std::ptr::null_mut();
        let handle_fn = init_plugin(&mut state_ptr);

        assert!(!state_ptr.is_null());

        // 1. IsWorking before Start -> 0
        let mut is_working_buf = [0u8; 1];
        let res = handle_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(res, 1);
        assert_eq!(is_working_buf[0], 0);

        // 2. Start plugin (Endpoint 0)
        let config_json = br#"{"plugin_params":{"min_iceberg_usdt":20000,"min_exec_ratio_x10":20,"min_refill_count":2}}"#;
        let res = handle_fn(state_ptr, 0, config_json.as_ptr(), config_json.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // 3. IsWorking after Start -> 1
        let res = handle_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(res, 1);
        assert_eq!(is_working_buf[0], 1);

        // 4. Send initial depth payload
        let mut payload_depth1 = vec![0u8; 32];
        let stream_id_depth = b"stream_depth";
        payload_depth1[..stream_id_depth.len()].copy_from_slice(stream_id_depth);

        let depth_data1 = serde_json::json!({
            "ETHUSDT": {
                "bids": [
                    ["3000.0", "5.0"] // $15,000 USDT visible
                ],
                "asks": [
                    ["3005.0", "1.0"]
                ],
                "event_time": 1700000000000u64
            }
        });
        let body_depth1 = serde_json::to_vec(&depth_data1).unwrap();
        payload_depth1.extend_from_slice(&body_depth1);
        handle_fn(state_ptr, 6, payload_depth1.as_ptr(), payload_depth1.len(), std::ptr::null_mut(), 0);

        // Send trade 1
        let mut payload_trade1 = vec![0u8; 32];
        let stream_id_trade = b"stream_trades";
        payload_trade1[..stream_id_trade.len()].copy_from_slice(stream_id_trade);
        let trade_data1 = serde_json::json!({
            "ETHUSDT": {
                "trade_id": 501,
                "price": "3000.0",
                "quantity": "10.0", // $30,000 executed
                "buyer_is_maker": true,
                "event_time": 1700000000100u64
            }
        });
        let body_trade1 = serde_json::to_vec(&trade_data1).unwrap();
        payload_trade1.extend_from_slice(&body_trade1);
        handle_fn(state_ptr, 6, payload_trade1.as_ptr(), payload_trade1.len(), std::ptr::null_mut(), 0);

        // Send depth snapshot 2 (Refill 1)
        handle_fn(state_ptr, 6, payload_depth1.as_ptr(), payload_depth1.len(), std::ptr::null_mut(), 0);

        // Send trade 2
        let mut payload_trade2 = vec![0u8; 32];
        payload_trade2[..stream_id_trade.len()].copy_from_slice(stream_id_trade);
        let trade_data2 = serde_json::json!({
            "ETHUSDT": {
                "trade_id": 502,
                "price": "3000.0",
                "quantity": "5.0", // $15,000 executed
                "buyer_is_maker": true,
                "event_time": 1700000000200u64
            }
        });
        let body_trade2 = serde_json::to_vec(&trade_data2).unwrap();
        payload_trade2.extend_from_slice(&body_trade2);
        handle_fn(state_ptr, 6, payload_trade2.as_ptr(), payload_trade2.len(), std::ptr::null_mut(), 0);

        // Send depth snapshot 3 (Refill 2 -> Triggers Iceberg Alert!)
        let res = handle_fn(state_ptr, 6, payload_depth1.as_ptr(), payload_depth1.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // 5. Query DataMonitor (Endpoint 4)
        let mut out_buf = vec![0u8; 8192];
        let read_bytes = handle_fn(state_ptr, 4, std::ptr::null(), 0, out_buf.as_mut_ptr(), out_buf.len());
        assert!(read_bytes > 0);

        let output_str = String::from_utf8_lossy(&out_buf[..read_bytes]);
        assert!(output_str.contains("ICEBERG (BUZDAĞI EMİR) TESPİT PANELİ"));
        assert!(output_str.contains("ETHUSDT"));
        assert!(output_str.contains("BUY_ICEBERG"));

        // 6. Stop plugin (Endpoint 1)
        let res = handle_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // Check working -> 0
        handle_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(is_working_buf[0], 0);
    }
}
