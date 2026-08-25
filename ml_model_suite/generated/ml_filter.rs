// Auto-generated ML Filter Rule Engine in Pure Rust
// Compiled for microsecond execution in Cycle Orc C-ABI Plugins

#[derive(Debug, Clone, Copy)]
pub struct MLFeatures {
    pub trend_100b_pct: f64,
    pub trend_50b_pct: f64,
    pub trend_20b_pct: f64,
    pub stoch_pos_pct: f64,
    pub norm_atr_pct: f64,
    pub volatility_range_pct: f64,
    pub volume_ratio: f64,
    pub entry_hour: f64,
    pub dist_to_100low_pct: f64,
    pub last_bar_body_ratio: f64,
    pub last_bar_is_bullish: f64,
}

pub fn evaluate_ml_filter(f: &MLFeatures) -> bool {
    if f.trend_100b_pct <= -11.1611 {
        if f.dist_to_100low_pct <= 18.4769 {
            if f.stoch_pos_pct <= 20.8048 {
                if f.volume_ratio <= 0.8083 {
                    // Leaf node: WIN prob = 98.59% (0.9858555648272841/0.9999999999999999)
                    true
                } else {
                    // Leaf node: WIN prob = 90.32% (0.9031861953079554/0.9999999999999994)
                    true
                }
            } else {
                if f.trend_100b_pct <= -15.7969 {
                    // Leaf node: WIN prob = 43.59% (0.43592862935928645/1.0000000000000004)
                    false
                } else {
                    // Leaf node: WIN prob = 0.00% (0.0/1.0)
                    false
                }
            }
        } else {
            if f.norm_atr_pct <= 5.4755 {
                // Leaf node: WIN prob = 0.00% (0.0/1.0)
                false
            } else {
                // Leaf node: WIN prob = 0.00% (0.0/1.0)
                false
            }
        }
    } else {
        if f.stoch_pos_pct <= 42.9402 {
            if f.trend_20b_pct <= 2.8724 {
                if f.volatility_range_pct <= 8.3884 {
                    // Leaf node: WIN prob = 98.84% (0.9884255503272217/1.0)
                    true
                } else {
                    // Leaf node: WIN prob = 20.42% (0.2042450397633191/0.9999999999999999)
                    false
                }
            } else {
                if f.volume_ratio <= 0.6146 {
                    // Leaf node: WIN prob = 98.77% (0.987711037096589/1.0)
                    true
                } else {
                    // Leaf node: WIN prob = 76.17% (0.7617167729527279/0.9999999999999999)
                    true
                }
            }
        } else {
            if f.stoch_pos_pct <= 93.2466 {
                // Leaf node: WIN prob = 0.00% (0.0/1.0)
                false
            } else {
                // Leaf node: WIN prob = 26.41% (0.2640628838123312/1.0000000000000004)
                false
            }
        }
    }
}