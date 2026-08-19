use plugin_absorption::init_plugin;
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
        let config_json = br#"{"plugin_params":{"min_absorption_usdt":30000,"min_wall_usdt":20000}}"#;
        let res = handle_fn(state_ptr, 0, config_json.as_ptr(), config_json.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // 3. IsWorking after Start -> 1
        let res = handle_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(res, 1);
        assert_eq!(is_working_buf[0], 1);

        // 4. Send initial depth payload (Ask Wall at 100.0)
        let mut payload_depth1 = vec![0u8; 32];
        let stream_id_depth = b"stream_depth";
        payload_depth1[..stream_id_depth.len()].copy_from_slice(stream_id_depth);

        let depth_data1 = serde_json::json!({
            "ACEUSDT": {
                "bids": [
                    ["99.5", "1.0"]
                ],
                "asks": [
                    ["100.0", "1000.0"] // $100,000 USDT Ask Wall
                ],
                "event_time": 1700000000000u64
            }
        });
        let body_depth1 = serde_json::to_vec(&depth_data1).unwrap();
        payload_depth1.extend_from_slice(&body_depth1);
        handle_fn(state_ptr, 6, payload_depth1.as_ptr(), payload_depth1.len(), std::ptr::null_mut(), 0);

        // Send Market Buy trade hitting Ask wall ($40,000 executed)
        let mut payload_trade1 = vec![0u8; 32];
        let stream_id_trade = b"stream_trades";
        payload_trade1[..stream_id_trade.len()].copy_from_slice(stream_id_trade);
        let trade_data1 = serde_json::json!({
            "ACEUSDT": {
                "trade_id": 801,
                "price": "100.0",
                "quantity": "400.0", // $40,000 executed >= $30,000 min
                "buyer_is_maker": false,
                "event_time": 1700000000100u64
            }
        });
        let body_trade1 = serde_json::to_vec(&trade_data1).unwrap();
        payload_trade1.extend_from_slice(&body_trade1);
        handle_fn(state_ptr, 6, payload_trade1.as_ptr(), payload_trade1.len(), std::ptr::null_mut(), 0);

        // Send Depth Snapshot 2 -> Ask Wall survives!
        let mut payload_depth2 = vec![0u8; 32];
        payload_depth2[..stream_id_depth.len()].copy_from_slice(stream_id_depth);

        let depth_data2 = serde_json::json!({
            "ACEUSDT": {
                "bids": [
                    ["99.5", "1.0"]
                ],
                "asks": [
                    ["100.0", "600.0"] // Remaining $60,000 Ask Wall
                ],
                "event_time": 1700000000200u64
            }
        });
        let body_depth2 = serde_json::to_vec(&depth_data2).unwrap();
        payload_depth2.extend_from_slice(&body_depth2);

        let res = handle_fn(state_ptr, 6, payload_depth2.as_ptr(), payload_depth2.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // 5. Query DataMonitor (Endpoint 4)
        let mut out_buf = vec![0u8; 8192];
        let read_bytes = handle_fn(state_ptr, 4, std::ptr::null(), 0, out_buf.as_mut_ptr(), out_buf.len());
        assert!(read_bytes > 0);

        let output_str = String::from_utf8_lossy(&out_buf[..read_bytes]);
        assert!(output_str.contains("EMİLİM (ABSORPTION) VE DUVAR TEYİT PANELİ"));
        assert!(output_str.contains("ACEUSDT"));
        assert!(output_str.contains("ASK_ABSORPTION"));

        // 6. Stop plugin (Endpoint 1)
        let res = handle_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // Check working -> 0
        handle_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(is_working_buf[0], 0);
    }
}
