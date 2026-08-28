#!/usr/bin/env python3
"""
Canlı Yapay Zeka (ML) Tahmin & Inference Motoru
------------------------------------------------
Binance Futures canlı mum verilerini anlık olarak çeker, eğitilmiş ML modellerini yükler
ve anlık piyasa şartlarında işlem sinyali & kazanma olasılığı hesaplar.
"""

import os
import sys
import json
import joblib
import urllib.request
import datetime
import numpy as np
import pandas as pd


def fetch_live_klines(symbol="MAGMAUSDT", interval="1m", limit=120):
    url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval={interval}&limit={limit}"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
    with urllib.request.urlopen(req, timeout=10) as resp:
        data = json.loads(resp.read().decode('utf-8'))

    bars = []
    for row in data:
        bars.append({
            'open_time': int(row[0]),
            'open': float(row[1]),
            'high': float(row[2]),
            'low': float(row[3]),
            'close': float(row[4]),
            'volume': float(row[5]),
            'close_time': int(row[6]),
        })
    return bars


def run_live_inference_symbol(symbol="MAGMAUSDT", interval="1m", model_filename="magmausdt_1m_ml_model.joblib", scaler_filename="magmausdt_scaler.joblib"):
    base_dir = "/home/smhvz/Desktop/cycle-orc"
    models_dir = os.path.join(base_dir, "ml_model_suite", "models")

    model_path = os.path.join(models_dir, model_filename)
    scaler_path = os.path.join(models_dir, scaler_filename)
    features_path = os.path.join(models_dir, "magmausdt_feature_names.json")

    if not os.path.exists(model_path) or not os.path.exists(scaler_path):
        print(f"❌ Model dosyası bulunamadı: {model_path}")
        return

    model = joblib.load(model_path)
    scaler = joblib.load(scaler_path)

    if os.path.exists(features_path):
        with open(features_path, "r") as f:
            feature_names = json.load(f)
    else:
        feature_names = [
            'trend_100b_pct', 'trend_50b_pct', 'trend_20b_pct', 'stoch_pos_pct',
            'norm_atr_pct', 'volatility_range_pct', 'volume_ratio',
            'entry_hour', 'dist_to_100low_pct', 'last_bar_body_ratio',
            'last_bar_is_bullish'
        ]

    print("==========================================================================================")
    print(f"⚡ CANLI PİYASA YAPAY ZEKA TAHMİNİ (INFERENCE): {symbol} {interval}")
    print(f"🕒 Zaman (UTC): {datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')}")
    print("==========================================================================================")

    bars = fetch_live_klines(symbol, interval, 120)
    print(f"✅ Binance Futures'tan son {len(bars)} adet canlı {interval} mumu başarıyla çekildi.")

    closes = np.array([b['close'] for b in bars])
    opens = np.array([b['open'] for b in bars])
    highs = np.array([b['high'] for b in bars])
    lows = np.array([b['low'] for b in bars])
    vols = np.array([b['volume'] for b in bars])

    entry_price = closes[-1]
    lowest_100 = lows[-100:].min() if len(lows) >= 100 else lows.min()
    highest_100 = highs[-100:].max() if len(highs) >= 100 else highs.max()

    # ATR (14)
    tr_list = []
    for idx in range(len(bars)):
        if idx == 0:
            tr = highs[idx] - lows[idx]
        else:
            hl = highs[idx] - lows[idx]
            hp = abs(highs[idx] - closes[idx - 1])
            lp = abs(lows[idx] - closes[idx - 1])
            tr = max(hl, hp, lp)
        tr_list.append(tr)
    atr_14 = sum(tr_list[-14:]) / 14.0

    current_utc_hour = datetime.datetime.now(datetime.timezone.utc).hour

    feat_dict = {
        'trend_100b_pct': ((closes[-1] - closes[-100]) / closes[-100]) * 100.0 if len(closes) >= 100 else 0.0,
        'trend_50b_pct': ((closes[-1] - closes[-50]) / closes[-50]) * 100.0 if len(closes) >= 50 else 0.0,
        'trend_20b_pct': ((closes[-1] - closes[-20]) / closes[-20]) * 100.0 if len(closes) >= 20 else 0.0,
        'stoch_pos_pct': ((entry_price - lowest_100) / max(highest_100 - lowest_100, 1e-8)) * 100.0,
        'norm_atr_pct': (atr_14 / entry_price) * 100.0,
        'volatility_range_pct': ((highest_100 - lowest_100) / entry_price) * 100.0,
        'volume_ratio': vols[-10:].mean() / max(vols[-100:].mean(), 1e-8) if len(vols) >= 100 else 1.0,
        'entry_hour': current_utc_hour,
        'dist_to_100low_pct': ((entry_price - lowest_100) / entry_price) * 100.0,
        'last_bar_body_ratio': abs(closes[-1] - opens[-1]) / max(highs[-1] - lows[-1], 1e-8),
        'last_bar_is_bullish': 1 if closes[-1] > opens[-1] else 0,
    }

    X_live = np.array([[feat_dict[col] for col in feature_names]])
    X_scaled = scaler.transform(X_live)

    prob_win = float(model.predict_proba(X_scaled)[0, 1])

    # Stop Loss & Take Profit Seviyeleri (Stratejiye Göre)
    raw_sl = lowest_100 - (2.0 * atr_14)
    sl_dist = max(entry_price - raw_sl, entry_price * 0.005)
    stop_loss = entry_price - sl_dist
    take_profit = entry_price + (2.0 * sl_dist)

    print(f"\n📈 PİYASA ANLIK DURUMU:")
    print(f"   • Anlık Kapanış Fiyatı   : {entry_price:.6f} USDT")
    print(f"   • Son 100 Bar En Düşük  : {lowest_100:.6f} USDT")
    print(f"   • Son 100 Bar En Yüksek : {highest_100:.6f} USDT")
    print(f"   • Anlık ATR(14)         : {atr_14:.6f} USDT")
    print(f"   • Hesaplanan Stop Loss  : {stop_loss:.6f} USDT (-%{(sl_dist/entry_price)*100:.2f})")
    print(f"   • Hesaplanan Take Profit: {take_profit:.6f} USDT (+%{(sl_dist*2.0/entry_price)*100:.2f})")

    print(f"\n🧠 YAPAY ZEKA TAHMİNİ:")
    print(f"   • Algoritma Türü        : {type(model).__name__}")
    print(f"   • Kazanma Olasılığı (WIN): %{prob_win*100:.2f}")

    print(f"\n🚦 EŞİK DEĞERLERİNE GÖRE KARARLAR:")
    for th in [0.40, 0.50, 0.55, 0.60]:
        status = "🟢 İŞLEM ÖNERİLİR (LONG)" if prob_win >= th else "🔴 PAS GEÇ (SKIP)"
        print(f"   • Eşik >= {th:.2f} : {status}")

    print("\n📊 ÇIKARILAN CANLI ÖZNİTELİKLER (FEATURES):")
    for k, v in feat_dict.items():
        if isinstance(v, float):
            print(f"   • {k:<22}: {v:<+10.4f}")
        else:
            print(f"   • {k:<22}: {v}")

    print("==========================================================================================\n")


if __name__ == "__main__":
    symbol = sys.argv[1] if len(sys.argv) > 1 else "MAGMAUSDT"
    interval = sys.argv[2] if len(sys.argv) > 2 else "1m"
    run_live_inference_symbol(symbol, interval)
