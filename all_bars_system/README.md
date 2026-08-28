# All-Bars Backtest System (Her Mumda İşlem Backtest Sistemi)

Bu klasör, Binance Futures piyasa verileri (veya çevrimdışı sentetik veriler) üzerinde **Her Mumda İşlem (Every-Bar)** mantığıyla backtest koşturan bağımsız Python sistemini içerir.

## 📂 Klasör Yapısı

```text
all_bars_system/
├── run.py                 # Ana CLI çalıştırma betiği
├── README.md              # Sistem dokümantasyonu
├── output/                # Çıktı veritabanı (SQLite) ve CSV dosyalarının kaydedildiği dizin
│   ├── tacusdt_all_bars_backtest.db
│   └── tacusdt_all_bars_closed_trades.csv
└── all_bars/              # Sistem modülleri
    ├── __init__.py
    ├── fetcher.py         # Binance REST API & sentetik veri jeneratörü
    ├── indicators.py      # ATR(14) indikatörü
    ├── engine.py          # Backtest simülasyon motoru & performans metrikleri
    ├── storage.py         # SQLite veritabanı ve CSV kayıt modülü
    └── reporter.py        # Terminal ASCII raporlama modülü
```

## 🚀 Çalıştırma

Varsayılan ayarlarla (`TACUSDT 1h` 1500 mum) çalıştırmak için:

```bash
python3 run.py
```

Farklı parametrelerle çalıştırmak için:

```bash
python3 run.py --symbol TACUSDT --interval 15m --limit 1500 --pos-size 100
```

## 📊 Çıktılar

1. **SQLite Veritabanı:** `output/tacusdt_all_bars_backtest.db`
   - `closed_trades`: İşlem sonuçları, PnL, Win/Loss durumları.
   - `trade_lookback_bars`: ML eğitimi için her işlemin giriş öncesindeki 100 barlık mum geçmişi.
2. **CSV Dökümü:** `output/tacusdt_all_bars_closed_trades.csv`
