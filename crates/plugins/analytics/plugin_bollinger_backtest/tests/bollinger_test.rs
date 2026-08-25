use plugin_bollinger_backtest::{calculate_ema, calculate_triple_ema, run_ema_mtf_backtest, Bar};

#[test]
fn test_ema_calculation() {
    let mut bars = Vec::with_capacity(20);
    for i in 0..20u64 {
        let price = 10.0 + (i as f64);
        bars.push(Bar {
            open_time: i * 3600000u64,
            open: price,
            high: price + 0.5,
            low: price - 0.5,
            close: price,
            volume: 1000.0,
            close_time: i * 3600000u64 + 3599999u64,
        });
    }

    let ema3 = calculate_ema(&bars, 3);
    let ema9 = calculate_ema(&bars, 9);
    assert_eq!(ema3.len(), 20);
    assert_eq!(ema9.len(), 20);
    assert!(ema3[19] > 0.0);
    assert!(ema9[19] > 0.0);

    let triple = calculate_triple_ema(&bars);
    assert_eq!(triple.len(), 20);
    assert!(triple[7].is_none());
    assert!(triple[8].is_some());
}

#[test]
fn test_ema_mtf_backtest_720_bars_1to2_rr() {
    let mut bars_1h = Vec::with_capacity(720);
    for i in 0..720u64 {
        let cycle = (i as f64 * 0.05).sin() * 2.0;
        let price = 10.0 + (i as f64 * 0.02) + cycle;

        bars_1h.push(Bar {
            open_time: i * 3600000u64,
            open: price,
            high: price + 0.3,
            low: price - 0.3,
            close: price + 0.1,
            volume: 5000.0,
            close_time: i * 3600000u64 + 3599999u64,
        });
    }

    let mut bars_15m = Vec::with_capacity(2880);
    for i in 0..2880u64 {
        let cycle = (i as f64 * 0.0125).sin() * 2.0;
        let price = 10.0 + (i as f64 * 0.005) + cycle;

        bars_15m.push(Bar {
            open_time: i * 900000u64,
            open: price,
            high: price + 0.15,
            low: price - 0.15,
            close: price + 0.05,
            volume: 1250.0,
            close_time: i * 900000u64 + 899999u64,
        });
    }

    let summary = run_ema_mtf_backtest("TACUSDT", &bars_1h, &bars_15m, 1000.0, 10.0);

    assert_eq!(summary.symbol, "TACUSDT");
    assert_eq!(summary.primary_interval, "1h");
    assert_eq!(summary.secondary_interval, "15m");
    assert_eq!(summary.initial_capital_usdt, 1000.0);
    assert_eq!(summary.max_risk_per_trade_usdt, 10.0);
    assert_eq!(summary.total_bars_1h, 720);
    assert_eq!(summary.total_bars_15m, 2880);
    assert_eq!(summary.risk_reward_target, "1:2");

    for trade in &summary.trade_history {
        assert_eq!(trade.risk_usdt, 10.0);
        assert_eq!(trade.target_reward_usdt, 20.0);

        if trade.status == "WIN" {
            assert_eq!(trade.pnl_usdt, 20.0);
        } else if trade.status == "LOSS" {
            assert_eq!(trade.pnl_usdt, -10.0);
        }
    }
}

#[tokio::test]
async fn test_live_binance_tacusdt_ema_backtest() {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap();

    let url_1h = "https://fapi.binance.com/fapi/v1/klines?symbol=TACUSDT&interval=1h&limit=720";
    let url_15m = "https://fapi.binance.com/fapi/v1/klines?symbol=TACUSDT&interval=15m&limit=1000";

    let resp_1h = client.get(url_1h).send().await.unwrap();
    let resp_15m = client.get(url_15m).send().await.unwrap();

    let text_1h = resp_1h.text().await.unwrap();
    let text_15m = resp_15m.text().await.unwrap();

    let res_1h: Vec<Vec<serde_json::Value>> = match serde_json::from_str(&text_1h) {
        Ok(v) => v,
        Err(e) => panic!("Failed to parse 1h klines: {} - Body: {}", e, &text_1h[0..text_1h.len().min(200)]),
    };

    let res_15m: Vec<Vec<serde_json::Value>> = match serde_json::from_str(&text_15m) {
        Ok(v) => v,
        Err(e) => panic!("Failed to parse 15m klines: {} - Body: {}", e, &text_15m[0..text_15m.len().min(200)]),
    };

    let parse_u = |v: &serde_json::Value| v.as_u64().unwrap_or_else(|| v.as_str().unwrap().parse().unwrap());
    let parse_f = |v: &serde_json::Value| v.as_f64().unwrap_or_else(|| v.as_str().unwrap().parse().unwrap());

    let bars_1h: Vec<Bar> = res_1h.iter().map(|row| Bar {
        open_time: parse_u(&row[0]),
        open: parse_f(&row[1]),
        high: parse_f(&row[2]),
        low: parse_f(&row[3]),
        close: parse_f(&row[4]),
        volume: parse_f(&row[5]),
        close_time: parse_u(&row[6]),
    }).collect();

    let bars_15m: Vec<Bar> = res_15m.iter().map(|row| Bar {
        open_time: parse_u(&row[0]),
        open: parse_f(&row[1]),
        high: parse_f(&row[2]),
        low: parse_f(&row[3]),
        close: parse_f(&row[4]),
        volume: parse_f(&row[5]),
        close_time: parse_u(&row[6]),
    }).collect();

    let summary = run_ema_mtf_backtest("TACUSDT", &bars_1h, &bars_15m, 1000.0, 10.0);
    println!("\n==========================================================================================");
    println!("📈 BINANCE FUTURES TACUSDT - EMA (3, 6, 9) ÇOKLU ZAMAN DİLİMİ (1h + 15m) BACKTEST RAPORU");
    println!("==========================================================================================");
    println!("Başlangıç Kasası           : {:.2} USDT", summary.initial_capital_usdt);
    println!("İşlem Başı Max Risk        : {:.2} USDT (Kasanın %1'i)", summary.max_risk_per_trade_usdt);
    println!("Hedef Risk / Ödül Oranı    : {}", summary.risk_reward_target);
    println!("İncelenen Periyot          : Son 1 Ay (720 adet 1h mum / 1000 adet 15m mum)");
    println!("------------------------------------------------------------------------------------------");
    println!("Bitiş Kasa Durumu          : {:.2} USDT", summary.final_capital_usdt);
    println!("Net Kâr / Zarar            : {:+.2} USDT ({:+.2}%)", summary.net_profit_usdt, summary.total_return_pct);
    println!("Toplam İşlem Sayısı        : {} adet", summary.total_trades);
    println!("Kazanılan İşlemler         : {} adet", summary.winning_trades);
    println!("Kaybedilen İşlemler        : {} adet", summary.losing_trades);
    println!("Kazanma Oranı (Win Rate)   : {:.2}%", summary.win_rate_pct);
    println!("Profit Factor (Kâr Oranı)  : {:.2}", summary.profit_factor);
    println!("Maksimum Çekilme (Max DD)  : {:.2} USDT ({:.2}%)", summary.max_drawdown_usdt, summary.max_drawdown_pct);
    println!("İşlem Başına Ort. Getiri   : {:+.2} USDT", summary.avg_trade_usdt);
    println!("------------------------------------------------------------------------------------------");
    println!("DETAYLI İŞLEM GEÇMİŞİ (SON İŞLEMLER):");
    for t in &summary.trade_history {
        println!(
            "  • Trade #{:<2} | {:<5} | Size: {:>8.2} USDT | Entry: {:>8.5} | SL: {:>8.5} | TP: {:>8.5} | Result: {:<4} | PnL: {:>+6.2} USDT | Kasa: {:>8.2} USDT",
            t.id, t.side, t.position_size_usdt, t.entry_price, t.stop_loss, t.take_profit, t.status, t.pnl_usdt, t.equity_after_trade
        );
    }
    println!("==========================================================================================\n");

    assert!(summary.total_bars_1h > 0);
}
