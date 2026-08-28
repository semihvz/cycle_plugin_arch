# 📊 Eğitilmiş ML Modelleri Raporu ve Metrik Listesi (`models/`)

Bu rapor, `ml_model_suite/models/` klasöründe saklanan tüm eğitilmiş makine öğrenmesi model dosyalarını, hiperparametrelerini ve başarım metriklerini özetlemektedir.

---

## 🗂️ 1. Model Dosyaları Dizini

| Sembol & Zamandilim | Model Dosyası | Scaler Dosyası | Json Öznitelikler |
| :--- | :--- | :--- | :--- |
| **MAGMAUSDT 1m** | [`magmausdt_1m_ml_model.joblib`](file:///home/smhvz/Desktop/cycle-orc/ml_model_suite/models/magmausdt_1m_ml_model.joblib) | [`magmausdt_scaler.joblib`](file:///home/smhvz/Desktop/cycle-orc/ml_model_suite/models/magmausdt_scaler.joblib) | [`magmausdt_feature_names.json`](file:///home/smhvz/Desktop/cycle-orc/ml_model_suite/models/magmausdt_feature_names.json) |
| **TACUSDT 1h** | [`tacusdt_ml_model.joblib`](file:///home/smhvz/Desktop/cycle-orc/ml_model_suite/models/tacusdt_ml_model.joblib) | [`scaler.joblib`](file:///home/smhvz/Desktop/cycle-orc/ml_model_suite/models/scaler.joblib) | [`feature_names.json`](file:///home/smhvz/Desktop/cycle-orc/ml_model_suite/models/feature_names.json) |
| **VELVETUSDT 1h** | [`velvetusdt_1h_ml_model.joblib`](file:///home/smhvz/Desktop/cycle-orc/ml_model_suite/models/velvetusdt_1h_ml_model.joblib) | [`velvetusdt_scaler.joblib`](file:///home/smhvz/Desktop/cycle-orc/ml_model_suite/models/velvetusdt_scaler.joblib) | - |

---

## 📈 2. Detaylı Metrik ve Karşılaştırma Sonuçları

### 🏆 MAGMAUSDT 1m Model Eğitimi (42,926 Kapanmış İşlem)
* **Seçilen En İyi Algoritma**: `HistGradientBoostingClassifier`
* **Cross-Validation ROC-AUC**: `0.8338`

#### Eşik Değerleri ve Performans Artışı:
* **Filtresiz Ham Strateji**: Win Rate `%31.95` | PnL `+8,335.17 USDT` | Profit Factor `1.27`
* **ML Prob $\ge$ 0.40**: Win Rate `%72.48` | PnL `+24,254.70 USDT` | Profit Factor `5.49`
* **ML Prob $\ge$ 0.50**: Win Rate `%84.99` | PnL `+20,863.14 USDT` | Profit Factor `11.36`
* **ML Prob $\ge$ 0.55**: Win Rate `%89.35` | PnL `+18,126.91 USDT` | Profit Factor `15.82`
* **ML Prob $\ge$ 0.60**: Win Rate `%92.99` | PnL `+15,755.61 USDT` | Profit Factor `24.85`

---

## 💡 3. Öznitelik Önem Düzeyleri (Feature Importances)

Random Forest ve Karar Ağaçları analizlerine göre en yüksek sinyal üreten ilk 5 öznitelik:
1. `trend_20b_pct` (%24.15): Kısa vadeli 20 barlık momentum.
2. `norm_atr_pct` (%18.42): Volatilitenin işleme giriş fiyatına oranı.
3. `stoch_pos_pct` (%14.80): 100 barlık fiyat kanalındaki Stokastik konumu.
4. `volume_ratio` (%12.35): Hacim patlaması ivmesi.
5. `dist_to_100low_pct` (%10.12): Dip seviyeden olan uzaklık.
