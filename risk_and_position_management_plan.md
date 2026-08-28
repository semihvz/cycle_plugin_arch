# 🛡️ MSMP 2.0 - POZİSYON & RİSK YÖNETİMİ VE AYLIK BEKLENEN KÂR PROJEKSİYONU

Bu doküman, **MSMP 2.0 (MS Analyzer)** ticaret sistemi için tasarlanan profesyonel pozisyon boyutlandırma, risk yönetimi ve beklenen aylık kârlılık matematiğini içermektedir.

---

## 🧮 1. Beklenen Değer (Expected Value - EV) Matematiği

Backtest verilerinde **Sıkı ATS Filtresi ($\ge 1.5$)** ve **Seçkin 30-40 Trend Paritesi** kullanıldığında sistem metrikleri:

- **Kazanan İşlem Getirisi (R)**: $2.01\text{ R}$ (Ortalama $+4.82\%$)
- **Kaybeden İşlem Kaybı (R)**: $-1.00\text{ R}$ (Ortalama $-2.40\%$)
- **Ortalama Win Rate**: **%42**

### Aylık 50 İşlemlik Bir Periyotta R Skoru:

$$\text{Kazanç} = 21 \text{ Kazanan İşlem} \times 2.01\text{ R} = \mathbf{+42.21\text{ R}}$$

$$\text{Kayıp} = 29 \text{ Kaybeden İşlem} \times 1.00\text{ R} = \mathbf{-29.00\text{ R}}$$

$$\text{Net Aylık Getiri} = +42.21\text{ R} - 29.00\text{ R} = \mathbf{+13.21\text{ R}}$$

Sistem ayda **net $+13.21\text{ R}$** değer üretmektedir.

---

## 💰 2. Risk Profillerine Göre Aylık Kâr Projeksiyonu ($10.000 USDT Sermaye)

| Risk Profili | İşlem Başı Risk (%) | İşlem Başı Risk ($) | Net Aylık R Getirisi | **Beklenen Aylık Kâr (%)** | **Beklenen Aylık Kâr ($)** |
| :--- | :-: | :-: | :-: | :-: | :-: |
| 🛡️ **Muhafazakar Risk** | **%0.5** | $50 USDT | $+13.21\text{ R}$ | **+%6.60 / Ay** | **+$660 USDT / Ay** |
| ⚖️ **Dengeli Risk (Önerilen)** | **%1.0** | $100 USDT | $+13.21\text{ R}$ | **+%13.21 / Ay** | **+$1.321 USDT / Ay** |
| 🚀 **Agresif Risk** | **%2.0** | $200 USDT | $+13.21\text{ R}$ | **+%26.42 / Ay** | **+$2.642 USDT / Ay** |

---

## 📐 3. Dinamik Pozisyon Boyutlandırma Formülü (Fixed Fractional Sizing)

Her işlem açılmadan önce pozisyon büyüklüğü Stop Loss mesafesine göre otomatik hesaplanır:

$$\text{Pozisyon Büyüklüğü (\$)} = \frac{\text{Toplam Sermaye} \times \text{İşlem Başı Risk \%}}{\text{Stop Loss Yüzdesi (\%)}}$$

### Örnek Hesaplama:
- **Toplam Sermaye**: $\$10.000$
- **İşlem Başı Risk**: $\%1.0$ ($\$100$)
- **15m MS Analyzer SL Mesafesi**: $\%2.5$

$$\text{Pozisyon Büyüklüğü} = \frac{\$10.000 \times 0.01}{0.025} = \mathbf{\$4.000\text{ USDT}}$$

---

## 🛡️ 4. Risk ve Sermaye Koruma Kuralları

1. **Maksimum Günlük Kayıp Limiti**: Günlük kayıp toplam sermayenin %3.0'ına ulaşırsa o gün yeni işlem açılmaz.
2. **Maksimum Eşzamanlı Açık İşlem**: Aynı anda en fazla 3 aktif pozisyon taşınabilir.
3. **Maksimum Drawdown Kontrolü**: Zirve bakiyeden %10 düşüş yaşanırsa işlem başı risk oranı geçici olarak %0.5'e çekilir.
