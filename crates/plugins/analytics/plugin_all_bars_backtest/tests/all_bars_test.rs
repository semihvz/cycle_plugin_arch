use plugin_all_bars_backtest::{calculate_atr_series, run_all_bars_backtest, Bar};

#[test]
fn test_synthetic_all_bars_backtest() {
    let mut bars = Vec::with_capacity(150);
    for i in 0..150u64 {
        let price = 10.0 + (i as f64 * 0.05).sin();
        bars.push(Bar {
            open_time: i * 3600000u64,
            open: price,
            high: price + 0.2,
            low: price - 0.2,
            close: price + 0.05,
            volume: 1000.0,
            close_time: i * 3600000u64 + 3599999u64,
        });
    }

    let atr = calculate_atr_series(&bars, 14);
    assert_eq!(atr.len(), 150);
    assert!(atr[14] > 0.0);

    let summary = run_all_bars_backtest("TACUSDT", "1h", &bars);
    assert_eq!(summary.symbol, "TACUSDT");
    assert_eq!(summary.total_bars, 150);
    assert_eq!(summary.total_trades, 50); // 150 - 100 lookback = 50 trades
}

#[tokio::test]
async fn test_live_binance_tacusdt_all_bars_backtest() {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap();

    let url = "https://fapi.binance.com/fapi/v1/klines?symbol=TACUSDT&interval=1h&limit=1500";
    let resp = client.get(url).send().await.unwrap();
    let text = resp.text().await.unwrap();

    let raw: Vec<Vec<serde_json::Value>> = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => panic!("Failed to parse Binance 1h klines: {} - Body: {}", e, &text[0..text.len().min(200)]),
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

    let summary = run_all_bars_backtest("TACUSDT", "1h", &bars);

    println!("\n====================================================================================================");
    println!("🔥 BINANCE FUTURES TACUSDT - HER MUMDA İŞLEM (EVERY-BAR) BACKTEST RAPORU");
    println!("====================================================================================================");
    println!("Sembol ve Zaman Dilimi    : {} / {}", summary.symbol, summary.interval);
    println!("Toplam Çekilen Bar Sayısı : {} adet 1h mum", summary.total_bars);
    println!("Açılan Toplam İşlem Sayısı: {} adet (100-bar lookback sonrası)", summary.total_trades);
    println!("Sabit Pozisyon Büyüklüğü  : {:.2} USDT", summary.fixed_position_size_usdt);
    println!("----------------------------------------------------------------------------------------------------");
    println!("Kazanılan İşlemler        : {} adet", summary.winning_trades);
    println!("Kaybedilen İşlemler       : {} adet", summary.losing_trades);
    println!("Halen Açık İşlemler       : {} adet", summary.open_trades);
    println!("Kazanma Oranı (Win Rate)  : {:.2}%", summary.win_rate_pct);
    println!("Net Toplam Kâr/Zarar      : {:+.2} USDT", summary.total_net_pnl_usdt);
    println!("Profit Factor (Kâr Oranı) : {:.2}", summary.profit_factor);
    println!("Maksimum Çekilme (Max DD) : {:.2} USDT ({:.2}%)", summary.max_drawdown_usdt, summary.max_drawdown_pct);
    println!("İşlem Başı Ort. Kâr/Zarar : {:+.2} USDT", summary.avg_trade_pnl_usdt);
    println!("----------------------------------------------------------------------------------------------------");
    println!("TÜM İŞLEMLERİN SIRALI LİSTESİ (TOPLAM {} İŞLEM):", summary.total_trades);

    for t in &summary.trade_history {
        let exit_str = t.exit_time_str.as_deref().unwrap_or("Halen Açık");
        let exit_p_str = t.exit_price.map(|p| format!("{:.5}", p)).unwrap_or_else(|| "---".to_string());

        println!(
            "  • Trade #{:<4} | Giriş Zamanı: {} | Giriş: {:>8.5} | 100BarLow: {:>8.5} | ATR14: {:>7.5} | SL: {:>8.5} | TP: {:>8.5} | Çıkış Zamanı: {} | Çıkış: {:>8} | Result: {:<4} | Risk: {:>5.2} USDT | TP-Reward: {:>5.2} USDT | PnL: {:>+6.2} USDT (Barlar: {:>2})",
            t.id, t.entry_time_str, t.entry_price, t.lowest_100_price, t.atr_14, t.stop_loss, t.take_profit, exit_str, exit_p_str, t.status, t.risk_usdt, t.target_reward_usdt, t.pnl_usdt, t.holding_bars
        );
    }
    println!("================================================================================--------------------\n");

    assert!(summary.total_trades > 0);
}
