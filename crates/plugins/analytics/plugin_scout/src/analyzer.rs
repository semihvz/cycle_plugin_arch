use std::cmp::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config;
use crate::models::{MarketState, Opportunity, SymbolState, Verdict};

fn now_ts() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before epoch")
        .as_secs_f64()
}

pub struct OrderbookFluxAnalyzer;

impl OrderbookFluxAnalyzer {
    pub fn new() -> Self {
        Self
    }

    pub fn get_depth_candidates(&self, market: &mut MarketState) -> Vec<String> {
        let now = now_ts();
        let mut scored: Vec<(f64, String)> = Vec::new();

        for (symbol, state) in market.states.iter_mut() {
            state.refresh(now);
            if !state.is_recent(now) {
                continue;
            }
            let score = state.price_score();
            if score <= 0.0 {
                continue;
            }
            scored.push((score, symbol.clone()));
        }

        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));
        scored
            .into_iter()
            .take(config::DEPTH_CANDIDATE_COUNT)
            .map(|(_, symbol)| symbol)
            .collect()
    }

    pub fn get_best_opportunity(&self, market: &mut MarketState) -> Option<Opportunity> {
        let now = now_ts();
        let mut best_strong: Option<Opportunity> = None;
        let mut best_good: Option<Opportunity> = None;

        let depth_symbols: Vec<String> = market.depth_symbols.iter().cloned().collect();

        for symbol in depth_symbols {
            let state = market.states.get_mut(&symbol);
            let Some(state) = state else { continue };
            state.refresh(now);
            if !state.is_recent(now) {
                continue;
            }

            let Some(opp) = self.calc_opportunity(&symbol, state) else {
                continue;
            };

            match opp.verdict {
                Verdict::Guclu => {
                    if best_strong.is_none() || opp.score > best_strong.as_ref().unwrap().score {
                        best_strong = Some(opp);
                    }
                }
                Verdict::Iyi => {
                    if best_good.is_none() || opp.score > best_good.as_ref().unwrap().score {
                        best_good = Some(opp);
                    }
                }
                _ => {}
            }
        }

        best_strong.or(best_good)
    }

    fn calc_opportunity(&self, symbol: &str, state: &SymbolState) -> Option<Opportunity> {
        if state.mid <= 0.0 || state.spread_bps <= 0.0 {
            return None;
        }
        if state.price_ticks_per_s() < config::MIN_TICKS_PER_SECOND {
            return None;
        }
        if state.ob_updates_per_s() <= 0.0 || state.ob_changes_per_s() <= 0.0 {
            return None;
        }

        let efficiency = state.price_bps_per_s() / state.ob_changes_per_s();
        let adjusted_spread = state.spread_bps.max(config::MIN_SPREAD_BPS);
        let score = (state.price_bps_per_s() * state.price_ticks_per_s()) / adjusted_spread;

        let verdict = if efficiency >= 0.05 && score >= 30.0 {
            Verdict::Guclu
        } else if efficiency >= 0.03 && score >= 10.0 {
            Verdict::Iyi
        } else if efficiency >= 0.01 && score >= 3.0 {
            Verdict::Normal
        } else if efficiency < 0.01 && state.ob_changes_per_s() > 200.0 {
            Verdict::BotGurultu
        } else {
            Verdict::Zayif
        };

        Some(Opportunity {
            symbol: symbol.to_string(),
            score,
            verdict,
            efficiency,
            price_bps_per_s: state.price_bps_per_s(),
            price_ticks_per_s: state.price_ticks_per_s(),
            ob_changes_per_s: state.ob_changes_per_s(),
            spread_bps: state.spread_bps,
        })
    }
}
