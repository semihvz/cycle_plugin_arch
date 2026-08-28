use std::env;
use std::str::FromStr;
use ohlcv_engine::client::BinanceClient;
use plugin_ms_analyzer::narrative;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<String> = env::args().collect();
    
    let symbol = args.get(1).map(|s| s.as_str()).unwrap_or("MAGMAUSDT");
    let interval = args.get(2).map(|s| s.as_str()).unwrap_or("15m");
    let bar_limit: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(100);

    println!("==========================================================================================");
    println!("🚀 MS ANALYZER (MSMP 2.0) - MARKET STRUCTURE & MICROSTRUCTURE ANALİZİ");
    println!("📌 Sembol: {} | Periyot: {} | Analiz Edilen Bar Sayısı: {}", symbol, interval, bar_limit);
    println!("==========================================================================================");

    let client = BinanceClient::new();
    
    // Fetch up to 400 bars for macro trend context, but slice according to request
    let fetch_limit = bar_limit.max(400);
    println!("📥 Binance Futures API'den {} adet 15m mum verisi çekiliyor...", fetch_limit);
    
    let all_klines = match client.fetch_klines(symbol, interval, fetch_limit).await {
        Ok(k) => k,
        Err(e) => {
            eprintln!("❌ Veri çekme hatası: {}", e);
            return Err(e);
        }
    };

    if all_klines.is_empty() {
        println!("❌ Mum verisi alınamadı!");
        return Ok(());
    }

    let len = all_klines.len();
    println!("✅ Toplam {} adet mum başarıyla alındı.", len);

    // Filter/slice exact bars for analysis
    let core_limit = bar_limit.min(len);
    let amp_limit = (bar_limit * 4).min(len);
    let acute_limit = 96.min(len);

    let core_klines = &all_klines[len.saturating_sub(core_limit)..];
    let amp_klines = &all_klines[len.saturating_sub(amp_limit)..];
    let acute_klines = &all_klines[len.saturating_sub(acute_limit)..];

    // Generate narrative report
    let report = narrative::generate_report(core_klines, amp_klines, acute_klines);
    let stream_id = format!("{}_{}", symbol.to_lowercase(), interval);

    // Print formatted table from Rust engine
    let table = report.format_table(symbol, interval, &stream_id, core_klines.len());
    println!("\n{}", table);

    // Extra Detailed Analysis Output
    println!("==========================================================================================");
    println!("🔍 DETAYLI METRİK & MİKRO YAPI ANALİZ ÖZETİ");
    println!("------------------------------------------------------------------------------------------");
    println!("• Canlı Fiyat        : {}", report.current_price);
    println!("• ATR (14)           : {}", report.atr);
    println!("• Volatilite Bandı   : POC - 1.5σ: {:.4}  <--->  POC + 1.5σ: {:.4}", report.volatility_band.0, report.volatility_band.1);
    println!("• Likidite Dengesi   : BSL/SSL Oranı = {:.2} ({})", 
        report.bsl_ssl_ratio,
        if report.bsl_ssl_ratio > rust_decimal::Decimal::ONE { "Alıcı Ağır / Buy-Side Liquidity Baskın" } else { "Satıcı Ağır / Sell-Side Liquidity Baskın" }
    );
    println!("• Ağırlıklı Trend    : ATS = {:.2} / 10.0 ({})", report.ats, report.trend_label);
    println!("• Hurst Üssü (H)     : {:.4} ({})", 
        report.hurst,
        if report.hurst > rust_decimal::Decimal::from_str("0.55").unwrap() { "Güçlü Trend Kalıcılığı (Persistent)" } else if report.hurst < rust_decimal::Decimal::from_str("0.45").unwrap() { "Yatay / Mean-Reverting" } else { "Rassal Yürüyüş (Random Walk)" }
    );
    println!("• Belirleme Kat. (R²): {:.4} (Trend Güvenilirliği: %{:.1})", report.r_squared, report.r_squared * rust_decimal::Decimal::ONE_HUNDRED);
    println!("• Zaman Dilimi Uyumu : Confluence Index = %{:.1}", report.confluence_index);
    println!("• FVG Formasyonları   : Toplam FVG: {}, Aktif Emici Bölge: {}", report.fvg_count, report.active_absorber_count);
    
    if let Some(ref vac) = report.vacuum_zone {
        println!("------------------------------------------------------------------------------------------");
        println!("🌀 EN YÜKSEK MANYETİK ALAN (THE VACUUM ZONE):");
        println!("  - Fiyat Aralığı : [{:.4} - {:.4}]", vac.price_low, vac.price_high);
        println!("  - Manyetik Skor : {:.2}", vac.magnetic_score);
        println!("  - Etiket        : {}", vac.label);
        println!("  - Delta Onayı   : {}", if vac.delta_confirmed { "EVET ✅" } else { "HAYIR ❌" });
    }

    println!("==========================================================================================");

    Ok(())
}
