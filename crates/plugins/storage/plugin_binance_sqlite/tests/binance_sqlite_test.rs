use plugin_binance_sqlite::storage::SqliteStorage;
use plugin_binance_sqlite::models::{MarkPriceRecord, BestPriceRecord, TradeRecord, LiquidationRecord, DepthRecord};

#[test]
fn test_binance_sqlite_storage_operations() {
    let temp_db_file = std::env::temp_dir().join(format!("test_binance_sqlite_{}.db", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let db_path = temp_db_file.to_str().unwrap();

    let storage = SqliteStorage::new(db_path).expect("Failed to initialize SqliteStorage");

    // Insert MarkPriceRecord
    let mark_rec = MarkPriceRecord {
        symbol: "BTCUSDT".to_string(),
        mark_price: 50000.0,
        index_price: 50005.0,
        funding_rate: 0.0001,
        next_funding_time: 1700000000000,
        event_time: 1700000000000,
        local_recv_time_ms: 1700000000005,
    };
    storage.insert_mark_price(&mark_rec).expect("Failed mark price insert");

    // Insert BestPriceRecord
    let best_rec = BestPriceRecord {
        symbol: "BTCUSDT".to_string(),
        best_bid: 49999.0,
        best_bid_qty: 1.5,
        best_ask: 50001.0,
        best_ask_qty: 2.0,
        event_time: 1700000000000,
        local_recv_time_ms: 1700000000005,
    };
    storage.insert_best_price(&best_rec).expect("Failed best price insert");

    // Insert TradeRecord
    let trade_rec = TradeRecord {
        symbol: "BTCUSDT".to_string(),
        trade_id: 1001,
        price: 50000.0,
        quantity: 0.5,
        buyer_is_maker: false,
        event_time: 1700000000000,
        local_recv_time_ms: 1700000000005,
    };
    storage.insert_trade(&trade_rec).expect("Failed trade insert");

    // Insert LiquidationRecord
    let liq_rec = LiquidationRecord {
        symbol: "BTCUSDT".to_string(),
        side: "SELL".to_string(),
        order_type: "LIMIT".to_string(),
        price: 49000.0,
        average_price: 48950.0,
        original_qty: 10.0,
        filled_qty: 10.0,
        event_time: 1700000000000,
        local_recv_time_ms: 1700000000005,
    };
    storage.insert_liquidation(&liq_rec).expect("Failed liquidation insert");

    // Verify stats atomic counters
    assert_eq!(storage.stats.mark_price_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(storage.stats.best_price_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(storage.stats.trade_count.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(storage.stats.liquidation_count.load(std::sync::atomic::Ordering::Relaxed), 1);

    // Verify file size is greater than 0
    assert!(storage.get_file_size_bytes() > 0);

    let _ = std::fs::remove_file(temp_db_file);
}
