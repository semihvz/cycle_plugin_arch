#!/usr/bin/env python3
"""
MAGMAUSDT 1m (1-Dakikalık Mum) Son 1 Aylık Veri Toplama, All-Bars Simülasyonu ve Makine Öğrenmesi Pipeline
------------------------------------------------------------------------------------------------------
1. Binance Futures REST API'den son 1 aya ait (~43,200 adet) MAGMAUSDT 1m mum verilerini çeker (Paginated).
2. All-Bars stratejisi ile 100-bar lookback penceresi kullanarak tüm işlemleri simüle eder.
3. Kapalı işlemleri ve 100-bar geçmişini SQLite veritabanına, CSV ve Excel dosyalarına kaydeder.
4. 11 adet teknik öznitelik çıkararak 5-Fold Stratified Cross Validation ile ML modellerini eğitir ve değerlendirir.
5. En iyi modeli serileştirir ve sıfır gecikmeli C/Rust filtre kodunu üretir.
6. Canlı mum verileri üzerinde anlık ML tahmini (inference) gerçekleştirir.
"""

import urllib.request
import json
import sqlite3
import datetime
import os
import time
import pandas as pd
import numpy as np
import joblib
from sklearn.ensemble import RandomForestClassifier, ExtraTreesClassifier, HistGradientBoostingClassifier
from sklearn.tree import DecisionTreeClassifier
from sklearn.preprocessing import StandardScaler
from sklearn.model_selection import StratifiedKFold, cross_val_predict
from sklearn.metrics import roc_auc_score


def fetch_klines_1month(symbol="MAGMAUSDT", interval="1m", target_days=30):
    """
    Binance Futures REST API'den son `target_days` günlük mum verisini
    1500'erlik dilimlerle (pagination) çekerek birleştirir.
    """
    total_minutes = target_days * 24 * 60
    all_bars = []
    end_time = None
    chunk_size = 1500

    print(f"📥 Binance Futures API'den {symbol} {interval} (Son {target_days} Gün, ~{total_minutes} mum) indiriliyor...")

    fetched_count = 0
    while len(all_bars) < total_minutes:
        url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval={interval}&limit={chunk_size}"
        if end_time:
            url += f"&endTime={end_time}"

        req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
        try:
            with urllib.request.urlopen(req, timeout=15) as resp:
                data = json.loads(resp.read().decode('utf-8'))
        except Exception as e:
            print(f"⚠️ Hata: {e}. Retry ediliyor...")
            time.sleep(1)
            continue

        if not data:
            break

        chunk_bars = []
        for row in data:
            chunk_bars.append({
                'open_time': int(row[0]),
                'open': float(row[1]),
                'high': float(row[2]),
                'low': float(row[3]),
                'close': float(row[4]),
                'volume': float(row[5]),
                'close_time': int(row[6]),
            })

        # API geriye doğru veri döndürür veya endTime ile geriye gidilir
        first_bar_time = chunk_bars[0]['open_time']
        end_time = first_bar_time - 1

        all_bars = chunk_bars + all_bars
        fetched_count += len(chunk_bars)
        print(f"   • Toplam çekilen mum: {len(all_bars)} / ~{total_minutes}")

        if len(chunk_bars) < chunk_size:
            print("   • Binance tarafındaki tüm mevcut geçmiş veriye ulaşıldı.")
            break

        time.sleep(0.1)

    # Zaman sırasına göre sıralandığından emin ol
    all_bars = sorted(all_bars, key=lambda x: x['open_time'])
    print(f"✅ İndirme Tamamlandı! Toplam Sıralı {interval} Mum Sayısı: {len(all_bars)}\n")
    return all_bars


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


def generate_rust_decision_tree(tree, feature_names, symbol="MAGMAUSDT"):
    tree_ = tree.tree_
    feature_name = [
        feature_names[i] if i != -2 else "undefined!"
        for i in tree_.feature
    ]
    
    lines = []
    lines.append(f"// Auto-generated Zero-Latency C/Rust ML Filter for {symbol} 1m Data")
    lines.append("#[derive(Debug, Clone, Copy)]")
    lines.append("pub struct MagmaUSDT1mMLFeatures {")
    for fname in feature_names:
        lines.append(f"    pub {fname}: f64,")
    lines.append("}")
    lines.append("")
    lines.append("pub fn evaluate_magmausdt_1m_filter(f: &MagmaUSDT1mMLFeatures) -> (bool, f64) {")
    
    def recurse(node, depth):
        indent = "    " * (depth + 1)
        if tree_.feature[node] != -2:
            name = feature_name[node]
            threshold = tree_.threshold[node]
            lines.append(f"{indent}if f.{name} <= {threshold:.5f} {{")
            recurse(tree_.children_left[node], depth + 1)
            lines.append(f"{indent}}} else {{")
            recurse(tree_.children_right[node], depth + 1)
            lines.append(f"{indent}}}")
        else:
            value = tree_.value[node][0]
            total = value.sum()
            win_prob = value[1] / total if total > 0 else 0.0
            approved = "true" if win_prob >= 0.50 else "false"
            lines.append(f"{indent}return ({approved}, {win_prob:.4f});")

    recurse(0, 0)
    lines.append("}")
    return "\n".join(lines)


def main():
    symbol = "MAGMAUSDT"
    interval = "1m"
    target_days = 30
    fixed_pos_size = 50.0
    lookback = 100

    base_dir = "/home/smhvz/Desktop/cycle-orc"
    data_dir = os.path.join(base_dir, "data")
    all_bars_out_dir = os.path.join(base_dir, "all_bars_system", "output")
    models_dir = os.path.join(base_dir, "ml_model_suite", "models")
    gen_dir = os.path.join(base_dir, "ml_model_suite", "generated")

    os.makedirs(data_dir, exist_ok=True)
    os.makedirs(all_bars_out_dir, exist_ok=True)
    os.makedirs(models_dir, exist_ok=True)
    os.makedirs(gen_dir, exist_ok=True)

    db_path = os.path.join(data_dir, "magmausdt_1m_collector.db")
    db_path_allbars = os.path.join(all_bars_out_dir, "magmausdt_1m_all_bars_backtest.db")
    csv_path = os.path.join(base_dir, "magmausdt_closed_trades.csv")
    excel_path = os.path.join(base_dir, "magmausdt_closed_trades.xlsx")

    print("==========================================================================================")
    print(f"🔥 {symbol} {interval} (SON 1 AY) VERİ TOPLAMA, SIMÜLASYON VE MAKİNE ÖĞRENMESİ MODELİ")
    print("==========================================================================================")

    # 1. Veri İndirme (1 Ay = ~43,200 1m mum)
    bars = fetch_klines_1month(symbol, interval, target_days)

    if len(bars) < lookback + 10:
        raise ValueError(f"Yetersiz veri çekildi ({len(bars)} mum). En az {lookback + 10} mum gerekli.")

    atr_series = calculate_atr(bars, 14)

    # 2. SQLite Veritabanı Hazırlama
    for target_db in [db_path, db_path_allbars]:
        if os.path.exists(target_db):
            os.remove(target_db)

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

    # 3. All-Bars Strateji Simülasyonu
    print(f"⚙️ {len(bars)} mum üzerinde 100-bar lookback simülasyonu çalıştırılıyor...")
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
        atr_val = max(atr_val, 1e-8)

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
                exit_time_utc, exit_time_ms, exit_time_sec, round(entry_price, 6),
                round(lowest_100, 6), round(atr_val, 6), round(stop_loss, 6),
                round(take_profit, 6), round(exit_price, 6), fixed_pos_size,
                round(risk_usdt, 4), round(reward_usdt, 4), status, round(pnl_usdt, 4),
                round(pnl_pct, 2), holding_bars
            ))

            for idx_off, l_bar in enumerate(window_100):
                offset = idx_off - 100
                bar_open_sec = l_bar['open_time'] // 1000
                bar_open_utc = datetime.datetime.fromtimestamp(bar_open_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')

                lookback_rows.append((
                    trade_id, offset, l_bar['open_time'], bar_open_utc,
                    round(l_bar['open'], 6), round(l_bar['high'], 6),
                    round(l_bar['low'], 6), round(l_bar['close'], 6),
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
            entry_hour = int(entry_time_utc.split()[1].split(':')[0]) if ' ' in entry_time_utc else 0
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
                'trend_100b_pct': round(trend_100b_pct, 4),
                'trend_50b_pct': round(trend_50b_pct, 4),
                'trend_20b_pct': round(trend_20b_pct, 4),
                'stoch_pos_pct': round(stoch_pos_pct, 4),
                'norm_atr_pct': round(norm_atr_pct, 4),
                'volatility_range_pct': round(volatility_range_pct, 4),
                'volume_ratio': round(volume_ratio, 4),
                'entry_hour': entry_hour,
                'dist_to_100low_pct': round(dist_to_100low_pct, 4),
                'last_bar_body_ratio': round(last_bar_body_ratio, 4),
                'last_bar_is_bullish': last_bar_is_bullish,
            })

            trade_id += 1

    cursor.executemany("INSERT INTO closed_trades VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);", trade_rows)
    cursor.executemany("INSERT INTO trade_lookback_bars (trade_id, bar_offset, open_time_ms, open_time_utc, open, high, low, close, volume, close_time_ms) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);", lookback_rows)
    conn.commit()
    conn.close()

    # Kopyasını all_bars_system/output/ altına da yaz
    import shutil
    shutil.copyfile(db_path, db_path_allbars)

    df = pd.DataFrame(features_list)

    # CSV ve Excel çıktısı al
    df_export = pd.DataFrame(trade_rows, columns=[
        'trade_id', 'symbol', 'side', 'entry_time_utc', 'entry_unix_ms', 'entry_unix_sec',
        'exit_time_utc', 'exit_unix_ms', 'exit_unix_sec', 'entry_price',
        'lowest_100_price', 'atr_14', 'stop_loss_price', 'take_profit_price',
        'exit_price', 'position_size_usdt', 'risk_usdt', 'target_reward_usdt',
        'result', 'pnl_usdt', 'pnl_percent', 'holding_bars'
    ])
    df_export.to_csv(csv_path, index=False, encoding='utf-8-sig')
    try:
        df_export.to_excel(excel_path, index=False, engine='openpyxl')
    except Exception as e:
        print(f"⚠️ Excel aktarım uyarısı: {e}")

    db_size_mb = os.path.getsize(db_path) / (1024 * 1024)

    print(f"📊 {symbol} 1m (Son 1 Ay) Simülasyon Özet Raporu:")
    print(f"   • İndirilen Mum Sayısı    : {len(bars)} adet 1m bar")
    print(f"   • Simüle Edilen İşlem     : {len(df)} adet kapanmış işlem")
    print(f"   • Saklanan Lookback Barları: {len(lookback_rows)} satır")
    print(f"   • SQLite Veritabanı       : {db_path} ({db_size_mb:.2f} MB)")
    print(f"   • CSV Dökümü              : {csv_path}")
    print(f"   • Ham Win Rate            : %{(df['target'].mean() * 100):.2f}% ({df['target'].sum()} WIN / {len(df) - df['target'].sum()} LOSS)")
    print(f"   • Ham Net PnL             : {df['pnl_usdt'].sum():+.2f} USDT\n")

    # 4. Makine Öğrenmesi Model Eğitimi & Çoklu Algoritma Değerlendirmesi
    feature_cols = [
        'trend_100b_pct', 'trend_50b_pct', 'trend_20b_pct', 'stoch_pos_pct',
        'norm_atr_pct', 'volatility_range_pct', 'volume_ratio',
        'entry_hour', 'dist_to_100low_pct', 'last_bar_body_ratio',
        'last_bar_is_bullish'
    ]

    X = df[feature_cols]
    y = df['target']

    print("==========================================================================================")
    print(f"🤖 MAKİNE ÖĞRENMESİ ({symbol} 1m MODELİ) EĞİTİMİ VE ALGORİTMA KARŞILAŞTIRMASI")
    print("==========================================================================================")
    print(f"Toplam İşlem Sayısı: {len(X)} | WIN Etiket Sayısı: {y.sum()} | LOSS Etiket Sayısı: {len(y) - y.sum()}\n")

    scaler = StandardScaler()
    X_scaled = scaler.fit_transform(X)

    cv = StratifiedKFold(n_splits=5, shuffle=True, random_state=42)

    models = {
        "RandomForest": RandomForestClassifier(n_estimators=150, max_depth=6, random_state=42, class_weight='balanced'),
        "ExtraTrees": ExtraTreesClassifier(n_estimators=150, max_depth=6, random_state=42, class_weight='balanced'),
        "HistGradientBoosting": HistGradientBoostingClassifier(max_iter=100, max_depth=5, random_state=42),
        "DecisionTree": DecisionTreeClassifier(max_depth=4, min_samples_leaf=20, random_state=42, class_weight='balanced')
    }

    print(f"{'Algoritma':<22} | {'ROC-AUC':<10} | {'ML Win Rate (%)':<16} | {'ML Net PnL (USDT)':<18} | {'Profit Factor':<14}")
    print("-" * 90)

    base_pnl = df['pnl_usdt'].sum()
    base_gw = df[df['pnl_usdt'] > 0]['pnl_usdt'].sum()
    base_gl = abs(df[df['pnl_usdt'] < 0]['pnl_usdt'].sum())
    base_pf = base_gw / base_gl if base_gl > 0 else base_gw

    print(f"{'Ham Strateji (Filtresiz)':<22} | {'---':<10} | %{(y.mean()*100):<15.2f} | {base_pnl:<+17.2f} | {base_pf:<13.2f}")

    best_model_name = "RandomForest"
    best_model_obj = None
    best_pnl = -999999.0
    best_y_prob = None

    for m_name, model in models.items():
        y_prob = cross_val_predict(model, X_scaled, y, cv=cv, method='predict_proba')[:, 1]
        auc = roc_auc_score(y, y_prob)

        f_df = df[y_prob >= 0.50]
        f_win_rate = (f_df['target'].sum() / len(f_df)) * 100.0 if len(f_df) > 0 else 0.0
        f_pnl = f_df['pnl_usdt'].sum() if len(f_df) > 0 else 0.0
        f_gw = f_df[f_df['pnl_usdt'] > 0]['pnl_usdt'].sum() if len(f_df) > 0 else 0.0
        f_gl = abs(f_df[f_df['pnl_usdt'] < 0]['pnl_usdt'].sum()) if len(f_df) > 0 else 0.0
        f_pf = f_gw / f_gl if f_gl > 0 else f_gw

        print(f"{m_name:<22} | {auc:<10.4f} | %{f_win_rate:<15.2f} | {f_pnl:<+17.2f} | {f_pf:<13.2f}")

        if f_pnl > best_pnl:
            best_pnl = f_pnl
            best_model_name = m_name
            best_model_obj = model
            best_y_prob = y_prob

    print("-" * 90)
    print(f"🎯 En Yüksek Performans Veren Algoritma Seçildi: '{best_model_name}'\n")

    # Tüm veri kümesi üzerinde eğit
    best_model_obj.fit(X_scaled, y)

    model_file = os.path.join(models_dir, "magmausdt_1m_ml_model.joblib")
    scaler_file = os.path.join(models_dir, "magmausdt_scaler.joblib")
    features_file = os.path.join(models_dir, "magmausdt_feature_names.json")

    joblib.dump(best_model_obj, model_file)
    joblib.dump(scaler, scaler_file)
    with open(features_file, "w") as f:
        json.dump(feature_cols, f, indent=2)

    print(f"💾 Model Dosyaları Başarıyla Kaydedildi:")
    print(f"   • ML Model Dosyası  : {model_file}")
    print(f"   • Standart Ölçekleyici: {scaler_file}")
    print(f"   • Öznitelik İsimleri : {features_file}\n")

    # Karar Ağacı Rust Filtresi Üretimi
    dt = DecisionTreeClassifier(max_depth=4, random_state=42, class_weight='balanced')
    dt.fit(X, y)
    rust_code = generate_rust_decision_tree(dt, feature_cols, symbol)
    rust_file = os.path.join(gen_dir, "magmausdt_1m_filter.rs")
    with open(rust_file, "w") as f:
        f.write(rust_code)
    print(f"⚡ Zero-Latency C/Rust Filtre Kodu Üretildi: {rust_file}\n")

    # Olasılık Eşikleri (Thresholds) Tablosu
    print("------------------------------------------------------------------------------------------")
    print(f"🎯 OLASILIK EŞİKLERİNE (THRESHOLDS) GÖRE FİLTRELENMİŞ PERFORMANS TABLOSU ({best_model_name}):")
    print("------------------------------------------------------------------------------------------")
    thresholds = [0.40, 0.50, 0.55, 0.60]
    print(f"{'Eşik (Threshold)':<18} | {'İşlem Sayısı':<12} | {'Win Rate (%)':<14} | {'Net PnL (USDT)':<16} | {'Profit Factor':<14}")
    print("-" * 85)
    print(f"{'Ham Strateji (1m)':<18} | {len(df):<12} | %{(y.mean()*100):<13.2f} | {base_pnl:<+15.2f} | {base_pf:<13.2f}")

    for th in thresholds:
        f_df = df[best_y_prob >= th]
        if len(f_df) == 0:
            continue
        f_win_rate = (f_df['target'].sum() / len(f_df)) * 100.0
        f_pnl = f_df['pnl_usdt'].sum()
        f_gw = f_df[f_df['pnl_usdt'] > 0]['pnl_usdt'].sum()
        f_gl = abs(f_df[f_df['pnl_usdt'] < 0]['pnl_usdt'].sum())
        f_pf = f_gw / f_gl if f_gl > 0 else f_gw

        print(f"ML Prob >= {th:<9.2f} | {len(f_df):<12} | %{f_win_rate:<13.2f} | {f_pnl:<+15.2f} | {f_pf:<13.2f}")
    print("------------------------------------------------------------------------------------------\n")

    # Canlı Piyasa Tahmini (Inference Testi)
    print("==========================================================================================")
    print(f"⚡ {symbol} 1m CANLI PİYASA İNFERENCE (TAHMİN) TESTİ:")
    print("==========================================================================================")
    
    url_live = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval=1m&limit=120"
    req_live = urllib.request.Request(url_live, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
    with urllib.request.urlopen(req_live, timeout=10) as resp:
        data_live = json.loads(resp.read().decode('utf-8'))

    bars_live = []
    for r in data_live:
        bars_live.append({
            'open': float(r[1]), 'high': float(r[2]), 'low': float(r[3]), 'close': float(r[4]), 'volume': float(r[5])
        })

    closes_live = np.array([b['close'] for b in bars_live])
    opens_live = np.array([b['open'] for b in bars_live])
    highs_live = np.array([b['high'] for b in bars_live])
    lows_live = np.array([b['low'] for b in bars_live])
    vols_live = np.array([b['volume'] for b in bars_live])

    entry_p = closes_live[-1]
    low_100_live = lows_live[-100:].min() if len(lows_live) >= 100 else lows_live.min()
    high_100_live = highs_live[-100:].max() if len(highs_live) >= 100 else highs_live.max()

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
    live_win_prob = best_model_obj.predict_proba(X_live_scaled)[0, 1]

    signal = "TRADE_RECOMMENDED (LONG)" if live_win_prob >= 0.50 else "SKIP_TRADE (PAS GEÇ)"

    print(f"📊 Sembol: {symbol} 1m | Canlı Fiyat: {entry_p} USDT")
    print(f"🎯 Yapay Zeka {symbol} 1m Kazanma Olasılığı: %{live_win_prob*100:.2f}")
    print(f"🚀 Canlı Sinyal Kararı                       : {signal}")
    print("==========================================================================================\n")


if __name__ == "__main__":
    main()
