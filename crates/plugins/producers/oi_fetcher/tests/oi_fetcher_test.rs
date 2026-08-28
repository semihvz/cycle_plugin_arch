use plugin_oi_fetcher::init_plugin;
use std::ffi::c_void;

#[test]
fn test_oi_fetcher_c_abi_lifecycle() {
    unsafe {
        let mut state_ptr: *mut c_void = std::ptr::null_mut();
        let endpoint_fn = init_plugin(&mut state_ptr);
        assert!(!state_ptr.is_null());

        // Test DataMonitor initial text (endpoint 4)
        let mut monitor_buf = vec![0u8; 100];
        let len = endpoint_fn(state_ptr, 4, std::ptr::null(), 0, monitor_buf.as_mut_ptr(), monitor_buf.len());
        assert!(len > 0);
        let monitor_text = std::str::from_utf8(&monitor_buf[..len]).unwrap();
        assert!(monitor_text.contains("Hazir"));

        // Test Inbox action fetch_oi (endpoint 6)
        let req_json = br#"{"action":"fetch_oi","symbol":"BTCUSDT","interval":"5m","limit":5,"from":"test_requester"}"#;
        let inbox_res = endpoint_fn(
            state_ptr,
            6,
            req_json.as_ptr(),
            req_json.len(),
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(inbox_res, 0);

        let _ = Box::from_raw(state_ptr);
    }
}
