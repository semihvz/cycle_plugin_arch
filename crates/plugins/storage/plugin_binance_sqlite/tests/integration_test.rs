use plugin_binance_sqlite::models::*;
use plugin_binance_sqlite::storage::SqliteStorage;
use std::sync::atomic::Ordering;

#[test]
fn test_sqlite_storage_initialization_and_inserts() {
    let db_path = "test_binance_market_data.db";
    let _ = std::fs::remove_file(db_path);

    let storage = SqliteStorage::new(db_path).expect("Storage init failed");

    // Insert Mark Price
    let mark_rec = MarkPriceRecord {
        symbol: "BTCUSDT".to_string(),
        mark_price: 67000.50,
        index_price: 67010.00,
        funding_rate: 0.0001,
        next_funding_time: 1700000000000,
        event_time: 1699999999000,
        local_recv_time_ms: 1700000000123,
    };
    storage.insert_mark_price(&mark_rec).expect("Insert mark price failed");

    // Insert Best Price
    let best_rec = BestPriceRecord {
        symbol: "BTCUSDT".to_string(),
        best_bid: 67000.00,
        best_bid_qty: 1.5,
        best_ask: 67001.00,
        best_ask_qty: 2.0,
        event_time: 1699999999000,
        local_recv_time_ms: 1700000000124,
    };
    storage.insert_best_price(&best_rec).expect("Insert best price failed");

    // Insert Trade
    let trade_rec = TradeRecord {
        symbol: "BTCUSDT".to_string(),
        trade_id: 1234567,
        price: 67000.50,
        quantity: 0.25,
        buyer_is_maker: true,
        event_time: 1699999999000,
        local_recv_time_ms: 1700000000125,
    };
    storage.insert_trade(&trade_rec).expect("Insert trade failed");

    // Insert Liquidation
    let liq_rec = LiquidationRecord {
        symbol: "ETHUSDT".to_string(),
        side: "SELL".to_string(),
        order_type: "LIMIT".to_string(),
        price: 3500.00,
        average_price: 3495.00,
        original_qty: 10.0,
        filled_qty: 10.0,
        event_time: 1699999999000,
        local_recv_time_ms: 1700000000126,
    };
    storage.insert_liquidation(&liq_rec).expect("Insert liquidation failed");

    // Insert Depth
    let depth_rec = DepthRecord {
        symbol: "BTCUSDT".to_string(),
        bids_json: r#"[["67000.00","1.5"]]"#.to_string(),
        asks_json: r#"[["67001.00","2.0"]]"#.to_string(),
        last_update_id: 987654321,
        event_time: 1699999999000,
        local_recv_time_ms: 1700000000127,
    };
    storage.insert_depth(&depth_rec).expect("Insert depth failed");

    // Check stats
    assert_eq!(storage.stats.mark_price_count.load(Ordering::Relaxed), 1);
    assert_eq!(storage.stats.best_price_count.load(Ordering::Relaxed), 1);
    assert_eq!(storage.stats.trade_count.load(Ordering::Relaxed), 1);
    assert_eq!(storage.stats.liquidation_count.load(Ordering::Relaxed), 1);
    assert_eq!(storage.stats.depth_count.load(Ordering::Relaxed), 1);
    assert_eq!(storage.stats.last_insert_time_ms.load(Ordering::Relaxed), 1700000000127);

    let _ = std::fs::remove_file(db_path);
}
