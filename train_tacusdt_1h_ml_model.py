#!/usr/bin/env python3
import sqlite3
import os
import json
import numpy as np
import pandas as pd
import joblib
from sklearn.ensemble import HistGradientBoostingClassifier, RandomForestClassifier, ExtraTreesClassifier
from sklearn.tree import DecisionTreeClassifier, export_text
from sklearn.preprocessing import StandardScaler
from sklearn.model_selection import StratifiedKFold, cross_val_predict
from sklearn.metrics import roc_auc_score, classification_report

def extract_features_from_db(db_path):
    conn = sqlite3.connect(db_path)
    trades_df = pd.read_sql_query("SELECT * FROM closed_trades", conn)
    lookback_df = pd.read_sql_query("SELECT * FROM trade_lookback_bars", conn)
    conn.close()

    print(f"Loaded {len(trades_df)} closed trades and {len(lookback_df)} lookback bars from {db_path}")

    features_list = []
    
    grouped = lookback_df.groupby('trade_id')

    for _, trade in trades_df.iterrows():
        t_id = trade['trade_id']
        if t_id not in grouped.groups:
            continue
            
        t_bars = grouped.get_group(t_id).sort_values('bar_offset')
        if len(t_bars) < 100:
            continue

        closes_arr = t_bars['close'].values
        opens_arr = t_bars['open'].values
        highs_arr = t_bars['high'].values
        lows_arr = t_bars['low'].values
        vols_arr = t_bars['volume'].values

        entry_price = trade['entry_price']
        lowest_100 = trade['lowest_100_price']
        highest_100 = highs_arr.max()
        atr_val = trade['atr_14']

        trend_100b_pct = ((closes_arr[-1] - closes_arr[0]) / closes_arr[0]) * 100.0
        trend_50b_pct = ((closes_arr[-1] - closes_arr[-50]) / closes_arr[-50]) * 100.0
        trend_20b_pct = ((closes_arr[-1] - closes_arr[-20]) / closes_arr[-20]) * 100.0
        stoch_pos_pct = ((entry_price - lowest_100) / max(highest_100 - lowest_100, 1e-8)) * 100.0
        norm_atr_pct = (atr_val / entry_price) * 100.0
        volatility_range_pct = ((highest_100 - lowest_100) / entry_price) * 100.0
        vol_10_mean = vols_arr[-10:].mean()
        vol_100_mean = vols_arr.mean()
        volume_ratio = vol_10_mean / max(vol_100_mean, 1e-8)
        
        entry_utc = trade['entry_time_utc']
        entry_hour = int(entry_utc.split()[1].split(':')[0]) if ' ' in entry_utc else 0
        dist_to_100low_pct = ((entry_price - lowest_100) / entry_price) * 100.0
        last_body = abs(closes_arr[-1] - opens_arr[-1])
        last_range = max(highs_arr[-1] - lows_arr[-1], 1e-8)
        last_bar_body_ratio = last_body / last_range
        last_bar_is_bullish = 1 if closes_arr[-1] > opens_arr[-1] else 0

        target = 1 if trade['result'] == 'WIN' else 0

        features_list.append({
            'trade_id': t_id,
            'target': target,
            'pnl_usdt': trade['pnl_usdt'],
            'risk_usdt': trade['risk_usdt'],
            'reward_usdt': trade['target_reward_usdt'],
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

    return pd.DataFrame(features_list)

def generate_rust_decision_tree(tree, feature_names):
    tree_ = tree.tree_
    feature_name = [
        feature_names[i] if i != -2 else "undefined!"
        for i in tree_.feature
    ]
    
    lines = []
    lines.append("// Auto-generated Zero-Latency C/Rust ML Filter for TACUSDT 1h Collector Data")
    lines.append("#[derive(Debug, Clone, Copy)]")
    lines.append("pub struct TACUSDT1hMLFeatures {")
    for fname in feature_names:
        lines.append(f"    pub {fname}: f64,")
    lines.append("}")
    lines.append("")
    lines.append("pub fn evaluate_tacusdt_1h_filter(f: &TACUSDT1hMLFeatures) -> (bool, f64) {")
    
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
    db_path = "/home/smhvz/Desktop/cycle-orc/data/tacusdt_1h_collector.db"
    print("==========================================================================================")
    print("🧠 TACUSDT 1h VERİTABANI İLE MAKİNE ÖĞRENMESİ MODELİ EĞİTİM VE ANALİZİ")
    print("==========================================================================================")

    df = extract_features_from_db(db_path)
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

    # Multi-Model Evaluation
    models = {
        'HistGradientBoosting': HistGradientBoostingClassifier(max_iter=100, max_depth=5, random_state=42),
        'RandomForest': RandomForestClassifier(n_estimators=100, max_depth=5, random_state=42, class_weight='balanced'),
        'ExtraTrees': ExtraTreesClassifier(n_estimators=100, max_depth=5, random_state=42),
        'DecisionTree': DecisionTreeClassifier(max_depth=4, random_state=42, class_weight='balanced')
    }

    print("\n📊 Algoritma Performans Karşılaştırması (5-Fold Cross Validation):")
    print(f"{'Algoritma':<22} | {'ROC-AUC':<10} | {'ML Win Rate (%)':<16} | {'ML Net PnL (USDT)':<18} | {'Profit Factor':<14}")
    print("-" * 90)

    base_pnl = df['pnl_usdt'].sum()
    base_gw = df[df['pnl_usdt'] > 0]['pnl_usdt'].sum()
    base_gl = abs(df[df['pnl_usdt'] < 0]['pnl_usdt'].sum())
    base_pf = base_gw / base_gl if base_gl > 0 else base_gw

    print(f"{'Ham Strateji (Filtresiz)':<22} | {'---':<10} | %{(y.mean()*100):<15.2f} | {base_pnl:<+17.2f} | {base_pf:<13.2f}")

    best_model_name = None
    best_auc = 0.0
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

        if auc > best_auc:
            best_auc = auc
            best_model_name = m_name
            best_y_prob = y_prob

    print("-" * 90)
    print(f"🎯 En Başarılı Model: {best_model_name} (ROC-AUC: {best_auc:.4f})\n")

    # Fit best model on all data and save
    best_model = models[best_model_name]
    best_model.fit(X_scaled, y)

    models_dir = "/home/smhvz/Desktop/cycle-orc/ml_model_suite/models"
    os.makedirs(models_dir, exist_ok=True)

    joblib.dump(best_model, os.path.join(models_dir, "tacusdt_1h_collector_ml_model.joblib"))
    joblib.dump(scaler, os.path.join(models_dir, "scaler_1h_collector.joblib"))
    print("✅ Model Başarıyla Kaydedildi: 'tacusdt_1h_collector_ml_model.joblib'")

    # Generate C/Rust code using DecisionTree
    dt = DecisionTreeClassifier(max_depth=4, random_state=42, class_weight='balanced')
    dt.fit(X, y)
    rust_code = generate_rust_decision_tree(dt, feature_cols)
    gen_dir = "/home/smhvz/Desktop/cycle-orc/ml_model_suite/generated"
    os.makedirs(gen_dir, exist_ok=True)
    with open(os.path.join(gen_dir, "tacusdt_1h_filter.rs"), "w") as f:
        f.write(rust_code)
    print("✅ Gömülü C/Rust Filtre Kodu Üretildi: 'ml_model_suite/generated/tacusdt_1h_filter.rs'")

    # Feature Importance for RandomForest
    rf_explainer = RandomForestClassifier(n_estimators=100, max_depth=5, random_state=42)
    rf_explainer.fit(X_scaled, y)
    importances = rf_explainer.feature_importances_
    sorted_idx = np.argsort(importances)[::-1]

    print("\n💡 Öznitelik Önem Düzeyleri (Feature Importances):")
    for idx in sorted_idx:
        print(f"  • {feature_cols[idx]:<22}: {importances[idx]:.4f}")

    print("==========================================================================================")

if __name__ == "__main__":
    main()
