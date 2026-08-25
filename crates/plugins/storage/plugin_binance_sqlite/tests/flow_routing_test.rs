use plugin_binance_sqlite::init_plugin;
use rusqlite::Connection;
use serde_json::json;
use std::ffi::c_void;

#[test]
fn test_cabi_and_flow_routing_to_sqlite() {
    let db_path = "test_flow_market_data.db";
    let _ = std::fs::remove_file(db_path);

    unsafe {
        let mut state_ptr: *mut c_void = std::ptr::null_mut();
        let handle_endpoint = init_plugin(&mut state_ptr);

        // 1. Start plugin with custom DB path
        let start_config = json!({
            "plugin_params": {
                "db_path": db_path
            }
        });
        let config_bytes = serde_json::to_vec(&start_config).unwrap();
        let mut out_buf = [0u8; 1024];

        let ret = handle_endpoint(state_ptr, 0, config_bytes.as_ptr(), config_bytes.len(), out_buf.as_mut_ptr(), out_buf.len());
        assert_eq!(ret, 0);

        // Verify IsWorking (Endpoint 2)
        let is_working = handle_endpoint(state_ptr, 2, std::ptr::null(), 0, out_buf.as_mut_ptr(), out_buf.len());
        assert_eq!(is_working, 1);

        // 2. Simulate stream_markprice from FlowEngine (32-byte header + JSON payload)
        let markprice_data = json!({
            "BTCUSDT": {
                "mark_price": "68500.25",
                "index_price": "68505.00",
                "funding_rate": "0.00015",
                "next_funding_time": 1700000000000i64,
                "event_time": 1699999999000i64,
                "local_recv_time_ms": 1700000000500i64
            }
        });
        let payload_bytes = make_flow_payload("stream_markprice", &markprice_data);
        handle_endpoint(state_ptr, 6, payload_bytes.as_ptr(), payload_bytes.len(), out_buf.as_mut_ptr(), out_buf.len());

        // 3. Simulate stream_bestprice
        let bestprice_data = json!({
            "BTCUSDT": {
                "best_bid": "68500.00",
                "best_bid_qty": "3.5",
                "best_ask": "68500.50",
                "best_ask_qty": "1.2",
                "event_time": 1699999999000i64,
                "local_recv_time_ms": 1700000000501i64
            }
        });
        let payload_bytes = make_flow_payload("stream_bestprice", &bestprice_data);
        handle_endpoint(state_ptr, 6, payload_bytes.as_ptr(), payload_bytes.len(), out_buf.as_mut_ptr(), out_buf.len());

        // 4. Simulate stream_trades
        let trades_data = json!({
            "BTCUSDT": {
                "trade_id": 888999,
                "price": "68500.25",
                "quantity": "0.5",
                "buyer_is_maker": false,
                "event_time": 1699999999000i64,
                "local_recv_time_ms": 1700000000502i64
            }
        });
        let payload_bytes = make_flow_payload("stream_trades", &trades_data);
        handle_endpoint(state_ptr, 6, payload_bytes.as_ptr(), payload_bytes.len(), out_buf.as_mut_ptr(), out_buf.len());

        // 4b. Simulate stream_aggtrades
        let aggtrades_data = json!({
            "ETHUSDT": {
                "trade_id": 88899910,
                "price": "3550.00",
                "quantity": "2.5",
                "buyer_is_maker": true,
                "event_time": 1699999999000i64,
                "local_recv_time_ms": 1700000000502i64
            }
        });
        let payload_bytes_agg = make_flow_payload("stream_aggtrades", &aggtrades_data);
        handle_endpoint(state_ptr, 6, payload_bytes_agg.as_ptr(), payload_bytes_agg.len(), out_buf.as_mut_ptr(), out_buf.len());

        // 5. Simulate stream_liquidations
        let liq_data = json!([
            {
                "symbol": "BTCUSDT",
                "side": "BUY",
                "type": "LIMIT",
                "price": "69000.00",
                "average_price": "68995.00",
                "original_qty": "2.0",
                "filled_qty": "2.0",
                "event_time": 1699999999000i64,
                "local_recv_time_ms": 1700000000503i64
            }
        ]);
        let payload_bytes = make_flow_payload("stream_liquidations", &liq_data);
        handle_endpoint(state_ptr, 6, payload_bytes.as_ptr(), payload_bytes.len(), out_buf.as_mut_ptr(), out_buf.len());

        // 6. Simulate stream_depth
        let depth_data = json!({
            "BTCUSDT": {
                "bids": [["68500.00", "3.5"]],
                "asks": [["68500.50", "1.2"]],
                "last_update_id": 555666777,
                "event_time": 1699999999000i64,
                "local_recv_time_ms": 1700000000504i64
            }
        });
        let payload_bytes = make_flow_payload("stream_depth", &depth_data);
        handle_endpoint(state_ptr, 6, payload_bytes.as_ptr(), payload_bytes.len(), out_buf.as_mut_ptr(), out_buf.len());

        // 7. Inspect DataMonitor report (Endpoint 4)
        let mut monitor_buf = vec![0u8; 4096];
        let report_len = handle_endpoint(state_ptr, 4, std::ptr::null(), 0, monitor_buf.as_mut_ptr(), monitor_buf.len());
        assert!(report_len > 0);
        let report_str = std::str::from_utf8(&monitor_buf[..report_len]).unwrap();
        println!("DataMonitor output:\n{}", report_str);
        assert!(report_str.contains("BINANCE SQLITE RECORDER STATUS"));
        assert!(report_str.contains("Mark Price Records: 1"));
        assert!(report_str.contains("Best Price Records: 1"));
        assert!(report_str.contains("Trade Records: 2"));
        assert!(report_str.contains("Liquidation Records: 1"));
        assert!(report_str.contains("Depth Records: 1"));

        // 8. Stop plugin (Endpoint 1)
        handle_endpoint(state_ptr, 1, std::ptr::null(), 0, out_buf.as_mut_ptr(), out_buf.len());
        let is_working_after = handle_endpoint(state_ptr, 2, std::ptr::null(), 0, out_buf.as_mut_ptr(), out_buf.len());
        assert_eq!(is_working_after, 0);
    }

    // 9. Query SQLite file directly to verify data persistence
    let conn = Connection::open(db_path).expect("Could not open test DB");
    
    let mark_count: i64 = conn.query_row("SELECT COUNT(*) FROM mark_prices", [], |r| r.get(0)).unwrap();
    let best_count: i64 = conn.query_row("SELECT COUNT(*) FROM best_prices", [], |r| r.get(0)).unwrap();
    let trade_count: i64 = conn.query_row("SELECT COUNT(*) FROM trades", [], |r| r.get(0)).unwrap();
    let liq_count: i64 = conn.query_row("SELECT COUNT(*) FROM liquidations", [], |r| r.get(0)).unwrap();
    let depth_count: i64 = conn.query_row("SELECT COUNT(*) FROM depth", [], |r| r.get(0)).unwrap();

    assert_eq!(mark_count, 1);
    assert_eq!(best_count, 1);
    assert_eq!(trade_count, 2);
    assert_eq!(liq_count, 1);
    assert_eq!(depth_count, 1);

    let (symbol, mark_p): (String, f64) = conn.query_row(
        "SELECT symbol, mark_price FROM mark_prices LIMIT 1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?))
    ).unwrap();
    assert_eq!(symbol, "BTCUSDT");
    assert_eq!(mark_p, 68500.25);

    let _ = std::fs::remove_file(db_path);
}

fn make_flow_payload(stream_id: &str, json_val: &serde_json::Value) -> Vec<u8> {
    let mut header = [0u8; 32];
    let bytes = stream_id.as_bytes();
    let len = bytes.len().min(32);
    header[..len].copy_from_slice(&bytes[..len]);

    let data_bytes = serde_json::to_vec(json_val).unwrap();
    let mut payload = Vec::with_capacity(32 + data_bytes.len());
    payload.extend_from_slice(&header);
    payload.extend_from_slice(&data_bytes);
    payload
}
