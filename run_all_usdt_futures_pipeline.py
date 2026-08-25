#!/usr/bin/env python3
import urllib.request
import json
import sqlite3
import datetime
import os
import time
import pandas as pd
import numpy as np
import joblib
from sklearn.ensemble import HistGradientBoostingClassifier
from sklearn.preprocessing import StandardScaler
from sklearn.model_selection import StratifiedKFold, cross_val_predict
from sklearn.metrics import roc_auc_score

def get_all_usdt_futures_symbols():
    url = "https://fapi.binance.com/fapi/v1/exchangeInfo"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
    try:
        with urllib.request.urlopen(req) as resp:
            data = json.loads(resp.read().decode('utf-8'))
        symbols = []
        for s in data.get('symbols', []):
            if s.get('quoteAsset') == 'USDT' and s.get('status') == 'TRADING' and s.get('contractType') == 'PERPETUAL':
                symbols.append(s['symbol'])
        return sorted(symbols)
    except Exception as e:
        print(f"Error fetching exchangeInfo: {e}")
        return ["BTCUSDT", "ETHUSDT", "SOLUSDT", "AVAXUSDT", "LINKUSDT", "DOGEUSDT", "ADAUSDT", "TACUSDT", "VELVETUSDT"]

def fetch_klines(symbol, interval="1h", limit=1500):
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
        return []

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
    data_dir = "/home/smhvz/Desktop/cycle-orc/data"
    os.makedirs(data_dir, exist_ok=True)
    db_path = os.path.join(data_dir, "all_symbols_futures_1h.db")
    csv_path = os.path.join(data_dir, "all_symbols_closed_trades.csv")
    excel_path = os.path.join(data_dir, "all_symbols_closed_trades.xlsx")

    print("==========================================================================================")
    print("🔥 ALL BINANCE FUTURES USDT PAIRS (1h MÜM) BÜYÜK VERİ TOPLAMA VE ML PİPELİNE")
    print("==========================================================================================")

    if os.path.exists(db_path):
        os.remove(db_path)

    print("Binance Futures borsa verisindeki tüm aktif USDT pariteleri sorgulanıyor...")
    all_symbols = get_all_usdt_futures_symbols()
    print(f"Toplam Aktif USDT Paritesi Bulundu: {len(all_symbols)} adet parite\n")

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    cursor.execute("""
    CREATE TABLE closed_trades (
        trade_id INTEGER PRIMARY KEY AUTOINCREMENT,
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
        symbol TEXT NOT NULL,
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
    conn.commit()

    global_trade_id = 1
    total_trades_count = 0
    total_lookback_count = 0
    all_trade_rows = []
    features_list = []
    processed_symbols = 0

    fixed_pos_size = 50.0
    lookback = 100

    print("Tüm USDT pariteleri sırayla işleniyor...")

    for sym_idx, symbol in enumerate(all_symbols):
        bars = fetch_klines(symbol, "1h", 1500)
        if len(bars) < 150:
            continue

        atr_series = calculate_atr(bars, 14)
        sym_trades = 0

        sym_trade_rows = []
        sym_lookback_rows = []

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
                sym_trade_rows.append((
                    global_trade_id, symbol, 'LONG', entry_time_utc, entry_time_ms, entry_time_sec,
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

                    sym_lookback_rows.append((
                        global_trade_id, symbol, offset, l_bar['open_time'], bar_open_utc,
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
                    'trade_id': global_trade_id,
                    'symbol': symbol,
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

                global_trade_id += 1
                sym_trades += 1

        cursor.executemany("INSERT INTO closed_trades VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);", sym_trade_rows)
        cursor.executemany("INSERT INTO trade_lookback_bars (trade_id, symbol, bar_offset, open_time_ms, open_time_utc, open, high, low, close, volume, close_time_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);", sym_lookback_rows)
        conn.commit()

        all_trade_rows.extend(sym_trade_rows)
        total_trades_count += sym_trades
        total_lookback_count += len(sym_lookback_rows)
        processed_symbols += 1

        if (sym_idx + 1) % 25 == 0 or (sym_idx + 1) == len(all_symbols):
            print(f"  [{sym_idx+1}/{len(all_symbols)}] İşlendi: {processed_symbols} parite | Toplam İşlem: {total_trades_count:,} | Bakılan Mum: {total_lookback_count:,}")

    conn.close()

    df = pd.DataFrame(features_list)
    db_size_mb = os.path.getsize(db_path) / (1024 * 1024)

    # Save Master CSV and Excel
    df_export = pd.DataFrame(all_trade_rows, columns=[
        'trade_id', 'symbol', 'side', 'entry_time_utc', 'entry_unix_ms', 'entry_unix_sec',
        'exit_time_utc', 'exit_unix_ms', 'exit_unix_sec', 'entry_price',
        'lowest_100_price', 'atr_14', 'stop_loss_price', 'take_profit_price',
        'exit_price', 'position_size_usdt', 'risk_usdt', 'target_reward_usdt',
        'result', 'pnl_usdt', 'pnl_percent', 'holding_bars'
    ])
    df_export.to_csv(csv_path, index=False, encoding='utf-8-sig')
    
    # Save first 50,000 trades to Excel to avoid Excel row limit crash
    df_export.head(50000).to_excel(excel_path, index=False, engine='openpyxl')

    print(f"\n==========================================================================================")
    print(f"📊 TÜM BINANCE FUTURES USDT PARİTELERİ 1h SONUÇ RAPORU:")
    print(f"==========================================================================================")
    print(f"   • Toplam İşlenen Parite  : {processed_symbols} adet USDT paritesi")
    print(f"   • Toplam Kapanmış İşlem  : {len(df):,} adet işlem")
    print(f"   • Toplam 100-Bar Mum     : {total_lookback_count:,} satır veritabanı kaydı")
    print(f"   • SQLite Veritabanı      : {db_path} ({db_size_mb:.2f} MB)")
    print(f"   • Master CSV Dosyası     : {csv_path} ({(os.path.getsize(csv_path)/(1024*1024)):.2f} MB)")
    print(f"   • Master Excel Dosyası   : {excel_path}")
    print(f"   • Ham Win Rate           : %{(df['target'].mean() * 100):.2f}% ({df['target'].sum():,} WIN / {len(df) - df['target'].sum():,} LOSS)")
    print(f"   • Ham Net PnL            : {df['pnl_usdt'].sum():+,.2f} USDT\n")

    # Train Multi-Asset Machine Learning Model
    print("------------------------------------------------------------------------------------------")
    print("🤖 TÜM PARİTELER ÜZERİNDE ÇOKLU-VARLIK (MULTI-ASSET) YAPAY ZEKA MODELİ EĞİTİLİYOR...")
    print("------------------------------------------------------------------------------------------")

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
    rf = HistGradientBoostingClassifier(max_iter=100, max_depth=6, random_state=42)
    y_prob = cross_val_predict(rf, X_scaled, y, cv=cv, method='predict_proba')[:, 1]

    auc = roc_auc_score(y, y_prob)
    print(f"  • Multi-Asset ROC-AUC Skoru: {auc:.4f}\n")

    thresholds = [0.40, 0.50, 0.55, 0.60]
    print(f"{'Eşik (Threshold)':<18} | {'İşlem Sayısı':<12} | {'Win Rate (%)':<14} | {'Net PnL (USDT)':<18} | {'Profit Factor':<14}")
    print("-" * 90)

    base_pnl = df['pnl_usdt'].sum()
    base_gw = df[df['pnl_usdt'] > 0]['pnl_usdt'].sum()
    base_gl = abs(df[df['pnl_usdt'] < 0]['pnl_usdt'].sum())
    base_pf = base_gw / base_gl if base_gl > 0 else base_gw

    print(f"{'Ham Strateji (Hepsi)':<18} | {len(df):<12,} | %{(y.mean()*100):<13.2f} | {base_pnl:<+17,.2f} | {base_pf:<13.2f}")

    for th in thresholds:
        f_df = df[y_prob >= th]
        if len(f_df) == 0:
            continue
        f_win_rate = (f_df['target'].sum() / len(f_df)) * 100.0
        f_pnl = f_df['pnl_usdt'].sum()
        f_gw = f_df[f_df['pnl_usdt'] > 0]['pnl_usdt'].sum()
        f_gl = abs(f_df[f_df['pnl_usdt'] < 0]['pnl_usdt'].sum())
        f_pf = f_gw / f_gl if f_gl > 0 else f_gw

        print(f"ML Prob >= {th:<9.2f} | {len(f_df):<12,} | %{f_win_rate:<13.2f} | {f_pnl:<+17,.2f} | {f_pf:<13.2f}")
    print("------------------------------------------------------------------------------------------\n")

    rf.fit(X_scaled, y)
    models_dir = "/home/smhvz/Desktop/cycle-orc/ml_model_suite/models"
    os.makedirs(models_dir, exist_ok=True)
    joblib.dump(rf, os.path.join(models_dir, "multi_asset_1h_ml_model.joblib"))
    joblib.dump(scaler, os.path.join(models_dir, "multi_asset_scaler.joblib"))
    print("✅ Çoklu-Varlık Yapay Zeka Modeli Kaydedildi: 'multi_asset_1h_ml_model.joblib'")
    print("==========================================================================================")

if __name__ == "__main__":
    main()
