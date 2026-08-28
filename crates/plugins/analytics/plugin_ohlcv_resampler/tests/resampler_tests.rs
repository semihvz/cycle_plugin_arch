use plugin_ohlcv_resampler::{
    align_timestamp_sec, interval_to_seconds, resample_bars, Bar, OhlcvResamplerEngine,
};

fn create_sample_1m_bars(start_time_sec: u64, count: usize, base_price: f64) -> Vec<Bar> {
    let mut bars = Vec::with_capacity(count);
    for i in 0..count {
        let t = start_time_sec + (i as u64 * 60);
        let offset = (i as f64) * 0.5;
        let open = base_price + offset;
        let high = open + 2.0;
        let low = open - 1.5;
        let close = open + 1.0;
        let volume = 10.0 + (i as f64);

        bars.push(Bar {
            open_time: t,
            open,
            high,
            low,
            close,
            volume,
            quote_volume: Some(close * volume),
            trades_count: Some(5),
            buy_volume: Some(volume * 0.6),
            sell_volume: Some(volume * 0.4),
            close_time: Some(t + 59),
        });
    }
    bars
}

#[test]
fn test_interval_conversions() {
    assert_eq!(interval_to_seconds("1m"), 60);
    assert_eq!(interval_to_seconds("3m"), 180);
    assert_eq!(interval_to_seconds("5m"), 300);
    assert_eq!(interval_to_seconds("15m"), 900);
    assert_eq!(interval_to_seconds("30m"), 1800);
    assert_eq!(interval_to_seconds("1h"), 3600);
    assert_eq!(interval_to_seconds("4h"), 14400);
    assert_eq!(interval_to_seconds("1d"), 86400);
}

#[test]
fn test_timestamp_alignment() {
    // 10:07:30 UTC -> 10:00:00 for 15m bucket
    let timestamp = 1600000450;
    let aligned_15m = align_timestamp_sec(timestamp, 900);
    assert_eq!(aligned_15m % 900, 0);

    let aligned_1h = align_timestamp_sec(timestamp, 3600);
    assert_eq!(aligned_1h % 3600, 0);

    let aligned_4h = align_timestamp_sec(timestamp, 14400);
    assert_eq!(aligned_4h % 14400, 0);
}

#[test]
fn test_1m_to_15m_resampling_accuracy() {
    let start_ts = 1700000100; // Aligned to 15m boundary (1700000100 % 900 == 0)
    assert_eq!(start_ts % 900, 0);

    let bars_1m = create_sample_1m_bars(start_ts, 15, 100.0);

    let resampled = resample_bars("BTCUSDT", "15m", &bars_1m);
    assert_eq!(resampled.len(), 1);

    let c = &resampled[0];
    assert_eq!(c.symbol, "BTCUSDT");
    assert_eq!(c.target_interval, "15m");
    assert_eq!(c.open_time, start_ts);
    assert_eq!(c.close_time, start_ts + 899);
    assert_eq!(c.bar_count, 15);

    // Open should match first 1m bar's open
    assert_eq!(c.open, bars_1m[0].open);
    // Close should match last 1m bar's close
    assert_eq!(c.close, bars_1m[14].close);

    // High should be max high across all 15 bars
    let expected_max_high = bars_1m.iter().map(|b| b.high).fold(f64::MIN, f64::max);
    assert_eq!(c.high, expected_max_high);

    // Low should be min low across all 15 bars
    let expected_min_low = bars_1m.iter().map(|b| b.low).fold(f64::MAX, f64::min);
    assert_eq!(c.low, expected_min_low);

    // Volume should be exact sum of all 15 bar volumes
    let expected_vol: f64 = bars_1m.iter().map(|b| b.volume).sum();
    assert!((c.volume - expected_vol).abs() < 1e-6);

    // Trades count sum
    assert_eq!(c.trades_count, 15 * 5);
}

#[test]
fn test_1m_to_1h_and_4h_resampling() {
    let start_ts = 1700006400; // Aligned to 4h boundary
    assert_eq!(start_ts % 14400, 0);

    let bars_1m = create_sample_1m_bars(start_ts, 240, 500.0); // 240 mins = 4 hours

    let resampled_1h = resample_bars("ETHUSDT", "1h", &bars_1m);
    assert_eq!(resampled_1h.len(), 4);
    for h in &resampled_1h {
        assert_eq!(h.bar_count, 60);
    }

    let resampled_4h = resample_bars("ETHUSDT", "4h", &bars_1m);
    assert_eq!(resampled_4h.len(), 1);
    assert_eq!(resampled_4h[0].bar_count, 240);
    assert_eq!(resampled_4h[0].open, bars_1m[0].open);
    assert_eq!(resampled_4h[0].close, bars_1m[239].close);
}

#[test]
fn test_resampler_engine_streaming() {
    let engine = OhlcvResamplerEngine::new(100);
    let start_ts = 1700000100; // Aligned to 15m boundary
    let bars_1m = create_sample_1m_bars(start_ts, 35, 1000.0); // 35 bars: 15 + 15 + 5

    let target_tfs = vec!["15m".to_string(), "1h".to_string()];

    for bar in bars_1m {
        engine.process_bar("SOLUSDT", &bar, &target_tfs);
    }

    let active_guard = engine.active_candles.lock().unwrap();

    // Should have active candles for SOLUSDT in both 15m and 1h
    let sol_15m = active_guard.get(&("SOLUSDT".to_string(), "15m".to_string()));
    assert!(sol_15m.is_some());
    assert_eq!(sol_15m.unwrap().bar_count, 5); // 35 % 15 = 5 in the 3rd 15m bucket

    let sol_1h = active_guard.get(&("SOLUSDT".to_string(), "1h".to_string()));
    assert!(sol_1h.is_some());
    assert_eq!(sol_1h.unwrap().bar_count, 35);
}
