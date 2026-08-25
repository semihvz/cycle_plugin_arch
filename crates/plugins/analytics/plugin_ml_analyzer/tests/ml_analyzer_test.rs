use plugin_ml_analyzer::{analyze_symbol_ai, evaluate_ml_model, Bar, MLFeatures};

#[test]
fn test_ml_decision_evaluator() {
    let feats = MLFeatures {
        trend_100b_pct: -15.0,
        trend_50b_pct: -10.0,
        trend_20b_pct: 1.0,
        stoch_pos_pct: 10.0,
        norm_atr_pct: 5.0,
        volatility_range_pct: 20.0,
        volume_ratio: 1.2,
        dist_to_100low_pct: 5.0,
        last_bar_body_ratio: 0.6,
        last_bar_is_bullish: true,
    };

    let (approved, prob) = evaluate_ml_model(&feats);
    assert!(approved);
    assert!(prob >= 0.80);
}

#[tokio::test]
async fn test_live_binance_ml_analyzer() {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap();

    let symbols = vec!["TACUSDT", "VELVETUSDT", "BTCUSDT"];

    println!("\n==========================================================================================");
    println!("🤖 PLUGIN_ML_ANALYZER CANLI PİYASA İNFERENCE VE TARAMA TESTİ");
    println!("==========================================================================================");

    for sym in &symbols {
        let url = format!("https://fapi.binance.com/fapi/v1/klines?symbol={}&interval=15m&limit=120", sym);
        let resp = client.get(&url).send().await.unwrap();
        let text = resp.text().await.unwrap();

        let raw: Vec<Vec<serde_json::Value>> = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => panic!("Failed to parse {} 15m klines: {} - Body: {}", sym, e, &text[0..text.len().min(200)]),
        };

        let parse_u = |v: &serde_json::Value| v.as_u64().unwrap_or_else(|| v.as_str().unwrap().parse().unwrap());
        let parse_f = |v: &serde_json::Value| v.as_f64().unwrap_or_else(|| v.as_str().unwrap().parse().unwrap());

        let bars: Vec<Bar> = raw.iter().map(|row| Bar {
            open_time: parse_u(&row[0]),
            open: parse_f(&row[1]),
            high: parse_f(&row[2]),
            low: parse_f(&row[3]),
            close: parse_f(&row[4]),
            volume: parse_f(&row[5]),
            close_time: parse_u(&row[6]),
        }).collect();

        if let Some(pred) = analyze_symbol_ai(sym, "15m", &bars) {
            println!(
                "  • {:<11} | Fiyat: {:<10.5} | AI Win Olasılığı: %{:<5.2} | Sinyal: {:<25} | SL: {:.5} | TP: {:.5}",
                pred.symbol, pred.current_price, pred.win_probability_pct, pred.signal_decision, pred.predicted_stop_loss, pred.predicted_take_profit
            );
            assert!(pred.current_price > 0.0);
        }
    }
    println!("==========================================================================================\n");
}
