use plugin_binance_trader::init_plugin;
use std::ffi::c_void;

#[test]
fn test_binance_trader_c_abi_lifecycle() {
    unsafe {
        let mut state_ptr: *mut c_void = std::ptr::null_mut();
        let endpoint_fn = init_plugin(&mut state_ptr);
        assert!(!state_ptr.is_null());

        // Test DataMonitor status message (endpoint 4)
        let mut monitor_buf = vec![0u8; 256];
        let len = endpoint_fn(state_ptr, 4, std::ptr::null(), 0, monitor_buf.as_mut_ptr(), monitor_buf.len());
        assert!(len > 0);

        // Test IsWorking (endpoint 2)
        let is_working_before = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(is_working_before, 0);

        // Test Start (endpoint 0)
        let start_res = endpoint_fn(state_ptr, 0, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(start_res, 0);

        let is_working_after = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(is_working_after, 1);

        // Test Inbox invalid action handling (endpoint 6)
        let inbox_msg = br#"{"action":"invalid_action","from":"test_module"}"#;
        let res = endpoint_fn(state_ptr, 6, inbox_msg.as_ptr(), inbox_msg.len(), std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        // Test Stop (endpoint 1)
        let stop_res = endpoint_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(stop_res, 0);

        let _ = Box::from_raw(state_ptr);
    }
}
