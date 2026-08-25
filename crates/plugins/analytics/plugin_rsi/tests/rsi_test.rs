use plugin_rsi::{calculate_rsi_14, Bar};

#[test]
fn test_rsi_1440_bars_uptrend() {
    let mut bars = Vec::with_capacity(1440);
    let mut price = 100.0;
    for i in 0..1440 {
        price += 1.0;
        bars.push(Bar {
            open_time: (i * 60000) as u64,
            open: price - 0.5,
            high: price + 0.5,
            low: price - 1.0,
            close: price,
            volume: 10.0,
            close_time: (i * 60000 + 59999) as u64,
        });
    }

    let metrics = calculate_rsi_14("BTCUSDT", "1m", &bars, 14).expect("RSI calculation failed");
    assert_eq!(metrics.bar_count, 1440);
    assert_eq!(metrics.symbol, "BTCUSDT");
    assert_eq!(metrics.interval, "1m");
    assert_eq!(metrics.period, 14);
    assert_eq!(metrics.state, "OVERBOUGHT");
    assert!((metrics.latest_rsi - 100.0).abs() < 1e-4);
}

#[test]
fn test_rsi_1440_bars_downtrend() {
    let mut bars = Vec::with_capacity(1440);
    let mut price = 2000.0;
    for i in 0..1440 {
        price -= 1.0;
        bars.push(Bar {
            open_time: (i * 60000) as u64,
            open: price + 0.5,
            high: price + 1.0,
            low: price - 0.5,
            close: price,
            volume: 10.0,
            close_time: (i * 60000 + 59999) as u64,
        });
    }

    let metrics = calculate_rsi_14("ETHUSDT", "1m", &bars, 14).expect("RSI calculation failed");
    assert_eq!(metrics.bar_count, 1440);
    assert_eq!(metrics.state, "OVERSOLD");
    assert!((metrics.latest_rsi - 0.0).abs() < 1e-4);
}

#[test]
fn test_rsi_1440_bars_oscillating() {
    let mut bars = Vec::with_capacity(1440);
    let mut price = 100.0;
    for i in 0..1440 {
        if i % 2 == 0 {
            price += 2.0;
        } else {
            price -= 2.0;
        }
        bars.push(Bar {
            open_time: (i * 60000) as u64,
            open: price,
            high: price + 1.0,
            low: price - 1.0,
            close: price,
            volume: 50.0,
            close_time: (i * 60000 + 59999) as u64,
        });
    }

    let metrics = calculate_rsi_14("TACUSDT", "1m", &bars, 14).expect("RSI calculation failed");
    assert_eq!(metrics.bar_count, 1440);
    assert!(metrics.latest_rsi >= 0.0 && metrics.latest_rsi <= 100.0);
    assert!((metrics.latest_rsi - 50.0).abs() < 5.0);
}
