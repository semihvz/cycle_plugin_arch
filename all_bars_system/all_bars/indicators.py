"""
Teknik İndikatör Hesaplama Modülü (ATR, TR)
"""


def calculate_atr(bars, period=14):
    """True Range (TR) ve Wilder's Smoothing ATR serisini hesaplar."""
    tr_list = []
    for i in range(len(bars)):
        if i == 0:
            tr = bars[i]["high"] - bars[i]["low"]
        else:
            hl = bars[i]["high"] - bars[i]["low"]
            hp = abs(bars[i]["high"] - bars[i - 1]["close"])
            lp = abs(bars[i]["low"] - bars[i - 1]["close"])
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
