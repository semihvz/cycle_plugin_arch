"""
Binance Futures REST API Veri Sağlayıcı ve Sentetik Veri Jeneratörü
"""

import datetime
import json
import math
import urllib.request


def fetch_klines(symbol="TACUSDT", interval="1h", limit=1500):
    """Binance Futures REST API'den mum verilerini çeker."""
    url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval={interval}&limit={limit}"
    req = urllib.request.Request(
        url,
        headers={
            "User-Agent": (
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
            )
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = json.loads(resp.read().decode("utf-8"))

        bars = []
        for row in data:
            bars.append({
                "open_time": int(row[0]),
                "open": float(row[1]),
                "high": float(row[2]),
                "low": float(row[3]),
                "close": float(row[4]),
                "volume": float(row[5]),
                "close_time": int(row[6]),
            })
        return bars
    except Exception as e:
        print(f"[UYARI] Binance API veri çekme hatası: {e}. Sentetik yedek veri kullanılıyor...")
        return generate_fallback_bars(limit)


def generate_fallback_bars(count=1500):
    """Çevrimdışı/API erişilemez durumlar için sentetik mum verisi üretir."""
    now_ms = int(datetime.datetime.now(datetime.timezone.utc).timestamp() * 1000)
    start_time = now_ms - (count * 3600 * 1000)
    bars = []
    for i in range(count):
        cycle = math.sin(i * 0.05) * 0.0003
        price = max(0.001, 0.0028 + (i * 0.000001) + cycle)
        open_time = start_time + (i * 3600 * 1000)
        bars.append({
            "open_time": open_time,
            "open": price,
            "high": price + 0.00008,
            "low": price - 0.00008,
            "close": price + 0.00002,
            "volume": 500000.0,
            "close_time": open_time + 3599999,
        })
    return bars
