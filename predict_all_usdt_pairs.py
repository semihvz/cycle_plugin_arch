#!/usr/bin/env python3
"""
Tüm Binance Futures USDT Pariteleri İçin Çift Yönlü (LONG & SHORT) Canlı Yapay Zeka Tahmin Motoru
-----------------------------------------------------------------------------------------------
1. Binance Futures borsasındaki TÜM aktif USDT paritelerini bulur.
2. Hem LONG hem SHORT yönlü pozisyonlar için 11 adet teknik özniteliği çıkarır.
3. ThreadPoolExecutor (25 Worker) ile tüm pariteleri saniyeler içinde tara.
4. En yüksek sinyal veren LONG ve SHORT fırsatlarını ayrı tablolar halinde raporlar.
"""

import os
import sys
import json
import joblib
import urllib.request
import datetime
import time
import warnings
import pandas as pd
import numpy as np
from concurrent.futures import ThreadPoolExecutor, as_completed

warnings.filterwarnings('ignore')


def get_all_usdt_futures_symbols():
    url = "https://fapi.binance.com/fapi/v1/exchangeInfo"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            data = json.loads(resp.read().decode('utf-8'))
        symbols = []
        for s in data.get('symbols', []):
            if s.get('quoteAsset') == 'USDT' and s.get('status') == 'TRADING' and s.get('contractType') == 'PERPETUAL':
                symbols.append(s['symbol'])
        return sorted(symbols)
    except Exception as e:
        print(f"⚠️ ExchangeInfo çekme hatası: {e}")
        return ["BTCUSDT", "ETHUSDT", "SOLUSDT", "AVAXUSDT", "LINKUSDT", "DOGEUSDT", "MAGMAUSDT", "AKEUSDT", "TACUSDT", "VELVETUSDT"]


def process_symbol_prediction(symbol, model, scaler, feature_cols, interval="1m"):
    url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval={interval}&limit=120"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
    try:
        with urllib.request.urlopen(req, timeout=6) as resp:
            data = json.loads(resp.read().decode('utf-8'))
        
        bars = []
        for row in data:
            bars.append({
                'open': float(row[1]),
                'high': float(row[2]),
                'low': float(row[3]),
                'close': float(row[4]),
                'volume': float(row[5]),
            })

        if len(bars) < 100:
            return None

        closes = np.array([b['close'] for b in bars])
        opens = np.array([b['open'] for b in bars])
        highs = np.array([b['high'] for b in bars])
        lows = np.array([b['low'] for b in bars])
        vols = np.array([b['volume'] for b in bars])

        entry_price = closes[-1]
        lowest_100 = lows[-100:].min()
        highest_100 = highs[-100:].max()

        tr_list = []
        for i in range(len(bars)):
            if i == 0:
                tr = highs[i] - lows[i]
            else:
                hl = highs[i] - lows[i]
                hp = abs(highs[i] - closes[i - 1])
                lp = abs(lows[i] - closes[i - 1])
                tr = max(hl, hp, lp)
            tr_list.append(tr)
        atr_14 = sum(tr_list[-14:]) / 14.0

        current_utc_hour = datetime.datetime.now(datetime.timezone.utc).hour

        # ------------------- 1. LONG ÖZNİTELİK & TAHMİN -------------------
        raw_sl_long = lowest_100 - (2.0 * atr_14)
        sl_dist_long = max(entry_price - raw_sl_long, entry_price * 0.005)
        stop_loss_long = entry_price - sl_dist_long
        take_profit_long = entry_price + (2.0 * sl_dist_long)

        feat_long = {
            'trend_100b_pct': ((closes[-1] - closes[-100]) / closes[-100]) * 100.0,
            'trend_50b_pct': ((closes[-1] - closes[-50]) / closes[-50]) * 100.0,
            'trend_20b_pct': ((closes[-1] - closes[-20]) / closes[-20]) * 100.0,
            'stoch_pos_pct': ((entry_price - lowest_100) / max(highest_100 - lowest_100, 1e-8)) * 100.0,
            'norm_atr_pct': (atr_14 / entry_price) * 100.0,
            'volatility_range_pct': ((highest_100 - lowest_100) / entry_price) * 100.0,
            'volume_ratio': vols[-10:].mean() / max(vols[-100:].mean(), 1e-8),
            'entry_hour': current_utc_hour,
            'dist_to_100low_pct': ((entry_price - lowest_100) / entry_price) * 100.0,
            'last_bar_body_ratio': abs(closes[-1] - opens[-1]) / max(highs[-1] - lows[-1], 1e-8),
            'last_bar_is_bullish': 1 if closes[-1] > opens[-1] else 0,
        }

        X_long = np.array([[feat_long[c] for c in feature_cols]])
        X_long_scaled = scaler.transform(X_long)
        prob_long = float(model.predict_proba(X_long_scaled)[0, 1])

        # ------------------- 2. SHORT ÖZNİTELİK & TAHMİN -------------------
        raw_sl_short = highest_100 + (2.0 * atr_14)
        sl_dist_short = max(raw_sl_short - entry_price, entry_price * 0.005)
        stop_loss_short = entry_price + sl_dist_short
        take_profit_short = entry_price - (2.0 * sl_dist_short)

        # Short yönü için simetrik/ters göstergeler
        feat_short = {
            'trend_100b_pct': -((closes[-1] - closes[-100]) / closes[-100]) * 100.0,
            'trend_50b_pct': -((closes[-1] - closes[-50]) / closes[-50]) * 100.0,
            'trend_20b_pct': -((closes[-1] - closes[-20]) / closes[-20]) * 100.0,
            'stoch_pos_pct': ((highest_100 - entry_price) / max(highest_100 - lowest_100, 1e-8)) * 100.0,
            'norm_atr_pct': (atr_14 / entry_price) * 100.0,
            'volatility_range_pct': ((highest_100 - lowest_100) / entry_price) * 100.0,
            'volume_ratio': vols[-10:].mean() / max(vols[-100:].mean(), 1e-8),
            'entry_hour': current_utc_hour,
            'dist_to_100low_pct': ((highest_100 - entry_price) / entry_price) * 100.0,
            'last_bar_body_ratio': abs(closes[-1] - opens[-1]) / max(highs[-1] - lows[-1], 1e-8),
            'last_bar_is_bullish': 1 if closes[-1] < opens[-1] else 0, # Ayı mumu
        }

        X_short = np.array([[feat_short[c] for c in feature_cols]])
        X_short_scaled = scaler.transform(X_short)
        prob_short = float(model.predict_proba(X_short_scaled)[0, 1])

        return {
            'symbol': symbol,
            'price': entry_price,
            # LONG Metrics
            'prob_long': prob_long,
            'prob_long_pct': round(prob_long * 100.0, 2),
            'sl_long': stop_loss_long,
            'tp_long': take_profit_long,
            # SHORT Metrics
            'prob_short': prob_short,
            'prob_short_pct': round(prob_short * 100.0, 2),
            'sl_short': stop_loss_short,
            'tp_short': take_profit_short,
            'trend_100b': round(feat_long['trend_100b_pct'], 2),
            'volume_ratio': round(feat_long['volume_ratio'], 2)
        }
    except Exception:
        return None


def main():
    interval = "1m"
    base_dir = "/home/smhvz/Desktop/cycle-orc"
    models_dir = os.path.join(base_dir, "ml_model_suite", "models")

    model_path = os.path.join(models_dir, "magmausdt_1m_ml_model.joblib")
    scaler_path = os.path.join(models_dir, "magmausdt_scaler.joblib")

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

    print("==========================================================================================")
    print(f"⚡ TÜM BINANCE FUTURES USDT PARİTELERİ ÇİFT YÖNLÜ (LONG & SHORT) TAHMİN TARAMASI ({interval})")
    print(f"🕒 Zaman (UTC): {datetime.datetime.now(datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')}")
    print("==========================================================================================")

    symbols = get_all_usdt_futures_symbols()
    print(f"📥 Toplam {len(symbols)} adet aktif USDT vadeli parite bulundu. Paralel tarama başlatılıyor...")

    start_time = time.time()
    results = []

    with ThreadPoolExecutor(max_workers=25) as executor:
        futures = {executor.submit(process_symbol_prediction, sym, model, scaler, feature_cols, interval): sym for sym in symbols}
        for future in as_completed(futures):
            res = future.result()
            if res:
                results.append(res)

    res_df = pd.DataFrame(results)
    elapsed_sec = round(time.time() - start_time, 2)

    print(f"✅ Çift Yönlü Tarama Tamamlandı! ({elapsed_sec} saniyede {len(res_df)} parite analiz edildi)\n")

    # 1. TOP LONG SİNYALLERİ (prob_long >= 0.50)
    df_long = res_df.sort_values('prob_long', ascending=False)
    top_longs = df_long[df_long['prob_long'] >= 0.50]

    print("==========================================================================================")
    print(f"🟢 1. EN YÜKSEK LONG SİNYALİ VEREN PARİTELER (Win Prob >= %50.0) [{len(top_longs)} Parite]:")
    print("==========================================================================================")
    if len(top_longs) > 0:
        print(f"{'Sıra':<4} | {'Sembol':<12} | {'Canlı Fiyat':<14} | {'LONG Win Prob (%)':<18} | {'Stop Loss (SL)':<16} | {'Take Profit (TP)':<16}")
        print("-" * 95)
        for i, (_, row) in enumerate(top_longs.head(15).iterrows()):
            print(f"{i+1:<4} | {row['symbol']:<12} | {row['price']:<14.6f} | 🟢 %{row['prob_long_pct']:<15.2f} | {row['sl_long']:<16.6f} | {row['tp_long']:<16.6f}")

    # 2. TOP SHORT SİNYALLERİ (prob_short >= 0.50)
    df_short = res_df.sort_values('prob_short', ascending=False)
    top_shorts = df_short[df_short['prob_short'] >= 0.50]

    print("\n==========================================================================================")
    print(f"🔴 2. EN YÜKSEK SHORT SİNYALİ VEREN PARİTELER (Win Prob >= %50.0) [{len(top_shorts)} Parite]:")
    print("==========================================================================================")
    if len(top_shorts) > 0:
        print(f"{'Sıra':<4} | {'Sembol':<12} | {'Canlı Fiyat':<14} | {'SHORT Win Prob (%)':<18} | {'Stop Loss (SL)':<16} | {'Take Profit (TP)':<16}")
        print("-" * 95)
        for i, (_, row) in enumerate(top_shorts.head(15).iterrows()):
            print(f"{i+1:<4} | {row['symbol']:<12} | {row['price']:<14.6f} | 🔴 %{row['prob_short_pct']:<15.2f} | {row['sl_short']:<16.6f} | {row['tp_short']:<16.6f}")
    else:
        print("🔴 Şu an %50 üstünde SHORT sinyali veren parite bulunamadı.")

    output_csv = os.path.join(base_dir, "all_usdt_dual_predictions.csv")
    res_df.to_csv(output_csv, index=False)
    print(f"\n💾 Tüm çift yönlü tahmin sonuçları kaydedildi: {output_csv}")
    print("==========================================================================================\n")


if __name__ == "__main__":
    main()
