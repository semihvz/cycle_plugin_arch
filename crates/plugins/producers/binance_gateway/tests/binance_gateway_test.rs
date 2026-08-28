use plugin_binance_gateway::init_plugin;
use std::ffi::c_void;

#[test]
fn test_binance_gateway_c_abi_lifecycle() {
    unsafe {
        let mut state_ptr: *mut c_void = std::ptr::null_mut();
        let endpoint_fn = init_plugin(&mut state_ptr);

        assert!(!state_ptr.is_null());

        // Test IsWorking endpoint (endpoint 2)
        let mut is_working_buf = [0u8; 1];
        let len = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(len, 1);
        assert_eq!(is_working_buf[0], 0); // Not working yet

        // Test Start endpoint (endpoint 0)
        let config_payload = br#"{"plugin_params":{"symbols":["BTCUSDT"]}}"#;
        let start_res = endpoint_fn(
            state_ptr,
            0,
            config_payload.as_ptr(),
            config_payload.len(),
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(start_res, 0);

        // Check IsWorking again
        let len = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, is_working_buf.as_mut_ptr(), 1);
        assert_eq!(len, 1);
        assert_eq!(is_working_buf[0], 1); // Now working

        // Test DataMonitor / RawData endpoint (endpoint 5)
        let mut data_buf = vec![0u8; 1024];
        let data_len = endpoint_fn(state_ptr, 5, std::ptr::null(), 0, data_buf.as_mut_ptr(), data_buf.len());
        assert!(data_len > 0);

        let data_str = std::str::from_utf8(&data_buf[..data_len]).unwrap();
        assert!(data_str.contains("stream_markprice"));
        assert!(data_str.contains("stream_bestprice"));

        // Test Stop endpoint (endpoint 1)
        let stop_res = endpoint_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(stop_res, 0);

        // Clean up Box state
        let _ = Box::from_raw(state_ptr);
    }
}
