# 📊 Portföy Olasılık Hesabı, Beklenen Değer ve Varış Zamanı Analizi

Bu belge, `cycle-orc` projesindeki yapay zeka tarafından taranan çoklu pozisyon portföylerinde **bireysel ve toplu TP/SL olasılıklarını**, matematiksel beklenen değeri ($E(X)$), $1:2$ Risk/Ödül yapısının koruyucu gücünü ve ampirik **holding time (pozisyonda kalma süresi)** istatistiklerini belgelemektedir.

---

## 🧮 1. Çoklu Pozisyon Ortak Olasılık Hesabı (Joint Probability)

Modelimizin tekil kazanma oranı $P(WIN) = 0.85 \dots 0.90$ bandındadır. Bağımsız $N=6$ pozisyonluk bir portföyün **tamamının ($N$ adedinin) aynı anda TP ile kapanma olasılığı**:

$$P(\text{Tüm } N \text{ Pozisyon TP}) = P(WIN)^N$$

* **%85 Tekil Başarı İle**: $0.85^6 \approx \mathbf{\%37.7}$
* **%90 Tekil Başarı İle**: $0.90^6 \approx \mathbf{\%53.1}$

> 💡 **Sonuç**: 6 pozisyonun 6'sının da TP olma olasılığı **%38 - %53** aralığındadır.

---

## 📈 2. İstatistiksel Beklenen Değer ($E(X)$) ve Portföy Senaryoları

Portföyün karla kapanması için 6 pozisyonun tamamının TP olmasına gerek yoktur. $1:2$ Risk/Ödül oranı ($50 USDT pozisyonda TP: **+$5.80**, SL: **-$2.90**) sayesinde portföy koruma altındadır:

$$\text{Beklenen Net Kar } E(\text{PnL}) = N \times \left( P(WIN) \times \text{TP} - (1 - P(WIN)) \times \text{SL} \right)$$

### Olası Senaryolar Tablosu ($300 USDT Toplam Portföy):

| Senaryo | Dağılım | Toplam Kazanç | Toplam Kayıp | Net Portföy PnL | Net Getiri (%) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Senaryo 1 (Tam Başarı)** | 6 TP / 0 SL | +$34.80 USDT | $0.00 USDT | **+$34.80 USDT** | **+%11.60** |
| **Senaryo 2 (En Olası)** | 5 TP / 1 SL | +$29.00 USDT | -$2.90 USDT | **+$26.10 USDT** | **+%8.70** |
| **Senaryo 3 (Ortalama)** | 4 TP / 2 SL | +$23.20 USDT | -$5.80 USDT | **+$17.40 USDT** | **+%5.80** |
| **Senaryo 4 (Kötü Gün)** | 3 TP / 3 SL | +$17.40 USDT | -$8.70 USDT | **+$8.70 USDT** | **+%2.90** |
| **Senaryo 5 (Başarısız)** | 2 TP / 4 SL | +$11.60 USDT | -$11.60 USDT | **$0.00 USDT** | **%0.00** |

> 🛡️ **Kritik Bulgusal**: 6 pozisyondan **4 tanesi SL olsa dahi (sadece 2 TP)** portföy başa baş noktadadır ($0.00 USDT PnL$). 3 TP / 3 SL senaryosunda dahi portföy net karlı kapanmaktadır!

---

## ⏱️ 3. Ampirik Pozisyonda Kalma Süresi (Holding Time Statistics)

42.926 adet 1-dakikalık (`1m`) closed trades veritabanı analizine göre:

* **Medyan Süre (50. Yüzdelik)**: **116 Bar (116 Dakika / 1.9 Saat)**
* **Hızlı Kapanışlar (25. Yüzdelik)**: **44 Bar (44 Dakika)**
* **Uzun Sürenler (75. Yüzdelik)**: **295 Bar (4.9 Saat)**

### Anlık Pozisyonların Tahmini Kapanış Zaman Çizelgesi:
1. **İç Dalga (0 - 45 Dakika)**: VTHOUSDT (LONG), 1000SHIBUSDT (SHORT)
2. **Orta Dalga (45 - 90 Dakika)**: LINKUSDT (SHORT), MOCAUSDT (SHORT)
3. **Son Dalga (90 - 180 Dakika / 1.5 - 3 Saat)**: FILUSDT (LONG), FLOCKUSDT (LONG)
