// 🚀 CYCLELANG HFT SAMPLE STRATEGY SCRIPT (.cy)

// 1. Dinamik Eklentileri Yükle
let gateway = plugin.load("plugin_binance_gateway")
let paper   = plugin.load("plugin_paper_exchange")

// 2. Çekirdek Atamaları (Core Pinning)
gateway.pin_core(0)
paper.pin_core(1)

// 3. Veri Akış Boru Hattını Bağla
pipe HFT_Pipeline {
    gateway.best_price -> paper.market_data
}

// 4. Eklentileri Başlat
gateway.start()
paper.start()

// 5. Strateji Parametreleri ve Hesaplama
let target_symbol = "BTCUSDT"
let order_qty = 0.1
let leverage_val = 20

log("🚀 Strateji Başlatıldı. Hedef Sembol: " + target_symbol)

// 6. Sanal Alım (Long) Emri İlet
buy(target_symbol, qty: order_qty, price: 64500, leverage: leverage_val)

// 7. Sanal Satım (Short) Emri İlet
sell("ETHUSDT", qty: 1.5, price: 3200, leverage: 50)

// 8. SQL Veritabanı Sorgusu Tetikle
sql("SELECT * FROM mark_prices ORDER BY id DESC LIMIT 3")

log("✓ Demo Strateji Betiği Yürütmesi Tamamlandı.")
