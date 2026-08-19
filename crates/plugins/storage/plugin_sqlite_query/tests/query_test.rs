use plugin_sqlite_query::storage_reader::StorageReader;
use plugin_sqlite_query::init_plugin;
use rusqlite::Connection;
use serde_json::json;
use std::ffi::c_void;

#[test]
fn test_sqlite_query_engine() {
    let db_path = "test_sqlite_query.db";
    let _ = std::fs::remove_file(db_path);

    // Populate test database
    {
        let conn = Connection::open(db_path).unwrap();
        conn.execute(
            "CREATE TABLE mark_prices (id INTEGER PRIMARY KEY, symbol TEXT, mark_price REAL, local_recv_time_ms INTEGER)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO mark_prices (symbol, mark_price, local_recv_time_ms) VALUES ('BTCUSDT', 67500.50, 1700000000000)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO mark_prices (symbol, mark_price, local_recv_time_ms) VALUES ('ETHUSDT', 3500.25, 1700000000001)",
            [],
        ).unwrap();
    }

    let reader = StorageReader::new(db_path).expect("Init StorageReader failed");

    // Test execute_sql
    let res = reader.execute_sql("SELECT symbol, mark_price FROM mark_prices ORDER BY id ASC").unwrap();
    assert_eq!(res.row_count, 2);
    assert_eq!(res.columns, vec!["symbol", "mark_price"]);
    assert_eq!(res.rows[0][0], "BTCUSDT");
    assert_eq!(res.rows[0][1], "67500.5");
    assert!(res.formatted_output.contains("BTCUSDT"));
    assert!(res.formatted_output.contains("ETHUSDT"));

    // Test list_tables
    let tables = reader.list_tables().unwrap();
    assert_eq!(tables.rows.len(), 1);
    assert_eq!(tables.rows[0][0], "mark_prices");

    // Test C-ABI endpoints
    unsafe {
        let mut state_ptr: *mut c_void = std::ptr::null_mut();
        let handle_endpoint = init_plugin(&mut state_ptr);

        let start_config = json!({ "plugin_params": { "db_path": db_path } });
        let start_bytes = serde_json::to_vec(&start_config).unwrap();
        let mut buf = [0u8; 4096];
        
        handle_endpoint(state_ptr, 0, start_bytes.as_ptr(), start_bytes.len(), buf.as_mut_ptr(), buf.len());

        // Send query via Inbox (Endpoint 6)
        let q_cmd = json!({ "action": "query", "sql": "SELECT symbol, mark_price FROM mark_prices WHERE symbol='BTCUSDT'" });
        let q_bytes = serde_json::to_vec(&q_cmd).unwrap();
        let len = handle_endpoint(state_ptr, 6, q_bytes.as_ptr(), q_bytes.len(), buf.as_mut_ptr(), buf.len());
        assert!(len > 0);
        let out_str = std::str::from_utf8(&buf[..len]).unwrap();
        assert!(out_str.contains("BTCUSDT"));

        // Check Outbox (Endpoint 7)
        let out_len = handle_endpoint(state_ptr, 7, std::ptr::null(), 0, buf.as_mut_ptr(), buf.len());
        assert!(out_len > 0);
    }

    let _ = std::fs::remove_file(db_path);
}
