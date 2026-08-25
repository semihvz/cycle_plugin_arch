use plugin_tacusdt_1h::{process_and_persist_tacusdt_1h, Bar};
use rusqlite::Connection;

#[tokio::test]
async fn test_live_tacusdt_1h_pipeline() {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .build()
        .unwrap();

    let url_1h = "https://fapi.binance.com/fapi/v1/klines?symbol=TACUSDT&interval=1h&limit=1500";
    let resp = client.get(url_1h).send().await.unwrap();
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

    let test_db_path = "/home/smhvz/Desktop/cycle-orc/data/tacusdt_1h_collector.db";
    let status = process_and_persist_tacusdt_1h("TACUSDT", "1h", &bars, test_db_path);

    println!("\n==========================================================================================");
    println!("🔥 TACUSDT 1h TÜM ZAMANLAR İŞLEM VE 100-BAR KAYIT EKLENTİSİ CANLI RAPORU");
    println!("==========================================================================================");
    println!("Sembol ve Zaman Dilimi    : {} / {}", status.symbol, status.interval);
    println!("Çekilen Toplam 1h Bar     : {} adet 1h mum", status.total_bars_fetched);
    println!("Tespit Edilen İşlem Sayısı: {} adet", status.total_trades_detected);
    println!("Ham Win Rate              : %{:.2}", status.win_rate_pct);
    println!("Ham Net PnL               : {:+.2} USDT", status.net_pnl_usdt);
    println!("Saklanan 100-Bar Mum Kaydı: {} satır", status.total_lookback_bars_persisted);
    println!("SQLite Veritabanı Yolu   : {} ({:.2} MB)", status.db_file_path, status.db_size_mb);
    println!("Son İşlem Özeti           : {}", status.last_trade_summary);
    println!("==========================================================================================\n");

    assert!(status.total_bars_fetched > 0);
    assert!(status.total_trades_detected > 0);
    assert!(status.total_lookback_bars_persisted > 0);

    let conn = Connection::open(test_db_path).unwrap();
    let trades_count: i64 = conn.query_row("SELECT count(*) FROM closed_trades;", [], |r| r.get(0)).unwrap();
    let lookback_count: i64 = conn.query_row("SELECT count(*) FROM trade_lookback_bars;", [], |r| r.get(0)).unwrap();

    println!("✅ SQLite Veritabanı Tablo Doğrulaması:");
    println!("   • closed_trades satır sayısı        : {} satır", trades_count);
    println!("   • trade_lookback_bars satır sayısı   : {} satır", lookback_count);

    assert_eq!(trades_count as usize, status.total_trades_detected);
    assert_eq!(lookback_count as usize, status.total_lookback_bars_persisted);
}
