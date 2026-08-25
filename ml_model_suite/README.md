# 🤖 Machine Learning Model Suite (`ml_model_suite`)

A dedicated Machine Learning suite for extracting dataset features, training multi-algorithm classifiers, evaluating cross-validation performance, running live CLI inference, and compiling Decision Trees into pure C/Rust code for microsecond dynamic plugin filtering.

---

## 📂 Dizin Yapısı (Directory Sitemap)

```text
ml_model_suite/
├── dataset_exporter.py       # SQLite db'den 100-bar teknik öznitelikleri çıkarır
├── model_trainer.py          # Random Forest, Extra Trees, Decision Tree eğitir ve joblib olarak kaydeder
├── inference_engine.py       # Canlı piyasa mumlarında anlık ML tahmini ve sinyali üretir
├── rust_filter_generator.py  # Eğitilen modeli mikro-saniyelik C/Rust koduna dönüştürür
├── requirements.txt          # Python kütüphane bağımlılıkları (scikit-learn, joblib, pandas, openpyxl)
├── data/                     # Çıkarılan öznitelik veri setleri (dataset.csv, dataset.json)
├── models/                   # Eğitilmiş ve serileştirilmiş model dosyaları (tacusdt_ml_model.joblib, scaler.joblib)
└── generated/                # Oluşturulan C/Rust kodları (ml_filter.rs)
```

---

## 🚀 Kullanım Adımları (Quick Start)

### 1. Veri Setini Çıkarma (`dataset_exporter.py`)
`tacusdt_backtest.db` veritabanından her bir işlemin giriş öncesi 100 barlık geçmişini okur ve 12 adet teknik öznitelik hesaplar:
```bash
python3 ml_model_suite/dataset_exporter.py
```

### 2. Modeli Eğitme ve Değerlendirme (`model_trainer.py`)
Çoklu makine öğrenmesi modellerini 5-Fold Stratified Cross-Validation ile eğitir, ROC-AUC ve Win Rate metriklerini hesaplar ve en başarılı modeli `models/tacusdt_ml_model.joblib` olarak kaydeder:
```bash
python3 ml_model_suite/model_trainer.py
```

### 3. Canlı Piyasa Tahmini / Inference (`inference_engine.py`)
Binance Futures canlı mumlarını çekerek eğitilmiş yapay zeka modelinin tahminini (Kazanma Olasılığı % ve İşlem Sinyali) ekrana basar:
```bash
python3 ml_model_suite/inference_engine.py
```

### 4. C/Rust Filtre Kodu Üretimi (`rust_filter_generator.py`)
Eğitilen karar ağacını sıfır gecikmeli (zero-latency) C-ABI Rust koduna dönüştürür:
```bash
python3 ml_model_suite/rust_filter_generator.py
```

---

## 📊 Performans Özet Tablosu

| Strateji Modu | İşlem Sayısı | Win Rate (%) | Net PnL (USDT) | Profit Factor |
| :--- | :---: | :---: | :---: | :---: |
| **Ham Strateji (Filtresiz)** | 1,289 | **%16.60** | **-9,225.35 USDT** | 0.25 |
| 🤖 **ML Filtre (Olasılık $\ge$ 0.40)** | 272 | **%75.74** | **+2,343.95 USDT** | 5.58 |
| 🤖 **ML Filtre (Olasılık $\ge$ 0.50)** | 239 | **%83.68** | **+2,500.03 USDT** | **9.37** |
| 🤖 **ML Filtre (Olasılık $\ge$ 0.55)** | 230 | **%85.22** | **+2,436.64 USDT** | 9.70 |
| 🤖 **ML Filtre (Olasılık $\ge$ 0.60)** | 225 | **%86.22** | **+2,426.96 USDT** | **10.44** |
