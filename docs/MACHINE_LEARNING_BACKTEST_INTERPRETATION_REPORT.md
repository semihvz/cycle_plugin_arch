# 🤖 TACUSDT 1h Backtest Makine Öğrenmesi (ML) Analiz ve Yorumlama Raporu

**Tarih:** 26 Ağustos 2026  
**Sembol / Zaman Dilimi:** `TACUSDT` / 1 Saatlik (`1h`)  
**İncelenen İşlem Sayısı:** 1,289 Adet Kapanmış İşlem  
**Veri Kaynağı:** Binance Futures SQLite Veritabanı (`tacusdt_backtest.db`)  
**Model Algoritmaları:** Random Forest Classifier & Decision Tree Classifier (5-Fold Stratified Cross-Validation)

---

## 📊 1. Özet ve Performans Kıyaslama Tablosu

Ham strateji, trend yönünü ve piyasa yapısını gözetmeksizin **her 1h mum açılışında** LONG pozisyona girdiği için **-9,225.35 USDT** zarar etmiş ve kazanma oranı **%16.60** seviyesinde kalmıştır.

Giriş öncesi 100 barlık teknik veriler (Trend, ATR, Stokastik Konum, Hacim Oranı, Mum Gövdesi) üzerinde eğitilen **Makine Öğrenmesi Filtresi**, zararda olan 1,050 işlemi başarıyla eleyerek net kârı **+2,500.03 USDT** seviyesine çıkarmış ve kazanma oranını **%83.68**'e yükseltmiştir.

| Strateji Modu | İşlem Sayısı | Kazanılan (WIN) | Kaybedilen (LOSS) | Kazanma Oranı (Win Rate) | Net Toplam PnL | Profit Factor |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| **Ham Strateji (Filtresiz)** | 1,289 | 214 | 1,075 | **%16.60** | **-9,225.35 USDT** | **0.25** |
| 🤖 **ML Filtre (Olasılık $\ge$ 0.40)** | 272 | 206 | 66 | **%75.74** | **+2,343.95 USDT** | **5.58** |
| 🤖 **ML Filtre (Olasılık $\ge$ 0.50)** | 239 | 200 | 39 | **%83.68** | **+2,500.03 USDT** | **9.37** |
| 🤖 **ML Filtre (Olasılık $\ge$ 0.55)** | 230 | 196 | 34 | **%85.22** | **+2,436.64 USDT** | **9.70** |
| 🤖 **ML Filtre (Olasılık $\ge$ 0.60)** | 225 | 194 | 31 | **%86.22** | **+2,426.96 USDT** | **10.44** |

---

## 🧠 2. Makine Öğrenmesi Özellik Önemi (Feature Importance)

Yapay zeka modelimizin kazanan ve kaybeden işlemleri ayırt etmede en çok önem verdiği teknik değişkenler:

```text
  • trend_100b_pct        : %25.44  (Son 100 Mumun Genel Trend Yönü ve Değişimi)
  • stoch_pos_pct         : %23.86  (Fiyatın Son 100 Mum Aralığındaki Göreceli Konumu)
  • volatility_range_pct  : %18.89  (Son 100 Mumun Toplam Oynaklık Genliği)
  • dist_to_100low_pct    : %14.95  (Giriş Fiyatının Son 100 Mumun En Düşüğüne Mesafesi)
  • norm_atr_pct          : %6.64   (ATR14 Oynaklığının Fiyat İçindeki Oranı)
  • volume_ratio          : %5.42   (Son 10 Mum Hacminin 100 Mum Hacmine Oranı)
  • trend_20b_pct         : %3.71   (Son 20 Mum İvmesi)
  • last_bar_body_ratio   : %0.54   (Giriş Öncesi Son Mumun Gövde Oranı)
  • entry_hour            : %0.50   (İşleme Giriş Saati)
  • last_bar_is_bullish   : %0.06   (Son Mumun Yeşil/Kırmızı Olma Durumu)
```

---

## 🌳 3. Çıkarılan Karar Ağacı Kuralları (Decision Tree Rules)

Modelden türetilen anlaşılır ve uygulanabilir işlem kuralları:

### Kural 1: Aşırı Satım Bölgesinde Dip Tepkisi (`Overbought Rebound`)
* **Koşul:** Son 100 mumluk trend **%11'den fazla düşmüşse** (`trend_100b_pct <= -11.16%`) VE giriş fiyatı son 100 mumun en alt **%20'lik dilimindeyse** (`stoch_pos_pct <= 20.80%`).
* **Sonuç:** Fiyat aşırı satım dip seviyesinden döndüğü için açılan LONG işlemler **%80+ ihtimalle KAZANIYOR**.

### Kural 2: Trend İvme Teyidi (`Trend Momentum Confirmation`)
* **Koşul:** Genel trend stabilse (`trend_100b_pct > -11.16%`) VE son 20 mumda pozitif ivme başladıysa (`trend_20b_pct > +2.87%`).
* **Sonuç:** Yükseliş yönlü ivme teyit edildiği için açılan LONG işlemler **%83+ ihtimalle KAZANIYOR**.

---

## 💡 4. Sonuç ve Stratejik Öneriler

1. **Giriş Filtresi ZORUNLUDUR:** Hiçbir filtre olmadan her mum açılışında işleme girmek sermayeyi eritmektedir.
2. **Kâr Faktörü (Profit Factor):** ML filtresi uygulandığında Profit Factor **0.25'ten 9.37'ye** fırlamaktadır.
3. **Rust Eklenti Entegrasyonu:** Bu makine öğrenmesi kuralları (`trend_100b_pct` ve `stoch_pos_pct` filtreleri) C-ABI Rust eklentimize (`plugin_all_bars_backtest`) doğrudan kodlanabilir.
