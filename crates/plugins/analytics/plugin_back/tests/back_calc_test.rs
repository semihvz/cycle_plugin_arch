use plugin_back::{calculate_atr_series, Bar};

#[test]
fn test_plugin_back_calculate_atr_series() {
    let mut bars = Vec::new();
    let mut base_time = 1700000000000u64;

    for i in 0..30 {
        bars.push(Bar {
            open_time: base_time,
            open: 50.0 + (i as f64) * 0.5,
            high: 52.0 + (i as f64) * 0.5,
            low: 49.0 + (i as f64) * 0.5,
            close: 51.0 + (i as f64) * 0.5,
            volume: 2000.0,
            close_time: base_time + 900000,
        });
        base_time += 900000;
    }

    let atr = calculate_atr_series(&bars, 14);
    assert_eq!(atr.len(), 30);
    assert_eq!(atr[0], 0.0);
    assert!(atr[13] > 0.0);
    assert!(atr[29] > 0.0);
}
