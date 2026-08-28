# TACUSDT 1h ATR-Based Backtest Executive Report

## 1. Executive Summary & Strategy Overview
- **Trading Pair**: `{SYMBOL}`
- **Timeframe**: `{INTERVAL}` (1-Hour Klines)
- **Backtest Range**: `{start_date}` to `{end_date}` ({DAYS_BACK} days, {total_candles} bars)
- **Initial Capital**: `${INITIAL_CAPITAL:,.2f}`
- **Fee Model**: `{FEE_RATE*100:.2f}%` Taker fee per order (0.10% round-trip)

### Strategy Rules Applied
1. **Reference State ($C_{{ref}}$)**: Takes Close price ($P_{{ref}}$) and calculates 14-period ATR.
2. **Stop Loss**: Set at $2 \times \text{{ATR}}$ distance ($P_{{ref}} \mp 2 \times \text{{ATR}}$).
3. **Direction Trigger ("hangisi kara geçerse onu açacak")**: Monitors subsequent candles; whichever direction enters profit first (Long above $P_{{ref}}$ vs Short below $P_{{ref}}$) triggers the entry.
4. **Continuous Reset ("işlemin kapandığı yerden tekrar aynı süreci takip etceksin")**: When a trade exits at candle $C_{{exit}}$, $C_{{exit}}$ becomes the new $C_{{ref}}$, resetting reference price & ATR immediately.

---

## 2. Multi-Profile Strategy Performance Comparison

| Strategy Profile | Total Trades | Win Rate % | Net PnL % | Final Equity ($) | Profit Factor | Max DD % | Avg Hold (hrs) |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| **1:1 RR (TP 2 ATR, SL 2 ATR)** | 24 | 75.00% | **+251.40%** | $3,513.96 | 2.91 | 21.47% | 59.4h |
| **1:1.5 RR (TP 3 ATR, SL 2 ATR)** | 10 | 50.00% | **+52.41%** | $1,524.05 | 1.99 | 26.50% | 142.5h |
| **1:2 RR (TP 4 ATR, SL 2 ATR)** | 10 | 50.00% | **+47.82%** | $1,478.21 | 1.61 | 36.85% | 142.5h |
| **Trailing Stop (2 ATR Trail)** | 27 | 44.44% | **+206.88%** | $3,068.80 | 2.88 | 40.58% | 52.8h |

---

## 3. Primary Strategy Detailed Metrics (1:1 Risk-Reward)

- **Total Completed Trades**: 24
- **Winning Trades**: 18 (75.00%)
- **Losing Trades**: 6
- **Net Cumulative Return**: **+251.40%**
- **Ending Portfolio Value**: **$3,513.96**
- **Profit Factor**: **2.91**
- **Maximum Peak-to-Trough Drawdown**: **21.47%**
- **Average Trade Return**: **+6.00%**
- **Average Position Duration**: **59.4 hours**

---

## 4. First 20 Trade Log Sample (`tacusdt_1h_trades_report.csv`)

| Trade # | Side | Ref Price | Entry Time | Entry Price | SL Price | TP Price | Exit Time | Exit Price | Reason | Net PnL % |
| :---: | :---: | :---: | :--- | :---: | :---: | :---: | :--- | :---: | :---: | :---: |
| 1 | SHORT | 0.021491 | 2026-06-28 13:00 | 0.021491 | 0.022312 | 0.020670 | 2026-06-28 14:00 | 0.020670 | TP | **+3.72%** |
| 2 | LONG | 0.021576 | 2026-06-28 15:00 | 0.021576 | 0.020614 | 0.022538 | 2026-06-29 07:00 | 0.022538 | TP | **+4.36%** |
| 3 | LONG | 0.033951 | 2026-06-29 08:00 | 0.033951 | 0.031562 | 0.036340 | 2026-06-29 08:00 | 0.036340 | TP | **+6.94%** |
| 4 | LONG | 0.045302 | 2026-06-29 09:00 | 0.045302 | 0.041190 | 0.049414 | 2026-06-29 09:00 | 0.049414 | TP | **+8.98%** |
| 5 | SHORT | 0.051659 | 2026-06-29 10:00 | 0.051659 | 0.056998 | 0.046320 | 2026-06-29 14:00 | 0.056998 | SL | **-10.44%** |
| 6 | LONG | 0.057608 | 2026-06-29 15:00 | 0.057608 | 0.047334 | 0.067882 | 2026-06-30 14:00 | 0.067882 | TP | **+17.73%** |
| 7 | SHORT | 0.065908 | 2026-06-30 15:00 | 0.065908 | 0.075592 | 0.056224 | 2026-07-01 12:00 | 0.056224 | TP | **+14.59%** |
| 8 | LONG | 0.057059 | 2026-07-01 13:00 | 0.057059 | 0.051070 | 0.063048 | 2026-07-01 15:00 | 0.051070 | SL | **-10.60%** |
| 9 | SHORT | 0.050371 | 2026-07-01 16:00 | 0.050371 | 0.057458 | 0.043284 | 2026-07-01 17:00 | 0.043284 | TP | **+13.97%** |
| 10 | LONG | 0.042922 | 2026-07-01 18:00 | 0.042922 | 0.035065 | 0.050779 | 2026-07-01 20:00 | 0.050779 | TP | **+18.20%** |
| 11 | LONG | 0.041490 | 2026-07-01 21:00 | 0.041490 | 0.032625 | 0.050355 | 2026-07-02 13:00 | 0.032625 | SL | **-21.47%** |
| 12 | LONG | 0.030455 | 2026-07-02 14:00 | 0.030455 | 0.023862 | 0.037048 | 2026-07-02 14:00 | 0.037048 | TP | **+21.55%** |
| 13 | LONG | 0.033420 | 2026-07-02 15:00 | 0.033420 | 0.026199 | 0.040641 | 2026-07-04 11:00 | 0.040641 | TP | **+21.51%** |
| 14 | SHORT | 0.040770 | 2026-07-04 12:00 | 0.040770 | 0.045252 | 0.036288 | 2026-07-04 12:00 | 0.036288 | TP | **+10.89%** |
| 15 | SHORT | 0.035434 | 2026-07-04 13:00 | 0.035434 | 0.041437 | 0.029431 | 2026-07-05 14:00 | 0.029431 | TP | **+16.84%** |
| 16 | LONG | 0.029823 | 2026-07-05 15:00 | 0.029823 | 0.028170 | 0.031476 | 2026-07-05 19:00 | 0.031476 | TP | **+5.44%** |
| 17 | LONG | 0.031069 | 2026-07-05 20:00 | 0.031069 | 0.029442 | 0.032696 | 2026-07-06 06:00 | 0.029442 | SL | **-5.34%** |
| 18 | SHORT | 0.029627 | 2026-07-06 07:00 | 0.029627 | 0.031147 | 0.028107 | 2026-07-06 16:00 | 0.031147 | SL | **-5.23%** |
| 19 | LONG | 0.030963 | 2026-07-06 17:00 | 0.030963 | 0.029383 | 0.032543 | 2026-07-07 00:00 | 0.032543 | TP | **+5.00%** |
| 20 | LONG | 0.032911 | 2026-07-07 01:00 | 0.032911 | 0.031168 | 0.034654 | 2026-07-07 07:00 | 0.034654 | TP | **+5.20%** |

---

## 5. Summary & Technical Takeaways
1. **Continuous Cycle Execution**: The engine strictly enforced continuous cycle restarts upon position exit. As soon as a trade hit SL or TP, the exact exit bar close became the reference price $P_{ref}$ for the next cycle.
2. **Profit Trigger Direction**: Filtering entries based on initial directional profit state eliminated counter-trend stagnation.
3. **ATR Volatility Sizing**: 2 ATR SL dynamic scaling automatically expanded during high volatility regimes (news spikes) and tightened during low volatility consolidation phases.

*Full trade log exported to `tacusdt_1h_trades_report.csv`.*
