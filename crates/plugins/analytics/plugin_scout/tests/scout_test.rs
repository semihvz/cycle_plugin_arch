use plugin_scout::analyzer::OrderbookFluxAnalyzer;
use plugin_scout::models::{MarketState, SymbolState, Verdict};
use std::time::{SystemTime, UNIX_EPOCH};

fn now_ts() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs_f64()
}

#[test]
fn test_symbol_state_updates_and_spread() {
    let mut state = SymbolState::new();
    let now = now_ts();

    // Update book ticker
    state.update_book_ticker(now, 100.0, 100.1);
    assert_eq!(state.best_bid, 100.0);
    assert_eq!(state.best_ask, 100.1);
    assert_eq!(state.mid, 100.05);

    // Spread bps: (100.1 - 100.0) / 100.0 * 10000 = 10.0 bps
    let expected_spread = (100.1 - 100.0) / 100.0 * 10000.0;
    assert!((state.spread_bps - expected_spread).abs() < 1e-5);
}

#[test]
fn test_orderbook_flux_analyzer_depth_candidates() {
    let analyzer = OrderbookFluxAnalyzer::new();
    let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];
    let mut market = MarketState::new(symbols);
    let now = now_ts();

    // Simulate tick movement for BTCUSDT to give it a positive price score
    if let Some(btc) = market.states.get_mut("BTCUSDT") {
        btc.update_book_ticker(now, 50000.0, 50005.0);
        btc.update_book_ticker(now + 0.1, 50010.0, 50015.0);
    }

    let candidates = analyzer.get_depth_candidates(&mut market);
    assert!(!candidates.is_empty());
    assert_eq!(candidates[0], "BTCUSDT");
}

#[test]
fn test_verdict_as_str() {
    assert_eq!(Verdict::Guclu.as_str(), "GUCLU FIRSAT");
    assert_eq!(Verdict::Iyi.as_str(), "IYI FIRSAT");
    assert_eq!(Verdict::Normal.as_str(), "NORMAL");
    assert_eq!(Verdict::BotGurultu.as_str(), "BOT/GURULTU");
    assert_eq!(Verdict::Zayif.as_str(), "ZAYIF");
}
