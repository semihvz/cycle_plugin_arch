use plugin_atr::{calculate_atr_14, Bar};

#[test]
fn test_calculate_atr_14() {
    let mut bars = Vec::new();
    let mut price = 100.0;

    for i in 0..100 {
        let high = price + 2.0;
        let low = price - 2.0;
        let close = price + 1.0;
        let open = price;

        bars.push(Bar {
            open_time: (i * 60000) as u64,
            open,
            high,
            low,
            close,
            volume: 1000.0,
            close_time: ((i + 1) * 60000 - 1) as u64,
        });

        price += 0.5;
    }

    let metrics = calculate_atr_14("BTCUSDT", "1m", &bars, 14).expect("ATR calculation failed");
    assert_eq!(metrics.symbol, "BTCUSDT");
    assert_eq!(metrics.interval, "1m");
    assert_eq!(metrics.period, 14);
    assert_eq!(metrics.bar_count, 100);
    assert!(metrics.latest_atr > 0.0);
    assert!(metrics.latest_close > 0.0);
    assert!(metrics.atr_pct_of_close > 0.0);
}
