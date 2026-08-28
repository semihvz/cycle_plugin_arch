use plugin_tacusdt_1h::{calculate_atr_series, process_and_persist_tacusdt_1h, Bar};
use rusqlite::Connection;

#[test]
fn test_calculate_atr_series() {
    let mut bars = Vec::new();
    let mut base_time = 1700000000000u64;

    for i in 0..20 {
        bars.push(Bar {
            open_time: base_time,
            open: 100.0 + (i as f64),
            high: 105.0 + (i as f64),
            low: 98.0 + (i as f64),
            close: 102.0 + (i as f64),
            volume: 1000.0,
            close_time: base_time + 3600000,
        });
        base_time += 3600000;
    }

    let atr = calculate_atr_series(&bars, 14);
    assert_eq!(atr.len(), 20);
    // Initial period - 1 entries should be 0.0
    for val in &atr[..13] {
        assert_eq!(*val, 0.0);
    }
    // Index 13 (14th element) should be non-zero ATR
    assert!(atr[13] > 0.0);
}

#[test]
fn test_process_and_persist_tacusdt_1h() {
    let temp_db_file = std::env::temp_dir().join(format!("test_tacusdt_1h_{}.db", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let db_path = temp_db_file.to_str().unwrap();

    let mut bars = Vec::new();
    let mut base_time = 1700000000000u64;

    // Generate 120 bars to exceed lookback (100)
    for i in 0..120 {
        let price = 1.0 + (i as f64) * 0.01;
        bars.push(Bar {
            open_time: base_time,
            open: price,
            high: price + 0.05,
            low: price - 0.02,
            close: price + 0.03,
            volume: 5000.0,
            close_time: base_time + 3600000,
        });
        base_time += 3600000;
    }

    let status = process_and_persist_tacusdt_1h("TACUSDT", "1h", &bars, db_path);
    assert_eq!(status.symbol, "TACUSDT");
    assert_eq!(status.interval, "1h");
    assert_eq!(status.total_bars_fetched, 120);

    // Verify SQLite table creation and data persistence
    let conn = Connection::open(db_path).unwrap();
    let trades_count: i64 = conn.query_row("SELECT count(*) FROM closed_trades;", [], |r| r.get(0)).unwrap();
    let lookback_count: i64 = conn.query_row("SELECT count(*) FROM trade_lookback_bars;", [], |r| r.get(0)).unwrap();

    assert_eq!(trades_count as usize, status.total_trades_detected);
    assert_eq!(lookback_count as usize, status.total_lookback_bars_persisted);

    let _ = std::fs::remove_file(temp_db_file);
}
