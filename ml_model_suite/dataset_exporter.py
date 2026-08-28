#!/usr/bin/env python3
import sqlite3
import pandas as pd
import numpy as np
import os
import json

def export_dataset(db_path="/home/smhvz/Desktop/cycle-orc/all_bars_system/output/all_usdt_futures_1h_backtest.db", output_dir="/home/smhvz/Desktop/cycle-orc/ml_model_suite/data"):
    os.makedirs(output_dir, exist_ok=True)
    
    if not os.path.exists(db_path):
        raise FileNotFoundError(f"Database not found at {db_path}. Please run backtest first.")

    print(f"Loading database from {db_path}...")
    conn = sqlite3.connect(db_path)
    
    lookback_cols = [r[1] for r in conn.execute("PRAGMA table_info(trade_lookback_bars);").fetchall()]
    id_col = "global_trade_id" if "global_trade_id" in lookback_cols else "trade_id"

    trades_df = pd.read_sql_query(f"SELECT * FROM closed_trades ORDER BY {id_col} ASC;", conn)
    print(f"Loaded {len(trades_df)} closed trades. Fetching lookback bars...")
    
    lookback_df = pd.read_sql_query(
        f"SELECT {id_col}, bar_offset, open, high, low, close, volume FROM trade_lookback_bars ORDER BY {id_col} ASC, bar_offset ASC;",
        conn
    )
    conn.close()

    print(f"Processing {len(trades_df)} closed trades and {len(lookback_df)} lookback bars (using key '{id_col}')...")
    print("Grouping lookback bars for fast O(1) indexing...")
    grouped_lookback = dict(list(lookback_df.groupby(id_col)))
    
    features_list = []

    for idx, trade in trades_df.iterrows():
        tid = trade[id_col]
        if tid not in grouped_lookback:
            continue
            
        t_bars = grouped_lookback[tid]
        
        if len(t_bars) < 100:
            continue

        closes = t_bars['close'].values
        opens = t_bars['open'].values
        highs = t_bars['high'].values
        lows = t_bars['low'].values
        volumes = t_bars['volume'].values

        entry_price = trade['entry_price']
        lowest_100 = trade['lowest_100_price']
        highest_100 = highs.max()

        trend_100b_pct = ((closes[-1] - closes[0]) / closes[0]) * 100.0
        trend_50b_pct = ((closes[-1] - closes[-50]) / closes[-50]) * 100.0
        trend_20b_pct = ((closes[-1] - closes[-20]) / closes[-20]) * 100.0
        stoch_pos_pct = ((entry_price - lowest_100) / max(highest_100 - lowest_100, 1e-8)) * 100.0
        norm_atr_pct = (trade['atr_14'] / entry_price) * 100.0
        volatility_range_pct = ((highest_100 - lowest_100) / entry_price) * 100.0
        vol_10_mean = volumes[-10:].mean()
        vol_100_mean = volumes.mean()
        volume_ratio = vol_10_mean / max(vol_100_mean, 1e-8)
        
        entry_time_str = trade['entry_time_utc']
        entry_hour = int(entry_time_str.split()[1].split(':')[0]) if ' ' in entry_time_str else 0
        dist_to_100low_pct = ((entry_price - lowest_100) / entry_price) * 100.0
        
        last_body = abs(closes[-1] - opens[-1])
        last_range = max(highs[-1] - lows[-1], 1e-8)
        last_bar_body_ratio = last_body / last_range
        last_bar_is_bullish = 1 if closes[-1] > opens[-1] else 0

        target = 1 if trade['result'] == 'WIN' else 0

        features_list.append({
            'trade_id': tid,
            'symbol': trade['symbol'],
            'side': trade.get('side', 'LONG'),
            'entry_time_utc': trade['entry_time_utc'],
            'entry_price': entry_price,
            'lowest_100_price': lowest_100,
            'highest_100_price': highest_100,
            'atr_14': trade['atr_14'],
            'stop_loss_price': trade['stop_loss_price'],
            'take_profit_price': trade['take_profit_price'],
            'risk_usdt': trade['risk_usdt'],
            'target_reward_usdt': trade['target_reward_usdt'],
            'result': trade['result'],
            'pnl_usdt': trade['pnl_usdt'],
            'holding_bars': trade['holding_bars'],
            'target': target,
            # Technical Features
            'trend_100b_pct': round(trend_100b_pct, 4),
            'trend_50b_pct': round(trend_50b_pct, 4),
            'trend_20b_pct': round(trend_20b_pct, 4),
            'stoch_pos_pct': round(stoch_pos_pct, 4),
            'norm_atr_pct': round(norm_atr_pct, 4),
            'volatility_range_pct': round(volatility_range_pct, 4),
            'volume_ratio': round(volume_ratio, 4),
            'entry_hour': entry_hour,
            'dist_to_100low_pct': round(dist_to_100low_pct, 4),
            'last_bar_body_ratio': round(last_bar_body_ratio, 4),
            'last_bar_is_bullish': last_bar_is_bullish,
        })

    df = pd.DataFrame(features_list)
    csv_file = os.path.join(output_dir, "dataset.csv")
    json_file = os.path.join(output_dir, "dataset.json")

    df.to_csv(csv_file, index=False)
    if len(df) <= 50000:
        with open(json_file, "w") as f:
            json.dump(features_list, f, indent=2)

    print(f"✅ Successfully exported dataset with {len(df)} samples:")
    print(f"   • CSV File : {csv_file}")
    if len(df) <= 50000:
        print(f"   • JSON File: {json_file}")
    return df

if __name__ == "__main__":
    export_dataset()
