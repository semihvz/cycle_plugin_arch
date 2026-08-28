# 🚀 Makine Öğrenmesi Modelleri Geliştirme Yol Haritası (ML Improvement Roadmap)

Bu belge, `cycle-orc` projesindeki yapay zeka ve makine öğrenmesi altyapısının tahmin gücünü (**Alpha**), kazanma oranını (**Win Rate**) ve karlılığını (**Profit Factor**) artırmak için hazırlanmış 6 ana geliştirme aksiyonunu detaylandırmaktadır.

---

## 🎯 1. Çoklu Zaman Dilimi Öznitelikleri (Multi-Timeframe Features)

1-dakikalık (`1m`) grafikte mikro dalgalanmaların yanıltıcı sinyallerini önlemek için üst zaman dilimlerindeki ana trend ve oynaklık öznitelik olarak eklenmelidir:

* **`trend_15m_pct`**: 15 dakikalık grafikteki son 20 barlık kapanış trendi.
* **`trend_1h_pct`**: 1 saatlik grafikteki ana piyasa trendi.
* **`vwap_dist_pct`**: Fiyatın Hacim Ağırlıklı Ortalama Fiyata (VWAP) olan yüzdesel uzaklığı.
* **`bb_percent_b`**: 15 dakikalık Bollinger Bantları $\%B$ göstergesi.

---

## 🏛️ 2. Piyasa Mikro Yapısı Verileri (Market Microstructure & Orderbook)

Sadece OHLCV fiyat mumlarının ötesine geçerek borsadan WebSocket ile anlık derinlik ve işlem akışı verileri eklenmelidir:

* **`orderbook_imbalance`**: Alış ve Satış tahtasındaki ilk 20 kademedeki toplam hacim dengesizliği:
  $$\text{Imbalance} = \frac{\text{Bid Volume} - \text{Ask Volume}}{\text{Bid Volume} + \text{Ask Volume}}$$
* **`cvd_1m` (Cumulative Volume Delta)**: Market Alıcılar ile Market Satıcılar arasındaki net aktif hacim farkı.
* **`vpin` (Volume-Synchronized Probability of Toxicity)**: Kurumsal / balina işlemlerinin tespiti.

---

## 🎯 3. Triple Barrier Method ile Akıllı Etiketleme (Marcos López de Prado)

Klasik ucu açık TP/SL beklemek yerine 3 katmanlı sınır mekanizması uygulanmalıdır:

1. **Üst Sınır (Take Profit Barrier)**: Hedef kar seviyesi.
2. **Alt Sınır (Stop Loss Barrier)**: Maksimum kabul edilebilir zarar seviyesi.
3. **Dikey Sınır (Vertical Barrier / Time-out)**: Örneğin 60 dakika içinde iki sınıra da ulaşamayan işlemler otomatik kapatılıp etiketi ($1$/$0$) süresine göre basılır.
4. **İz Süren Stop (Trailing Stop Loss)**: Kar oranı arttıkça stop seviyesini yukarı kaydırma.

---

## 🚀 4. Gelişmiş ML Mimarileri (XGBoost, CatBoost & LSTM/TFT)

Mevcut `HistGradientBoosting` modeline ek olarak aşağıdaki gelişmiş modeller turnuvaya dahil edilmelidir:

* **CatBoost & XGBoost**: GPU ivmelendirmeli ve kategorik öznitelikleri daha iyi işleyen gradient boosting mimarileri.
* **LSTM (Long Short-Term Memory)** / **Temporal Fusion Transformer (TFT)**: Mum dizilimlerinin zamansal dizilimlerini (sequence) doğrudan öğrenen derin öğrenme modelleri.

---

## ⚙️ 5. Optuna ile Otomatik Hiperparametre Optimizasyonu

Model hiperparametreleri manuel değil, Optuna Bayesian optimizasyon kütüphanesi ile 1,000 denemede otomatik aranmalıdır:

```python
import optuna

def objective(trial):
    params = {
        'learning_rate': trial.suggest_float('learning_rate', 0.01, 0.2),
        'max_depth': trial.suggest_int('max_depth', 3, 10),
        'l2_regularization': trial.suggest_float('l2_regularization', 1e-8, 10.0, log=True),
        'min_samples_leaf': trial.suggest_int('min_samples_leaf', 10, 100)
    }
    # Evaluate 5-Fold Stratified CV
    return profit_factor
```

---

## 💰 6. Dinamik Pozisyon Büyüklüğü (Kelly Kriteri)

Sabit $50 USDT yerine modelin tahmin olasılığına ($P$) göre pozisyon büyüklüğü dinamik ayarlanmalıdır:

$$\text{Position Size} = \begin{cases} 
\$25 \text{ USDT} & \text{if } 0.50 \le P < 0.55 \\
\$50 \text{ USDT} & \text{if } 0.55 \le P < 0.65 \\
\$100 \text{ USDT} & \text{if } 0.65 \le P < 0.80 \\
\$200 \text{ USDT} & \text{if } P \ge 0.80 
\end{cases}$$
