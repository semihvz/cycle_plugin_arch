use plugin_spoofing::init_plugin;
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
        let config_json = br#"{"plugin_params":{"min_wall_usdt":50000,"max_lifespan_ms":10000,"min_cancel_ratio_pct":70}}"#;
        let res = handle_fn(state_ptr, 0, config_json.as_ptr(), config_json.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // 3. IsWorking after Start -> 1
        let res = handle_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(res, 1);
        assert_eq!(is_working_buf[0], 1);

        // 4. Send depth payload to Inbox (Endpoint 6) with stream_depth
        let mut payload1 = vec![0u8; 32];
        let stream_id = b"stream_depth";
        payload1[..stream_id.len()].copy_from_slice(stream_id);

        let depth_data1 = serde_json::json!({
            "BTCUSDT": {
                "bids": [
                    ["60000.0", "10.0"] // $600,000 USDT wall
                ],
                "asks": [
                    ["60100.0", "1.0"]
                ],
                "event_time": 1700000000000u64
            }
        });
        let body1 = serde_json::to_vec(&depth_data1).unwrap();
        payload1.extend_from_slice(&body1);

        let res = handle_fn(state_ptr, 6, payload1.as_ptr(), payload1.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // Send depth payload 2 (Wall vanished)
        let mut payload2 = vec![0u8; 32];
        payload2[..stream_id.len()].copy_from_slice(stream_id);

        let depth_data2 = serde_json::json!({
            "BTCUSDT": {
                "bids": [
                    ["59900.0", "1.0"]
                ],
                "asks": [
                    ["60100.0", "1.0"]
                ],
                "event_time": 1700000001000u64 // 1000 ms later
            }
        });
        let body2 = serde_json::to_vec(&depth_data2).unwrap();
        payload2.extend_from_slice(&body2);

        let res = handle_fn(state_ptr, 6, payload2.as_ptr(), payload2.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // 5. Query DataMonitor (Endpoint 4)
        let mut out_buf = vec![0u8; 8192];
        let read_bytes = handle_fn(state_ptr, 4, std::ptr::null(), 0, out_buf.as_mut_ptr(), out_buf.len());
        assert!(read_bytes > 0);

        let output_str = String::from_utf8_lossy(&out_buf[..read_bytes]);
        assert!(output_str.contains("BINANCE FUTURES SPOOFING (FAKE ORDER) DETECTION PANEL"));
        assert!(output_str.contains("BTCUSDT"));
        assert!(output_str.contains("BID"));

        // 6. Stop plugin (Endpoint 1)
        let res = handle_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // Check working -> 0
        handle_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(is_working_buf[0], 0);
    }
}
