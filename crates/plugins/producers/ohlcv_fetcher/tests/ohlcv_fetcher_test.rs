use plugin_ohlcv_fetcher::init_plugin;
use std::ffi::c_void;

#[test]
fn test_ohlcv_fetcher_c_abi_lifecycle() {
    unsafe {
        let mut state_ptr: *mut c_void = std::ptr::null_mut();
        let endpoint_fn = init_plugin(&mut state_ptr);
        assert!(!state_ptr.is_null());

        // Test IsWorking (endpoint 2)
        let is_working_before = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(is_working_before, 0);

        // Test Start (endpoint 0)
        let start_res = endpoint_fn(state_ptr, 0, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(start_res, 0);

        let is_working_after = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(is_working_after, 1);

        // Test Inbox config subscription (endpoint 6)
        let inbox_payload = br#"{"stream_id":"test_15m","symbol":"BTCUSDT","interval":"15m","limit":10,"mode":"none"}"#;
        let inbox_res = endpoint_fn(
            state_ptr,
            6,
            inbox_payload.as_ptr(),
            inbox_payload.len(),
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(inbox_res, 0);

        // Test DataValid (endpoint 3)
        let valid_res = endpoint_fn(state_ptr, 3, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(valid_res, 1);

        // Test Stop (endpoint 1)
        let stop_res = endpoint_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(stop_res, 0);

        let _ = Box::from_raw(state_ptr);
    }
}
