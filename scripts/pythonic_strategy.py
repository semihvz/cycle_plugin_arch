# 🐍 PYTHONIC CYCLELANG HFT STRATEGY SCRIPT (.py / .cy)

# 1. Eklentileri Yükle (Import / Load)
import plugin_binance_gateway as gateway
import plugin_paper_exchange as paper

# 2. Çekirdek Atamaları (Core Pinning)
gateway.pin_core(0)
paper.pin_core(1)

# 3. Eklentileri Başlat
gateway.start()
paper.start()

# 4. Strateji Değişkenleri
target_symbol = "BTCUSDT"
order_qty = 0.25
leverage_val = 20

print("🚀 Pythonic Strateji Başlatıldı! Hedef: " + target_symbol)

# 5. Sanal Alım (Long) ve Satım (Short) Emirleri
buy("BTCUSDT", qty=0.25, price=64800, leverage=20)
sell("ETHUSDT", qty=2.0, price=3150, leverage=50)

# 6. Canlı SQL Veritabanı Sorgusu
sql("SELECT * FROM mark_prices ORDER BY id DESC LIMIT 3")

log("✓ Pythonic Strateji Betiği Yürütmesi Başarıyla Tamamlandı.")
