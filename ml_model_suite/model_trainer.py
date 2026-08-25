#!/usr/bin/env python3
import os
import joblib
import pandas as pd
import numpy as np
from sklearn.ensemble import RandomForestClassifier, ExtraTreesClassifier, HistGradientBoostingClassifier
from sklearn.tree import DecisionTreeClassifier, export_text
from sklearn.preprocessing import StandardScaler
from sklearn.metrics import classification_report, roc_auc_score, confusion_matrix
from sklearn.model_selection import StratifiedKFold, cross_val_predict

def train_and_evaluate(data_path="/home/smhvz/Desktop/cycle-orc/ml_model_suite/data/dataset.csv", output_dir="/home/smhvz/Desktop/cycle-orc/ml_model_suite/models"):
    os.makedirs(output_dir, exist_ok=True)
    
    if not os.path.exists(data_path):
        from dataset_exporter import export_dataset
        df = export_dataset()
    else:
        df = pd.read_csv(data_path)

    feature_cols = [
        'trend_100b_pct', 'trend_50b_pct', 'trend_20b_pct', 'stoch_pos_pct',
        'norm_atr_pct', 'volatility_range_pct', 'volume_ratio',
        'entry_hour', 'dist_to_100low_pct', 'last_bar_body_ratio',
        'last_bar_is_bullish'
    ]

    X = df[feature_cols]
    y = df['target']

    print("==========================================================================================")
    print("🤖 MAKİNE ÖĞRENMESİ MODEL EĞİTİMİ VE ÇOKLU ALGORİTMA PERFORMANS DĞERLENDİRMESİ")
    print("==========================================================================================")
    print(f"Toplam Veri Sayısı: {len(X)} | Pozitif Etiket (WIN): {y.sum()} | Negatif Etiket (LOSS): {len(y) - y.sum()}")

    scaler = StandardScaler()
    X_scaled = scaler.fit_transform(X)

    models = {
        "RandomForest": RandomForestClassifier(n_estimators=150, max_depth=6, random_state=42, class_weight='balanced'),
        "ExtraTrees": ExtraTreesClassifier(n_estimators=150, max_depth=6, random_state=42, class_weight='balanced'),
        "HistGradientBoosting": HistGradientBoostingClassifier(max_iter=100, max_depth=5, random_state=42),
        "DecisionTree": DecisionTreeClassifier(max_depth=4, min_samples_leaf=20, random_state=42, class_weight='balanced')
    }

    results = []

    print("\n------------------------------------------------------------------------------------------")
    print("📊 ALGORİTMA KARŞILAŞTIRMA VE ÇAPRAZ DOĞRULAMA (5-FOLD CROSS-VALIDATION) METRİKLERİ:")
    print("------------------------------------------------------------------------------------------")

    cv = StratifiedKFold(n_splits=5, shuffle=True, random_state=42)

    best_model_name = "RandomForest"
    best_model_obj = None
    best_pnl = -999999.0

    for name, model in models.items():
        y_prob = cross_val_predict(model, X_scaled, y, cv=cv, method='predict_proba')[:, 1]
        auc = roc_auc_score(y, y_prob)

        # Evaluate at Probability Threshold >= 0.50
        y_pred = (y_prob >= 0.50).astype(int)
        
        filtered_indices = np.where(y_prob >= 0.50)[0]
        if len(filtered_indices) > 0:
            filtered_df = df.iloc[filtered_indices]
            f_win_rate = (filtered_df['target'].sum() / len(filtered_df)) * 100.0
            f_pnl = filtered_df['pnl_usdt'].sum()
            f_gw = filtered_df[filtered_df['pnl_usdt'] > 0]['pnl_usdt'].sum()
            f_gl = abs(filtered_df[filtered_df['pnl_usdt'] < 0]['pnl_usdt'].sum())
            f_pf = f_gw / f_gl if f_gl > 0 else f_gw
        else:
            f_win_rate, f_pnl, f_pf = 0.0, 0.0, 0.0

        print(f"  • {name:<20} | ROC-AUC: {auc:.4f} | ML Filtreli Win Rate: %{f_win_rate:<6.2f} | PnL: {f_pnl:<+9.2f} USDT | Profit Factor: {f_pf:.2f}")

        if f_pnl > best_pnl:
            best_pnl = f_pnl
            best_model_name = name
            best_model_obj = model

    print("------------------------------------------------------------------------------------------\n")

    # Fit final best model on full dataset
    print(f"🎯 En Yüksek Performans Veren Algoritma Seçildi: '{best_model_name}'")
    best_model_obj.fit(X_scaled, y)

    # Export Model Files
    model_file = os.path.join(output_dir, "tacusdt_ml_model.joblib")
    scaler_file = os.path.join(output_dir, "scaler.joblib")
    features_file = os.path.join(output_dir, "feature_names.json")

    joblib.dump(best_model_obj, model_file)
    joblib.dump(scaler, scaler_file)
    import json
    with open(features_file, "w") as f:
        json.dump(feature_cols, f, indent=2)

    print(f"💾 Model Dosyaları Başarıyla Kaydedildi:")
    print(f"   • Model Kütüphanesi: {model_file}")
    print(f"   • Standart Ölçekleyici: {scaler_file}")
    print(f"   • Öznitelik İsimleri : {features_file}\n")

    # Feature Importance of Best Model
    if hasattr(best_model_obj, 'feature_importances_'):
        importances = pd.DataFrame({
            'Feature': feature_cols,
            'Importance': best_model_obj.feature_importances_
        }).sort_values('Importance', ascending=False)

        print("------------------------------------------------------------------------------------------")
        print("🧠 SEÇİLEN MODEL ÖZELLİK ÖNEM DERECELERİ (FEATURE IMPORTANCE):")
        print("------------------------------------------------------------------------------------------")
        for idx, row in importances.iterrows():
            print(f"  • {row['Feature']:<22}: %{row['Importance']*100:.2f}")
        print("------------------------------------------------------------------------------------------\n")

    return best_model_obj, scaler, feature_cols

if __name__ == "__main__":
    train_and_evaluate()
