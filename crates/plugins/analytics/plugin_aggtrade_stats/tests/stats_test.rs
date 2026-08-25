use std::ffi::c_void;
use plugin_aggtrade_stats::init_plugin;

fn make_inbox_payload(stream_id: &str, json_data: &serde_json::Value) -> Vec<u8> {
    let mut payload = Vec::with_capacity(32 + 1024);
    let mut header = [0u8; 32];
    let bytes = stream_id.as_bytes();
    let len = bytes.len().min(32);
    header[..len].copy_from_slice(&bytes[..len]);
    payload.extend_from_slice(&header);

    let json_bytes = serde_json::to_vec(json_data).unwrap();
    payload.extend_from_slice(&json_bytes);
    payload
}

#[test]
fn test_cabi_plugin_lifecycle_and_inbox() {
    let mut state_ptr: *mut c_void = std::ptr::null_mut();
    unsafe {
        let endpoint_fn = init_plugin(&mut state_ptr);
        assert!(!state_ptr.is_null());

        // 1. IsWorking before Start
        let is_working_before = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(is_working_before, 0);

        // 2. Start (Endpoint 0)
        let res = endpoint_fn(state_ptr, 0, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(res, 0);

        let is_working_after = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(is_working_after, 1);

        // 3. Send aggtrade payload via Inbox (Endpoint 6)
        let agg_json = serde_json::json!({
            "BTCUSDT": {
                "trade_id": 5001,
                "price": "50000.0",
                "quantity": "2.0",
                "buyer_is_maker": true,
                "local_recv_time_ms": 1700000000000i64
            },
            "ETHUSDT": {
                "trade_id": 6001,
                "price": "3000.0",
                "quantity": "5.0",
                "buyer_is_maker": false,
                "local_recv_time_ms": 1700000000000i64
            },
            "ACEUSDT": {
                "trade_id": 7001,
                "price": "10.0",
                "quantity": "500.0",
                "buyer_is_maker": true,
                "local_recv_time_ms": 1700000000000i64
            }
        });

        let payload_bytes = make_inbox_payload("stream_aggtrades", &agg_json);
        let inbox_res = endpoint_fn(
            state_ptr,
            6,
            payload_bytes.as_ptr(),
            payload_bytes.len(),
            std::ptr::null_mut(),
            0,
        );
        assert_eq!(inbox_res, 0);

        // 4. Check DataMonitor output (Endpoint 4)
        let mut buf = vec![0u8; 4096];
        let monitor_len = endpoint_fn(
            state_ptr,
            4,
            std::ptr::null(),
            0,
            buf.as_mut_ptr(),
            buf.len(),
        );

        assert!(monitor_len > 0);
        let monitor_str = String::from_utf8_lossy(&buf[..monitor_len]);
        println!("DataMonitor Output:\n{}", monitor_str);

        assert!(monitor_str.contains("BTCUSDT"));
        assert!(monitor_str.contains("ETHUSDT"));
        assert!(monitor_str.contains("ACEUSDT"));
        assert!(monitor_str.contains("100000.00000000 USDT/sn")); // 50000 * 2 = 100000
        assert!(monitor_str.contains("15000.00000000 USDT/sn"));  // 3000 * 5 = 15000
        assert!(monitor_str.contains("5000.00000000 USDT/sn"));   // 10 * 500 = 5000
        assert!(monitor_str.contains("Maker/Taker Hacim"));
        assert!(monitor_str.contains("Maker/Taker Adet"));

        // 5. Stop (Endpoint 1)
        endpoint_fn(state_ptr, 1, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        let is_working_stopped = endpoint_fn(state_ptr, 2, std::ptr::null(), 0, std::ptr::null_mut(), 0);
        assert_eq!(is_working_stopped, 0);
    }
}
