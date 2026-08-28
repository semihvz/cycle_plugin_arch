// Auto-generated Zero-Latency C/Rust ML Filter for MAGMAUSDT 1m Data
#[derive(Debug, Clone, Copy)]
pub struct MagmaUSDT1mMLFeatures {
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

pub fn evaluate_magmausdt_1m_filter(f: &MagmaUSDT1mMLFeatures) -> (bool, f64) {
    if f.volatility_range_pct <= 4.28630 {
        if f.volatility_range_pct <= 0.79585 {
            if f.trend_100b_pct <= -0.15505 {
                if f.entry_hour <= 6.50000 {
                    return (false, 0.1024);
                } else {
                    return (true, 0.5282);
                }
            } else {
                if f.entry_hour <= 21.50000 {
                    return (true, 0.8709);
                } else {
                    return (false, 0.4021);
                }
            }
        } else {
            if f.trend_100b_pct <= 0.42085 {
                if f.norm_atr_pct <= 0.09715 {
                    return (true, 0.5790);
                } else {
                    return (false, 0.4112);
                }
            } else {
                if f.norm_atr_pct <= 0.15745 {
                    return (false, 0.3787);
                } else {
                    return (true, 0.5301);
                }
            }
        }
    } else {
        if f.dist_to_100low_pct <= 5.48760 {
            if f.entry_hour <= 0.50000 {
                if f.trend_50b_pct <= -2.35075 {
                    return (true, 0.7287);
                } else {
                    return (false, 0.0757);
                }
            } else {
                if f.volume_ratio <= 0.49065 {
                    return (false, 0.4897);
                } else {
                    return (true, 0.6198);
                }
            }
        } else {
            if f.volatility_range_pct <= 12.88430 {
                if f.entry_hour <= 13.50000 {
                    return (true, 0.6889);
                } else {
                    return (true, 0.8573);
                }
            } else {
                if f.entry_hour <= 2.50000 {
                    return (true, 1.0000);
                } else {
                    return (false, 0.3811);
                }
            }
        }
    }
}