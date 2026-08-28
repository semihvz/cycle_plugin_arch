#!/usr/bin/env python3
import urllib.request
import json
import datetime
import os
import numpy as np
import pandas as pd
import joblib

def fetch_live_klines(symbol, interval="15m", limit=120):
    url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval={interval}&limit={limit}"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
    try:
        with urllib.request.urlopen(req) as resp:
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
    except Exception as e:
        print(f"Error fetching {symbol}: {e}")
        return []

def main():
    symbols = ["BTRUSDT", "TACUSDT", "VELVETUSDT", "BTCUSDT", "ETHUSDT", "SOLUSDT", "AVAXUSDT", "DOGEUSDT"]
    interval = "15m"
    
    models_dir = "/home/smhvz/Desktop/cycle-orc/ml_model_suite/models"
    model_path = os.path.join(models_dir, "tacusdt_15m_ml_model.joblib")
    scaler_path = os.path.join(models_dir, "scaler_15m.joblib")

    if not os.path.exists(model_path):
        model_path = os.path.join(models_dir, "tacusdt_ml_model.joblib")
        scaler_path = os.path.join(models_dir, "scaler.joblib")

    model = joblib.load(model_path)
    scaler = joblib.load(scaler_path)

    feature_cols = [
        'trend_100b_pct', 'trend_50b_pct', 'trend_20b_pct', 'stoch_pos_pct',
        'norm_atr_pct', 'volatility_range_pct', 'volume_ratio',
        'entry_hour', 'dist_to_100low_pct', 'last_bar_body_ratio',
        'last_bar_is_bullish'
    ]

    now_utc = datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')

    print("==========================================================================================")
    print(f"⚡ BINANCE FUTURES CANLI PİYASA 15M YAPAY ZEKA (ML) TAHMİN VE ANALİZ RAPORU")
    print(f"🕒 Analiz Zamanı: {now_utc}")
    print("==========================================================================================")

    results = []

    for sym in symbols:
        bars = fetch_live_klines(sym, interval, 120)
        if len(bars) < 100:
            continue

        closes = np.array([b['close'] for b in bars])
        opens = np.array([b['open'] for b in bars])
        highs = np.array([b['high'] for b in bars])
        lows = np.array([b['low'] for b in bars])
        vols = np.array([b['volume'] for b in bars])

        current_price = closes[-1]
        window_100 = bars[-100:]
        lowest_100 = min(b['low'] for b in window_100)
        highest_100 = max(b['high'] for b in window_100)

        # ATR 14
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

        raw_sl = lowest_100 - (2.0 * atr_14)
        sl_dist = max(current_price - raw_sl, current_price * 0.005)
        stop_loss = current_price - sl_dist
        take_profit = current_price + (2.0 * sl_dist)

        trend_100b_pct = ((closes[-1] - closes[-100]) / closes[-100]) * 100.0
        trend_50b_pct = ((closes[-1] - closes[-50]) / closes[-50]) * 100.0
        trend_20b_pct = ((closes[-1] - closes[-20]) / closes[-20]) * 100.0
        stoch_pos_pct = ((current_price - lowest_100) / max(highest_100 - lowest_100, 1e-8)) * 100.0
        norm_atr_pct = (atr_14 / current_price) * 100.0
        volatility_range_pct = ((highest_100 - lowest_100) / current_price) * 100.0
        vol_10_mean = vols[-10:].mean()
        vol_100_mean = vols[-100:].mean()
        volume_ratio = vol_10_mean / max(vol_100_mean, 1e-8)
        entry_hour = datetime.datetime.now(datetime.timezone.utc).hour
        dist_to_100low_pct = ((current_price - lowest_100) / current_price) * 100.0
        last_body = abs(closes[-1] - opens[-1])
        last_range = max(highs[-1] - lows[-1], 1e-8)
        last_bar_body_ratio = last_body / last_range
        last_bar_is_bullish = 1 if closes[-1] > opens[-1] else 0

        feat_vector = np.array([[
            trend_100b_pct, trend_50b_pct, trend_20b_pct, stoch_pos_pct,
            norm_atr_pct, volatility_range_pct, volume_ratio,
            entry_hour, dist_to_100low_pct, last_bar_body_ratio,
            last_bar_is_bullish
        ]])

        feat_scaled = scaler.transform(feat_vector)
        win_prob = model.predict_proba(feat_scaled)[0, 1]

        signal = "TRADE_RECOMMENDED (LONG)" if win_prob >= 0.50 else "SKIP_TRADE (YÜKSEK RİSK)"

        results.append({
            'symbol': sym,
            'price': current_price,
            'prob': win_prob * 100.0,
            'signal': signal,
            'sl': stop_loss,
            'tp': take_profit,
            'atr': atr_14,
            'stoch': stoch_pos_pct,
            'trend_100b': trend_100b_pct
        })

    print(f"{'Parite':<12} | {'Canlı Fiyat':<12} | {'AI Win Rate (%)':<16} | {'Karar Sinyali':<26} | {'Stop Loss':<12} | {'Take Profit':<12}")
    print("-" * 100)

    for r in results:
        print(f"{r['symbol']:<12} | {r['price']:<12.5f} | %{r['prob']:<15.2f} | {r['signal']:<26} | {r['sl']:<12.5f} | {r['tp']:<12.5f}")

    print("==========================================================================================")
    print("💡 CANLI DETAYLI GÖSTERGE ÖZETİ:")
    print("------------------------------------------------------------------------------------------")
    for r in results:
        status_emoji = "🟢" if "TRADE_RECOMMENDED" in r['signal'] else "🔴"
        print(f"{status_emoji} {r['symbol']:<10}: Win Prob: %{r['prob']:.2f} | Trend100b: {r['trend_100b']:+.2f}% | StochPos: {r['stoch']:.1f}% | ATR14: {r['atr']:.5f}")
    print("==========================================================================================")

if __name__ == "__main__":
    main()
