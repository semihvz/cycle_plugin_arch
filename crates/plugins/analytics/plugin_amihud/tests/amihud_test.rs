use plugin_amihud::init_plugin;
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
        let config_json = br#"{"plugin_params":{"window_ms":30000}}"#;
        let res = handle_fn(state_ptr, 0, config_json.as_ptr(), config_json.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // 3. IsWorking after Start -> 1
        let res = handle_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(res, 1);
        assert_eq!(is_working_buf[0], 1);

        // 4. Send payload to Inbox (Endpoint 6)
        // Header: 32 bytes stream_id ("stream_aggtrades") padded with 0
        let mut payload = vec![0u8; 32];
        let stream_id = b"stream_aggtrades";
        payload[..stream_id.len()].copy_from_slice(stream_id);

        let trade_data = serde_json::json!({
            "MUBARAKUSDT": {
                "trade_id": 101,
                "price": "1.25",
                "quantity": "1000.0",
                "event_time": 1700000000000u64
            }
        });
        let body = serde_json::to_vec(&trade_data).unwrap();
        payload.extend_from_slice(&body);

        let res = handle_fn(state_ptr, 6, payload.as_ptr(), payload.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // 5. Query DataMonitor (Endpoint 4)
        let mut out_buf = vec![0u8; 4096];
        let read_bytes = handle_fn(state_ptr, 4, std::ptr::null(), 0, out_buf.as_mut_ptr(), out_buf.len());
        assert!(read_bytes > 0);

        let output_str = String::from_utf8_lossy(&out_buf[..read_bytes]);
        assert!(output_str.contains("AMIHUD İLLİKİDİTE ANALİZİ"));
        assert!(output_str.contains("MUBARAKUSDT"));
        assert!(output_str.contains("1.2500"));

        // 6. Stop plugin (Endpoint 1)
        let res = handle_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // Check working -> 0
        handle_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(is_working_buf[0], 0);
    }
}
