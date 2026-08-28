#!/usr/bin/env python3
import sqlite3
import pandas as pd
import numpy as np
from sklearn.ensemble import RandomForestClassifier
from sklearn.tree import DecisionTreeClassifier, export_text
from sklearn.model_selection import StratifiedKFold, cross_val_predict

def load_data_and_features(db_path="/home/smhvz/Desktop/cycle-orc/all_bars_system/output/all_usdt_futures_1h_backtest.db"):
    conn = sqlite3.connect(db_path)
    lookback_cols = [r[1] for r in conn.execute("PRAGMA table_info(trade_lookback_bars);").fetchall()]
    id_col = "global_trade_id" if "global_trade_id" in lookback_cols else "trade_id"

    trades_df = pd.read_sql_query(f"SELECT * FROM closed_trades ORDER BY {id_col} ASC;", conn)
    lookback_df = pd.read_sql_query(f"SELECT * FROM trade_lookback_bars ORDER BY {id_col} ASC, bar_offset ASC;", conn)
    conn.close()

    features_list = []

    for trade_id, trade in trades_df.iterrows():
        tid = trade[id_col]
        t_bars = lookback_df[lookback_df[id_col] == tid].sort_values('bar_offset')
        
        if len(t_bars) < 100:
            continue

        closes = t_bars['close'].values
        opens = t_bars['open'].values
        highs = t_bars['high'].values
        lows = t_bars['low'].values
        volumes = t_bars['volume'].values

        entry_price = trade['entry_price']
        lowest_100 = trade['lowest_100_price']
        highest_100 = highs.max()

        trend_100b_pct = ((closes[-1] - closes[0]) / closes[0]) * 100.0
        trend_20b_pct = ((closes[-1] - closes[-20]) / closes[-20]) * 100.0
        stoch_pos_pct = ((entry_price - lowest_100) / max(highest_100 - lowest_100, 1e-8)) * 100.0
        norm_atr_pct = (trade['atr_14'] / entry_price) * 100.0
        volatility_range_pct = ((highest_100 - lowest_100) / entry_price) * 100.0
        vol_10_mean = volumes[-10:].mean()
        vol_100_mean = volumes.mean()
        volume_ratio = vol_10_mean / max(vol_100_mean, 1e-8)
        
        entry_hour = int(trade['entry_time_utc'].split()[1].split(':')[0])
        dist_to_100low_pct = ((entry_price - lowest_100) / entry_price) * 100.0
        
        last_body = abs(closes[-1] - opens[-1])
        last_range = max(highs[-1] - lows[-1], 1e-8)
        last_bar_body_ratio = last_body / last_range
        last_bar_is_bullish = 1 if closes[-1] > opens[-1] else 0

        target = 1 if trade['result'] == 'WIN' else 0

        features_list.append({
            'trade_id': tid,
            'entry_time_utc': trade['entry_time_utc'],
            'entry_price': entry_price,
            'risk_usdt': trade['risk_usdt'],
            'target_reward_usdt': trade['target_reward_usdt'],
            'pnl_usdt': trade['pnl_usdt'],
            'target': target,
            'trend_100b_pct': trend_100b_pct,
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

    return pd.DataFrame(features_list)

def main():
    print("==========================================================================================")
    print("🤖 MAKİNE ÖĞRENMESİ İLE TACUSDT BACKTEST YORUMLAMA VE FİLTRELEME MODELİ")
    print("==========================================================================================")

    df = load_data_and_features()
    print(f"Yüklenen İşlem Sayısı: {len(df)}")
    print(f"Ham Strateji - Win Rate: {(df['target'].mean() * 100):.2f}% ({df['target'].sum()} WIN / {(len(df) - df['target'].sum())} LOSS)")
    print(f"Ham Strateji - Toplam Net PnL: {df['pnl_usdt'].sum():.2f} USDT\n")

    feature_cols = [
        'trend_100b_pct', 'trend_20b_pct', 'stoch_pos_pct',
        'norm_atr_pct', 'volatility_range_pct', 'volume_ratio',
        'entry_hour', 'dist_to_100low_pct', 'last_bar_body_ratio',
        'last_bar_is_bullish'
    ]

    X = df[feature_cols]
    y = df['target']

    # Train Random Forest Classifier
    rf = RandomForestClassifier(n_estimators=100, max_depth=5, random_state=42, class_weight='balanced')
    
    # Stratified K-Fold Cross Validation predictions to avoid overfitting
    cv = StratifiedKFold(n_splits=5, shuffle=True, random_state=42)
    y_prob = cross_val_predict(rf, X, y, cv=cv, method='predict_proba')[:, 1]
    
    df['ml_win_prob'] = y_prob

    # Feature Importance Analysis
    rf.fit(X, y)
    importances = pd.DataFrame({
        'Feature': feature_cols,
        'Importance': rf.feature_importances_
    }).sort_values('Importance', ascending=False)

    print("------------------------------------------------------------------------------------------")
    print("🧠 MAKİNE ÖĞRENMESİ ÖZELLİK ÖNEM DERECELERİ (FEATURE IMPORTANCE):")
    print("------------------------------------------------------------------------------------------")
    for idx, row in importances.iterrows():
        print(f"  • {row['Feature']:<22}: %{row['Importance']*100:.2f}")
    print("------------------------------------------------------------------------------------------\n")

    # Decision Tree Rules for Interpretation
    dt = DecisionTreeClassifier(max_depth=3, min_samples_leaf=20, random_state=42, class_weight='balanced')
    dt.fit(X, y)
    rules_text = export_text(dt, feature_names=feature_cols)

    print("------------------------------------------------------------------------------------------")
    print("🌳 KARAR AĞACI KURALLARI (DECISION TREE RULES - Ne Zaman İşleme Girilmeli?):")
    print("------------------------------------------------------------------------------------------")
    print(rules_text)
    print("------------------------------------------------------------------------------------------\n")

    # Apply ML Threshold Filter
    print("------------------------------------------------------------------------------------------")
    print("🎯 MAKİNE ÖĞRENMESİ FİLTRESİ SONRASI STRATEJİ PERFORMANSI (Kıyaslama Tablosu):")
    print("------------------------------------------------------------------------------------------")
    
    thresholds = [0.40, 0.50, 0.55, 0.60]
    
    print(f"{'Eşik (Threshold)':<18} | {'İşlem Sayısı':<12} | {'Win Rate (%)':<14} | {'Net PnL (USDT)':<16} | {'Profit Factor':<14}")
    print("-" * 85)
    
    baseline_win_rate = (y.mean()) * 100
    baseline_pnl = df['pnl_usdt'].sum()
    gross_wins = df[df['pnl_usdt'] > 0]['pnl_usdt'].sum()
    gross_losses = abs(df[df['pnl_usdt'] < 0]['pnl_usdt'].sum())
    baseline_pf = gross_wins / gross_losses if gross_losses > 0 else gross_wins
    
    print(f"{'Ham Strateji (Yok)':<18} | {len(df):<12} | %{baseline_win_rate:<13.2f} | {baseline_pnl:<+15.2f} | {baseline_pf:<13.2f}")

    for th in thresholds:
        filtered_df = df[df['ml_win_prob'] >= th]
        if len(filtered_df) == 0:
            continue
        
        f_wins = filtered_df['target'].sum()
        f_total = len(filtered_df)
        f_win_rate = (f_wins / f_total) * 100.0
        f_pnl = filtered_df['pnl_usdt'].sum()
        f_gw = filtered_df[filtered_df['pnl_usdt'] > 0]['pnl_usdt'].sum()
        f_gl = abs(filtered_df[filtered_df['pnl_usdt'] < 0]['pnl_usdt'].sum())
        f_pf = f_gw / f_gl if f_gl > 0 else f_gw

        print(f"ML Prob >= {th:<9.2f} | {f_total:<12} | %{f_win_rate:<13.2f} | {f_pnl:<+15.2f} | {f_pf:<13.2f}")
    
    print("==========================================================================================\n")

if __name__ == "__main__":
    main()
