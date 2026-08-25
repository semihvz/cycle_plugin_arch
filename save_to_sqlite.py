#!/usr/bin/env python3
import urllib.request
import json
import sqlite3
import datetime
import os

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
    fixed_pos_size = 50.0
    lookback = 100
    db_path = "/home/smhvz/Desktop/cycle-orc/tacusdt_backtest.db"

    if os.path.exists(db_path):
        os.remove(db_path)

    print("Fetching historical TACUSDT 1h bars from Binance Futures...")
    bars = fetch_klines(symbol, interval, 1500)
    print(f"Total 1h bars fetched: {len(bars)}")

    atr_series = calculate_atr(bars, 14)

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Table 1: closed_trades
    cursor.execute("""
    CREATE TABLE closed_trades (
        trade_id INTEGER PRIMARY KEY,
        symbol TEXT NOT NULL,
        side TEXT NOT NULL,
        entry_time_utc TEXT NOT NULL,
        entry_unix_ms INTEGER NOT NULL,
        entry_unix_sec INTEGER NOT NULL,
        exit_time_utc TEXT NOT NULL,
        exit_unix_ms INTEGER NOT NULL,
        exit_unix_sec INTEGER NOT NULL,
        entry_price REAL NOT NULL,
        lowest_100_price REAL NOT NULL,
        atr_14 REAL NOT NULL,
        stop_loss_price REAL NOT NULL,
        take_profit_price REAL NOT NULL,
        exit_price REAL NOT NULL,
        position_size_usdt REAL NOT NULL,
        risk_usdt REAL NOT NULL,
        target_reward_usdt REAL NOT NULL,
        result TEXT NOT NULL,
        pnl_usdt REAL NOT NULL,
        pnl_percent REAL NOT NULL,
        holding_bars INTEGER NOT NULL
    );
    """)

    # Table 2: trade_lookback_bars (100 bars prior to entry for each trade)
    cursor.execute("""
    CREATE TABLE trade_lookback_bars (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        trade_id INTEGER NOT NULL,
        bar_offset INTEGER NOT NULL, -- -100 to -1
        open_time_ms INTEGER NOT NULL,
        open_time_utc TEXT NOT NULL,
        open REAL NOT NULL,
        high REAL NOT NULL,
        low REAL NOT NULL,
        close REAL NOT NULL,
        volume REAL NOT NULL,
        close_time_ms INTEGER NOT NULL,
        FOREIGN KEY (trade_id) REFERENCES closed_trades (trade_id)
    );
    """)

    cursor.execute("CREATE INDEX idx_closed_trades_result ON closed_trades(result);")
    cursor.execute("CREATE INDEX idx_closed_trades_entry_ms ON closed_trades(entry_unix_ms);")
    cursor.execute("CREATE INDEX idx_lookback_trade_id ON trade_lookback_bars(trade_id);")

    trade_id = 1
    closed_trades_count = 0
    lookback_bars_count = 0

    trade_rows = []
    lookback_rows = []

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
        take_profit = entry_price + (2.0 * sl_dist)

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

        if closed:
            trade_rows.append((
                trade_id, symbol, 'LONG', entry_time_utc, entry_time_ms, entry_time_sec,
                exit_time_utc, exit_time_ms, exit_time_sec, round(entry_price, 5),
                round(lowest_100, 5), round(atr_val, 5), round(stop_loss, 5),
                round(take_profit, 5), round(exit_price, 5), fixed_pos_size,
                round(risk_usdt, 2), round(reward_usdt, 2), status, round(pnl_usdt, 2),
                round(pnl_pct, 2), holding_bars
            ))

            # Add the 100 lookback bars for this trade
            for idx_off, l_bar in enumerate(window_100):
                offset = idx_off - 100 # -100 to -1
                bar_open_sec = l_bar['open_time'] // 1000
                bar_open_utc = datetime.datetime.fromtimestamp(bar_open_sec, tz=datetime.timezone.utc).strftime('%Y-%m-%d %H:%M:%S UTC')

                lookback_rows.append((
                    trade_id, offset, l_bar['open_time'], bar_open_utc,
                    round(l_bar['open'], 5), round(l_bar['high'], 5),
                    round(l_bar['low'], 5), round(l_bar['close'], 5),
                    round(l_bar['volume'], 2), l_bar['close_time']
                ))

            closed_trades_count += 1
            lookback_bars_count += len(window_100)

        trade_id += 1

    cursor.executemany("""
    INSERT INTO closed_trades VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
    """, trade_rows)

    cursor.executemany("""
    INSERT INTO trade_lookback_bars (trade_id, bar_offset, open_time_ms, open_time_utc, open, high, low, close, volume, close_time_ms)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?);
    """, lookback_rows)

    conn.commit()
    conn.close()

    db_size_mb = os.path.getsize(db_path) / (1024 * 1024)
    print(f"\n==============================================================")
    print(f"✅ SQLite Database Successfully Created: {db_path}")
    print(f"📊 Total Closed Trades Inserted: {closed_trades_count} rows in 'closed_trades'")
    print(f"📈 Total Lookback Bars Inserted: {lookback_bars_count} rows in 'trade_lookback_bars'")
    print(f"💾 Database File Size: {db_size_mb:.2f} MB")
    print(f"==============================================================\n")

if __name__ == "__main__":
    main()
