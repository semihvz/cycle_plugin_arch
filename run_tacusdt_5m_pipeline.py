#!/usr/bin/env python3
import urllib.request
import json
import sqlite3
import datetime
import os
import pandas as pd
import numpy as np
import joblib
from sklearn.ensemble import HistGradientBoostingClassifier, RandomForestClassifier
from sklearn.preprocessing import StandardScaler
from sklearn.model_selection import StratifiedKFold, cross_val_predict
from sklearn.metrics import roc_auc_score

def fetch_klines(symbol="TACUSDT", interval="5m", limit=1500):
    url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval={interval}&limit={limit}"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
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

def calculate_atr(bars, period=14):
    tr_list = []
    for i in range(len(bars)):
        if i == 0:
            tr = bars[i]['high'] - bars[i]['low']
        else:
            hl = bars[i]['high'] - bars[i]['low']
            hp = abs(bars[i]['high'] - bars[i - 1]['close'])
            lp = abs(bars[i]['low'] - bars[i - 1]['close'])
            tr = max(hl, hp, lp)
        tr_list.append(tr)
    
    atr = [0.0] * len(bars)
    if len(bars) < period:
        return atr
    
    first_sma = sum(tr_list[:period]) / period
    atr[period - 1] = first_sma
    prev_atr = first_sma
    for i in range(period, len(bars)):
        curr_atr = (prev_atr * (period - 1) + tr_list[i]) / period
        atr[i] = curr_atr
        prev_atr = curr_atr
    return atr

def main():
    symbol = "TACUSDT"
    interval = "5m"
    fixed_pos_size = 50.0
    lookback = 100
    data_dir = "/home/smhvz/Desktop/cycle-orc/data"
    os.makedirs(data_dir, exist_ok=True)
    db_path = os.path.join(data_dir, "tacusdt_5m_collector.db")

    print("==========================================================================================")
    print(f"⚡ TACUSDT {interval} (5-DAKİKALIK MUM) VERİ TOPLAMA, SQLITE KAYIT VE ML PİPELİNE")
    print("==========================================================================================")

    if os.path.exists(db_path):
        os.remove(db_path)

    print(f"Fetching historical {symbol} {interval} bars from Binance Futures...")
    bars = fetch_klines(symbol, interval, 1500)
    print(f"Total {interval} bars fetched: {len(bars)}")

    atr_series = calculate_atr(bars, 14)

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    cursor.execute("""
    CREATE TABLE closed_trades (
        trade_id INTEGER PRIMARY KEY,
        symbol TEXT NOT NULL,
        side TEXT NOT NULL,
        entry_time_utc TEXT NOT NULL,
        entry_unix_ms INTEGER NOT NULL,
        entry_unix_sec INTEGER NOT NULL,
        exit_time_utc TEXT NOT NULL,
        exit_unix_ms INTEGER NOT NULL,
        exit_unix_sec INTEGER NOT NULL,
        entry_price REAL NOT NULL,
        lowest_100_price REAL NOT NULL,
        atr_14 REAL NOT NULL,
        stop_loss_price REAL NOT NULL,
        take_profit_price REAL NOT NULL,
        exit_price REAL NOT NULL,
        position_size_usdt REAL NOT NULL,
        risk_usdt REAL NOT NULL,
        target_reward_usdt REAL NOT NULL,
        result TEXT NOT NULL,
        pnl_usdt REAL NOT NULL,
        pnl_percent REAL NOT NULL,
        holding_bars INTEGER NOT NULL
    );
    """)

    cursor.execute("""
    CREATE TABLE trade_lookback_bars (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        trade_id INTEGER NOT NULL,
        bar_offset INTEGER NOT NULL,
        open_time_ms INTEGER NOT NULL,
        open_time_utc TEXT NOT NULL,
        open REAL NOT NULL,
        high REAL NOT NULL,
        low REAL NOT NULL,
        close REAL NOT NULL,
        volume REAL NOT NULL,
        close_time_ms INTEGER NOT NULL,
        FOREIGN KEY (trade_id) REFERENCES closed_trades (trade_id)
    );
    """)

    trade_id = 1
    trade_rows = []
    lookback_rows = []
    features_list = []

    for i in range(lookback, len(bars)):
        entry_bar = bars[i]
        entry_price = entry_bar['open']
        entry_time_ms = entry_bar['open_time']
        entry_time_sec = entry_time_ms // 1000
        entry_time_utc = datetime.datetime.fromtimestamp(entry_time_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')

        window_100 = bars[i - lookback : i]
        lowest_100 = min(b['low'] for b in window_100)
        highest_100 = max(b['high'] for b in window_100)
        atr_val = atr_series[i - 1] if i > 0 else atr_series[i]
        atr_val = max(atr_val, 0.00001)

        raw_sl = lowest_100 - (2.0 * atr_val)
        sl_dist = max(entry_price - raw_sl, entry_price * 0.005)
        stop_loss = entry_price - sl_dist
        take_profit = entry_price + (2.0 * sl_dist)

        risk_ratio = sl_dist / entry_price
        risk_usdt = fixed_pos_size * risk_ratio
        reward_usdt = 2.0 * risk_usdt

        closed = False
        exit_time_ms = None
        exit_time_sec = None
        exit_time_utc = None
        exit_price = None
        status = None
        pnl_usdt = 0.0
        pnl_pct = 0.0
        holding_bars = 0

        for k in range(i, len(bars)):
            sim_bar = bars[k]
            holding_bars = k - i + 1

            if sim_bar['low'] <= stop_loss and sim_bar['high'] >= take_profit:
                closed = True
                exit_time_ms = sim_bar['close_time']
                exit_time_sec = exit_time_ms // 1000
                exit_time_utc = datetime.datetime.fromtimestamp(exit_time_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')
                exit_price = stop_loss
                status = "LOSS"
                pnl_usdt = -risk_usdt
                pnl_pct = -risk_ratio * 100.0
                break
            elif sim_bar['high'] >= take_profit:
                closed = True
                exit_time_ms = sim_bar['close_time']
                exit_time_sec = exit_time_ms // 1000
                exit_time_utc = datetime.datetime.fromtimestamp(exit_time_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')
                exit_price = take_profit
                status = "WIN"
                pnl_usdt = reward_usdt
                pnl_pct = 2.0 * risk_ratio * 100.0
                break
            elif sim_bar['low'] <= stop_loss:
                closed = True
                exit_time_ms = sim_bar['close_time']
                exit_time_sec = exit_time_ms // 1000
                exit_time_utc = datetime.datetime.fromtimestamp(exit_time_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')
                exit_price = stop_loss
                status = "LOSS"
                pnl_usdt = -risk_usdt
                pnl_pct = -risk_ratio * 100.0
                break

        if closed:
            trade_rows.append((
                trade_id, symbol, 'LONG', entry_time_utc, entry_time_ms, entry_time_sec,
                exit_time_utc, exit_time_ms, exit_time_sec, round(entry_price, 5),
                round(lowest_100, 5), round(atr_val, 5), round(stop_loss, 5),
                round(take_profit, 5), round(exit_price, 5), fixed_pos_size,
                round(risk_usdt, 2), round(reward_usdt, 2), status, round(pnl_usdt, 2),
                round(pnl_pct, 2), holding_bars
            ))

            for idx_off, l_bar in enumerate(window_100):
                offset = idx_off - 100
                bar_open_sec = l_bar['open_time'] // 1000
                bar_open_utc = datetime.datetime.fromtimestamp(bar_open_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')

                lookback_rows.append((
                    trade_id, offset, l_bar['open_time'], bar_open_utc,
                    round(l_bar['open'], 5), round(l_bar['high'], 5),
                    round(l_bar['low'], 5), round(l_bar['close'], 5),
                    round(l_bar['volume'], 2), l_bar['close_time']
                ))

            closes_arr = np.array([b['close'] for b in window_100])
            opens_arr = np.array([b['open'] for b in window_100])
            highs_arr = np.array([b['high'] for b in window_100])
            lows_arr = np.array([b['low'] for b in window_100])
            vols_arr = np.array([b['volume'] for b in window_100])

            trend_100b_pct = ((closes_arr[-1] - closes_arr[0]) / closes_arr[0]) * 100.0
            trend_50b_pct = ((closes_arr[-1] - closes_arr[-50]) / closes_arr[-50]) * 100.0
            trend_20b_pct = ((closes_arr[-1] - closes_arr[-20]) / closes_arr[-20]) * 100.0
            stoch_pos_pct = ((entry_price - lowest_100) / max(highest_100 - lowest_100, 1e-8)) * 100.0
            norm_atr_pct = (atr_val / entry_price) * 100.0
            volatility_range_pct = ((highest_100 - lowest_100) / entry_price) * 100.0
            vol_10_mean = vols_arr[-10:].mean()
            vol_100_mean = vols_arr.mean()
            volume_ratio = vol_10_mean / max(vol_100_mean, 1e-8)
            entry_hour = int(entry_time_utc.split()[1].split(':')[0])
            dist_to_100low_pct = ((entry_price - lowest_100) / entry_price) * 100.0
            last_body = abs(closes_arr[-1] - opens_arr[-1])
            last_range = max(highs_arr[-1] - lows_arr[-1], 1e-8)
            last_bar_body_ratio = last_body / last_range
            last_bar_is_bullish = 1 if closes_arr[-1] > opens_arr[-1] else 0

            target = 1 if status == 'WIN' else 0

            features_list.append({
                'trade_id': trade_id,
                'target': target,
                'pnl_usdt': pnl_usdt,
                'risk_usdt': risk_usdt,
                'reward_usdt': reward_usdt,
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
            })

        trade_id += 1

    cursor.executemany("INSERT INTO closed_trades VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);", trade_rows)
    cursor.executemany("INSERT INTO trade_lookback_bars (trade_id, bar_offset, open_time_ms, open_time_utc, open, high, low, close, volume, close_time_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);", lookback_rows)
    conn.commit()
    conn.close()

    df = pd.DataFrame(features_list)
    db_size_mb = os.path.getsize(db_path) / (1024 * 1024)

    print(f"\n📊 {symbol} {interval} Backtest ve Veri Toplama Tamamlandı:")
    print(f"   • Toplam Kapanmış İşlem : {len(df)} adet 5m işlem")
    print(f"   • Saklanan 100-Bar Mum   : {len(lookback_rows)} satır")
    print(f"   • SQLite Veritabanı     : {db_path} ({db_size_mb:.2f} MB)")
    print(f"   • Ham Win Rate          : %{(df['target'].mean() * 100):.2f}% ({df['target'].sum()} WIN / {len(df) - df['target'].sum()} LOSS)")
    print(f"   • Ham Net PnL           : {df['pnl_usdt'].sum():+.2f} USDT\n")

    # Machine Learning Model Training on 5m
    feature_cols = [
        'trend_100b_pct', 'trend_50b_pct', 'trend_20b_pct', 'stoch_pos_pct',
        'norm_atr_pct', 'volatility_range_pct', 'volume_ratio',
        'entry_hour', 'dist_to_100low_pct', 'last_bar_body_ratio',
        'last_bar_is_bullish'
    ]

    X = df[feature_cols]
    y = df['target']

    scaler = StandardScaler()
    X_scaled = scaler.fit_transform(X)

    cv = StratifiedKFold(n_splits=5, shuffle=True, random_state=42)
    rf = HistGradientBoostingClassifier(max_iter=100, max_depth=5, random_state=42)
    y_prob = cross_val_predict(rf, X_scaled, y, cv=cv, method='predict_proba')[:, 1]
    
    auc = roc_auc_score(y, y_prob)

    print("------------------------------------------------------------------------------------------")
    print(f"🤖 MAKİNE ÖĞRENMESİ ({symbol} {interval} MODELİ) PERFORMANSI:")
    print("------------------------------------------------------------------------------------------")
    print(f"  • ROC-AUC Skoru: {auc:.4f}")
    
    thresholds = [0.40, 0.50, 0.55, 0.60]
    print(f"{'Eşik (Threshold)':<18} | {'İşlem Sayısı':<12} | {'Win Rate (%)':<14} | {'Net PnL (USDT)':<16} | {'Profit Factor':<14}")
    print("-" * 85)

    base_pnl = df['pnl_usdt'].sum()
    base_gw = df[df['pnl_usdt'] > 0]['pnl_usdt'].sum()
    base_gl = abs(df[df['pnl_usdt'] < 0]['pnl_usdt'].sum())
    base_pf = base_gw / base_gl if base_gl > 0 else base_gw

    print(f"{'Ham Strateji (5m)':<18} | {len(df):<12} | %{(y.mean()*100):<13.2f} | {base_pnl:<+15.2f} | {base_pf:<13.2f}")

    for th in thresholds:
        f_df = df[y_prob >= th]
        if len(f_df) == 0:
            continue
        f_win_rate = (f_df['target'].sum() / len(f_df)) * 100.0
        f_pnl = f_df['pnl_usdt'].sum()
        f_gw = f_df[f_df['pnl_usdt'] > 0]['pnl_usdt'].sum()
        f_gl = abs(f_df[f_df['pnl_usdt'] < 0]['pnl_usdt'].sum())
        f_pf = f_gw / f_gl if f_gl > 0 else f_gw

        print(f"ML Prob >= {th:<9.2f} | {len(f_df):<12} | %{f_win_rate:<13.2f} | {f_pnl:<+15.2f} | {f_pf:<13.2f}")
    print("------------------------------------------------------------------------------------------\n")

    rf.fit(X_scaled, y)
    joblib.dump(rf, "/home/smhvz/Desktop/cycle-orc/ml_model_suite/models/tacusdt_5m_ml_model.joblib")
    joblib.dump(scaler, "/home/smhvz/Desktop/cycle-orc/ml_model_suite/models/scaler_5m.joblib")

    # Live Inference on latest 5m Binance Futures Candles
    print("==========================================================================================")
    print(f"⚡ {symbol} 5m CANLI PİYASA İNFERENCE (TAHMİN) TESTİ:")
    print("==========================================================================================")
    bars_live = fetch_klines(symbol, "5m", 120)
    
    closes_live = np.array([b['close'] for b in bars_live])
    opens_live = np.array([b['open'] for b in bars_live])
    highs_live = np.array([b['high'] for b in bars_live])
    lows_live = np.array([b['low'] for b in bars_live])
    vols_live = np.array([b['volume'] for b in bars_live])

    entry_p = closes_live[-1]
    low_100_live = lows_live.min()
    high_100_live = highs_live.max()

    tr_list = []
    for idx in range(len(bars_live)):
        if idx == 0:
            tr = highs_live[idx] - lows_live[idx]
        else:
            hl = highs_live[idx] - lows_live[idx]
            hp = abs(highs_live[idx] - closes_live[idx - 1])
            lp = abs(lows_live[idx] - closes_live[idx - 1])
            tr = max(hl, hp, lp)
        tr_list.append(tr)
    atr_live = sum(tr_list[-14:]) / 14.0

    live_feat = {
        'trend_100b_pct': ((closes_live[-1] - closes_live[0]) / closes_live[0]) * 100.0,
        'trend_50b_pct': ((closes_live[-1] - closes_live[-50]) / closes_live[-50]) * 100.0,
        'trend_20b_pct': ((closes_live[-1] - closes_live[-20]) / closes_live[-20]) * 100.0,
        'stoch_pos_pct': ((entry_p - low_100_live) / max(high_100_live - low_100_live, 1e-8)) * 100.0,
        'norm_atr_pct': (atr_live / entry_p) * 100.0,
        'volatility_range_pct': ((high_100_live - low_100_live) / entry_p) * 100.0,
        'volume_ratio': vols_live[-10:].mean() / max(vols_live.mean(), 1e-8),
        'entry_hour': datetime.datetime.now(datetime.timezone.utc).hour,
        'dist_to_100low_pct': ((entry_p - low_100_live) / entry_p) * 100.0,
        'last_bar_body_ratio': abs(closes_live[-1] - opens_live[-1]) / max(highs_live[-1] - lows_live[-1], 1e-8),
        'last_bar_is_bullish': 1 if closes_live[-1] > opens_live[-1] else 0,
    }

    X_live = np.array([[live_feat[c] for c in feature_cols]])
    X_live_scaled = scaler.transform(X_live)
    live_win_prob = rf.predict_proba(X_live_scaled)[0, 1]

    signal = "TRADE_RECOMMENDED (LONG)" if live_win_prob >= 0.50 else "SKIP_TRADE (PAS GEÇ)"

    print(f"📊 Sembol: {symbol} 5m | Canlı Fiyat: {entry_p} USDT")
    print(f"🎯 Yapay Zeka TACUSDT 5m Kazanma Olasılığı: %{live_win_prob*100:.2f}")
    print(f"🚀 Canlı Sinyal Kararı                     : {signal}")
    print("==========================================================================================\n")

if __name__ == "__main__":
    main()
