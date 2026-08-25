#!/usr/bin/env python3
import urllib.request
import json
import math
import datetime
import pandas as pd

def fetch_klines(symbol="TACUSDT", interval="1h", limit=1500):
    url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval={interval}&limit={limit}"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
    with urllib.request.urlopen(req) as resp:
        data = json.loads(resp.read().decode('utf-8'))
    
    bars = []
    for row in data:
        bars.append({
            'open_time': int(row[0]),
            'open': float(row[1]),
            'high': float(row[2]),
            'low': float(row[3]),
            'close': float(row[4]),
            'volume': float(row[5]),
            'close_time': int(row[6]),
        })
    return bars

def calculate_atr(bars, period=14):
    tr_list = []
    for i in range(len(bars)):
        if i == 0:
            tr = bars[i]['high'] - bars[i]['low']
        else:
            hl = bars[i]['high'] - bars[i]['low']
            hp = abs(bars[i]['high'] - bars[i - 1]['close'])
            lp = abs(bars[i]['low'] - bars[i - 1]['close'])
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

def main():
    symbol = "TACUSDT"
    interval = "1h"
    fixed_pos_size = 50.0 # 50 USDT
    lookback = 100

    print("Fetching historical TACUSDT 1h bars from Binance Futures...")
    bars = fetch_klines(symbol, interval, 1500)
    print(f"Total 1h bars fetched: {len(bars)}")

    atr_series = calculate_atr(bars, 14)
    closed_trades = []
    trade_id = 1

    for i in range(lookback, len(bars)):
        entry_bar = bars[i]
        entry_price = entry_bar['open']
        entry_time_ms = entry_bar['open_time']
        entry_time_sec = entry_time_ms // 1000
        entry_time_utc = datetime.datetime.fromtimestamp(entry_time_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')

        window_100 = bars[i - lookback : i]
        lowest_100 = min(b['low'] for b in window_100)
        atr_val = atr_series[i - 1] if i > 0 else atr_series[i]
        atr_val = max(atr_val, 0.00001)

        raw_sl = lowest_100 - (2.0 * atr_val)
        sl_dist = max(entry_price - raw_sl, entry_price * 0.005)
        stop_loss = entry_price - sl_dist
        take_profit = entry_price + (2.0 * sl_dist) # 1:2 R:R

        risk_ratio = sl_dist / entry_price
        risk_usdt = fixed_pos_size * risk_ratio
        reward_usdt = 2.0 * risk_usdt

        closed = False
        exit_time_ms = None
        exit_time_sec = None
        exit_time_utc = None
        exit_price = None
        status = None
        pnl_usdt = 0.0
        pnl_pct = 0.0
        holding_bars = 0

        for k in range(i, len(bars)):
            sim_bar = bars[k]
            holding_bars = k - i + 1

            if sim_bar['low'] <= stop_loss and sim_bar['high'] >= take_profit:
                closed = True
                exit_time_ms = sim_bar['close_time']
                exit_time_sec = exit_time_ms // 1000
                exit_time_utc = datetime.datetime.fromtimestamp(exit_time_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')
                exit_price = stop_loss
                status = "LOSS"
                pnl_usdt = -risk_usdt
                pnl_pct = -risk_ratio * 100.0
                break
            elif sim_bar['high'] >= take_profit:
                closed = True
                exit_time_ms = sim_bar['close_time']
                exit_time_sec = exit_time_ms // 1000
                exit_time_utc = datetime.datetime.fromtimestamp(exit_time_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')
                exit_price = take_profit
                status = "WIN"
                pnl_usdt = reward_usdt
                pnl_pct = 2.0 * risk_ratio * 100.0
                break
            elif sim_bar['low'] <= stop_loss:
                closed = True
                exit_time_ms = sim_bar['close_time']
                exit_time_sec = exit_time_ms // 1000
                exit_time_utc = datetime.datetime.fromtimestamp(exit_time_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')
                exit_price = stop_loss
                status = "LOSS"
                pnl_usdt = -risk_usdt
                pnl_pct = -risk_ratio * 100.0
                break

        # Record ONLY CLOSED trades
        if closed:
            closed_trades.append({
                'Trade_ID': trade_id,
                'Symbol': symbol,
                'Side': 'LONG',
                'Entry_Time_UTC': entry_time_utc,
                'Entry_Unix_ms': entry_time_ms,
                'Entry_Unix_sec': entry_time_sec,
                'Exit_Time_UTC': exit_time_utc,
                'Exit_Unix_ms': exit_time_ms,
                'Exit_Unix_sec': exit_time_sec,
                'Entry_Price': round(entry_price, 5),
                'Lowest_100_Price': round(lowest_100, 5),
                'ATR_14': round(atr_val, 5),
                'Stop_Loss_Price': round(stop_loss, 5),
                'Take_Profit_Price': round(take_profit, 5),
                'Exit_Price': round(exit_price, 5),
                'Position_Size_USDT': fixed_pos_size,
                'Risk_USDT': round(risk_usdt, 2),
                'Target_Reward_USDT': round(reward_usdt, 2),
                'Result': status,
                'PnL_USDT': round(pnl_usdt, 2),
                'PnL_Percent': round(pnl_pct, 2),
                'Holding_Bars': holding_bars,
            })
        trade_id += 1

    df = pd.DataFrame(closed_trades)
    print(f"Total closed trades exported: {len(df)}")
    print(f"WIN trades: {len(df[df['Result'] == 'WIN'])}, LOSS trades: {len(df[df['Result'] == 'LOSS'])}")

    excel_path = "/home/smhvz/Desktop/cycle-orc/tacusdt_closed_trades.xlsx"
    csv_path = "/home/smhvz/Desktop/cycle-orc/tacusdt_closed_trades.csv"

    # Export Excel and CSV
    df.to_excel(excel_path, index=False, engine='openpyxl')
    df.to_csv(csv_path, index=False, encoding='utf-8-sig')

    print(f"Successfully generated Excel file: {excel_path}")
    print(f"Successfully generated CSV file: {csv_path}")

if __name__ == "__main__":
    main()
