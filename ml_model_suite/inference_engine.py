#!/usr/bin/env python3
import os
import sys
import json
import joblib
import urllib.request
import pandas as pd
import numpy as np

class MLInferenceEngine:
    def __init__(self, models_dir="/home/smhvz/Desktop/cycle-orc/ml_model_suite/models"):
        self.model_path = os.path.join(models_dir, "tacusdt_ml_model.joblib")
        self.scaler_path = os.path.join(models_dir, "scaler.joblib")
        self.features_path = os.path.join(models_dir, "feature_names.json")

        if not os.path.exists(self.model_path) or not os.path.exists(self.scaler_path):
            raise FileNotFoundError("Trained ML model or scaler not found. Run model_trainer.py first.")

        self.model = joblib.load(self.model_path)
        self.scaler = joblib.load(self.scaler_path)
        
        with open(self.features_path, "r") as f:
            self.feature_names = json.load(f)

    def extract_features_from_bars(self, bars, atr_14=None, lowest_100=None):
        if len(bars) < 100:
            raise ValueError(f"Inference requires at least 100 bars, got {len(bars)}")

        closes = np.array([b['close'] for b in bars])
        opens = np.array([b['open'] for b in bars])
        highs = np.array([b['high'] for b in bars])
        lows = np.array([b['low'] for b in bars])
        volumes = np.array([b['volume'] for b in bars])

        entry_price = closes[-1]
        
        if lowest_100 is None:
            lowest_100 = lows[-100:].min()
        
        highest_100 = highs[-100:].max()

        if atr_14 is None:
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

        trend_100b_pct = ((closes[-1] - closes[-100]) / closes[-100]) * 100.0
        trend_50b_pct = ((closes[-1] - closes[-50]) / closes[-50]) * 100.0
        trend_20b_pct = ((closes[-1] - closes[-20]) / closes[-20]) * 100.0
        stoch_pos_pct = ((entry_price - lowest_100) / max(highest_100 - lowest_100, 1e-8)) * 100.0
        norm_atr_pct = (atr_14 / entry_price) * 100.0
        volatility_range_pct = ((highest_100 - lowest_100) / entry_price) * 100.0
        vol_10_mean = volumes[-10:].mean()
        vol_100_mean = volumes[-100:].mean()
        volume_ratio = vol_10_mean / max(vol_100_mean, 1e-8)
        
        entry_hour = 12 # Default for feature if unknown
        dist_to_100low_pct = ((entry_price - lowest_100) / entry_price) * 100.0
        
        last_body = abs(closes[-1] - opens[-1])
        last_range = max(highs[-1] - lows[-1], 1e-8)
        last_bar_body_ratio = last_body / last_range
        last_bar_is_bullish = 1 if closes[-1] > opens[-1] else 0

        feat_dict = {
            'trend_100b_pct': trend_100b_pct,
            'trend_50b_pct': trend_50b_pct,
            'trend_20b_pct': trend_20b_pct,
            'stoch_pos_pct': stoch_pos_pct,
            'norm_atr_pct': norm_atr_pct,
            'volatility_range_pct': volatility_range_pct,
            'volume_ratio': volume_ratio,
            'entry_hour': entry_hour,
            'dist_to_100low_pct': dist_to_100low_pct,
            'last_bar_body_ratio': last_bar_body_ratio,
            'last_bar_is_bullish': last_bar_is_bullish,
        }

        X_vec = np.array([[feat_dict[col] for col in self.feature_names]])
        return X_vec, feat_dict

    def predict(self, bars, threshold=0.50):
        X_vec, feat_dict = self.extract_features_from_bars(bars)
        X_scaled = self.scaler.transform(X_vec)
        prob_win = float(self.model.predict_proba(X_scaled)[0, 1])
        
        recommendation = "TRADE_RECOMMENDED" if prob_win >= threshold else "SKIP_TRADE"

        return {
            "prediction": recommendation,
            "win_probability": round(prob_win, 4),
            "threshold": threshold,
            "features": feat_dict
        }

def test_live_binance_inference():
    print("==========================================================================================")
    print("⚡ CANLI PİYASA MUM VERİSİ ÜZERİNDE YAPAY ZEKA İNFERANCE (TAHMİN) TESTİ")
    print("==========================================================================================")
    
    url = "https://fapi.binance.com/fapi/v1/klines?symbol=TACUSDT&interval=1h&limit=100"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
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

    engine = MLInferenceEngine()
    result = engine.predict(bars, threshold=0.50)

    print(f"📊 Sembol: TACUSDT 1h | Son Kapanış Fiyatı: {bars[-1]['close']}")
    print(f"🎯 Yapay Zeka Kazanma Olasılığı: %{result['win_probability']*100:.2f}")
    print(f"🚀 Sinyal Kararı              : {result['prediction']}")
    print("------------------------------------------------------------------------------------------")
    print("Çıkarılan Canlı Öznitelikler (Features):")
    for k, v in result['features'].items():
        print(f"  • {k:<22}: {v:.4f}")
    print("==========================================================================================\n")

if __name__ == "__main__":
    test_live_binance_inference()
