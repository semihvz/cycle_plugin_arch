use plugin_aggtrade_ohlcv::OhlcvEngine;
use serde_json::json;

#[test]
fn test_1s_ohlcv_candle_aggregation() {
    let engine = OhlcvEngine::new();
    let base_ms = 1700000000000u64; // Second 1700000000

    // Trade 1: Open @ 50000.0
    engine.process_trade("BTCUSDT", 1, 50000.0, 1.0, false, base_ms);
    // Trade 2: High @ 50200.0
    engine.process_trade("BTCUSDT", 2, 50200.0, 2.0, true, base_ms + 200);
    // Trade 3: Low @ 49800.0
    engine.process_trade("BTCUSDT", 3, 49800.0, 1.5, false, base_ms + 500);
    // Trade 4: Close @ 50100.0
    engine.process_trade("BTCUSDT", 4, 50100.0, 0.5, true, base_ms + 800);

    let active_guard = engine.active_candles.lock().unwrap();
    let btc_candle = active_guard.get("BTCUSDT").expect("BTCUSDT mum olusturulmali");

    assert_eq!(btc_candle.symbol, "BTCUSDT");
    assert_eq!(btc_candle.timestamp_sec, 1700000000);
    assert_eq!(btc_candle.open, 50000.0);
    assert_eq!(btc_candle.high, 50200.0);
    assert_eq!(btc_candle.low, 49800.0);
    assert_eq!(btc_candle.close, 50100.0);
    assert_eq!(btc_candle.volume, 5.0);
    assert_eq!(btc_candle.trades_count, 4);
    assert_eq!(btc_candle.buy_volume, 2.5); // Trade 1 (1.0) + Trade 3 (1.5)
    assert_eq!(btc_candle.sell_volume, 2.5); // Trade 2 (2.0) + Trade 4 (0.5)
}

#[test]
fn test_candle_transition_on_new_second() {
    let engine = OhlcvEngine::new();
    let sec1_ms = 1700000000000u64;
    let sec2_ms = 1700000001000u64;

    // Trades in second 1
    engine.process_trade("ETHUSDT", 100, 3000.0, 10.0, false, sec1_ms);
    engine.process_trade("ETHUSDT", 101, 3050.0, 5.0, true, sec1_ms + 500);

    // Trade in second 2 -> triggers completion of second 1 candle
    engine.process_trade("ETHUSDT", 102, 3020.0, 2.0, false, sec2_ms);

    let active_guard = engine.active_candles.lock().unwrap();
    let completed_guard = engine.completed_candles.lock().unwrap();

    let active = active_guard.get("ETHUSDT").unwrap();
    assert_eq!(active.timestamp_sec, 1700000001);
    assert_eq!(active.open, 3020.0);

    let history = completed_guard.get("ETHUSDT").unwrap();
    assert_eq!(history.len(), 1);
    let prev = &history[0];
    assert_eq!(prev.timestamp_sec, 1700000000);
    assert_eq!(prev.open, 3000.0);
    assert_eq!(prev.high, 3050.0);
    assert_eq!(prev.close, 3050.0);
}

#[test]
fn test_aggtrade_payload_processing() {
    let engine = OhlcvEngine::new();
    let now_ms = 1700000000000u64;

    let payload = json!({
        "BTCUSDT": {
            "trade_id": 999,
            "price": "60000.5",
            "quantity": "2.5",
            "buyer_is_maker": false,
            "event_time": now_ms
        }
    });

    let report = engine.process_aggtrade_payload(&payload, now_ms);

    assert!(report.contains("SANİYELİK OHLCV ÇUBUKLARI"));
    assert!(report.contains("BTCUSDT"));
    assert!(report.contains("60000.5000"));
    assert!(report.contains("2.5000"));
}
