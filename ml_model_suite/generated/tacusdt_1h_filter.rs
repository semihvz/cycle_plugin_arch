// Auto-generated Zero-Latency C/Rust ML Filter for TACUSDT 1h Collector Data
#[derive(Debug, Clone, Copy)]
pub struct TACUSDT1hMLFeatures {
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

pub fn evaluate_tacusdt_1h_filter(f: &TACUSDT1hMLFeatures) -> (bool, f64) {
    if f.trend_100b_pct <= -11.13245 {
        if f.dist_to_100low_pct <= 18.41757 {
            if f.stoch_pos_pct <= 20.42614 {
                if f.norm_atr_pct <= 19.18923 {
                    return (true, 0.9669);
                } else {
                    return (false, 0.0000);
                }
            } else {
                if f.volume_ratio <= 0.28028 {
                    return (true, 1.0000);
                } else {
                    return (false, 0.0000);
                }
            }
        } else {
            if f.norm_atr_pct <= 4.58447 {
                return (false, 0.0000);
            } else {
                return (false, 0.0000);
            }
        }
    } else {
        if f.stoch_pos_pct <= 42.69065 {
            if f.trend_20b_pct <= 2.54550 {
                if f.volatility_range_pct <= 8.25061 {
                    return (true, 0.9884);
                } else {
                    return (false, 0.1763);
                }
            } else {
                if f.norm_atr_pct <= 1.80419 {
                    return (false, 0.0000);
                } else {
                    return (true, 0.9355);
                }
            }
        } else {
            if f.last_bar_body_ratio <= 0.95785 {
                return (false, 0.0000);
            } else {
                if f.trend_50b_pct <= 12.01804 {
                    return (false, 0.0000);
                } else {
                    return (true, 1.0000);
                }
            }
        }
    }
}