#!/usr/bin/env python3
"""
Fast Parquet Reader & Dynamic Timeframe Resampler
-------------------------------------------------
Loads 1m Parquet files for any symbol and dynamically resamples into
15m, 1h, 4h, 1d (or custom interval) candles using Polars in milliseconds.
"""

import argparse
import os
import sys
import time
import polars as pl


def load_symbol_parquet(data_dir: str, symbol: str) -> pl.DataFrame:
    """Load all month parquet files for a given symbol."""
    sym_dir = os.path.join(data_dir, symbol.upper())
    if not os.path.exists(sym_dir):
        raise FileNotFoundError(f"Directory not found for symbol {symbol}: {sym_dir}")

    pattern = os.path.join(sym_dir, "*.parquet")
    lf = pl.scan_parquet(pattern)
    df = lf.collect()
    return df.sort("open_time")


def resample_ohlcv(df: pl.DataFrame, interval: str = "15m") -> pl.DataFrame:
    """
    Resample 1m OHLCV DataFrame into target timeframe using Polars.
    Rule:
    - open: first
    - high: max
    - low: min
    - close: last
    - volume: sum
    - quote_volume: sum
    - trades_count: sum
    """
    # Convert timestamp to Datetime column if int64 (epoch ms)
    if df["open_time"].dtype in (pl.Int64, pl.UInt64):
        df = df.with_columns(
            pl.from_epoch(pl.col("open_time"), time_unit="ms").alias("datetime")
        )
    else:
        df = df.rename({"open_time": "datetime"})

    # Map interval to Polars string format (e.g. 15m, 1h, 4h, 1d)
    interval_map = {
        "1m": "1m", "3m": "3m", "5m": "5m", "15m": "15m", "30m": "30m",
        "1h": "1h", "2h": "2h", "4h": "4h", "12h": "12h", "1d": "1d"
    }
    every_str = interval_map.get(interval.lower(), interval)

    resampled = df.group_by_dynamic("datetime", every=every_str, closed="left").agg([
        pl.col("open").first().alias("open"),
        pl.col("high").max().alias("high"),
        pl.col("low").min().alias("low"),
        pl.col("close").last().alias("close"),
        pl.col("volume").sum().alias("volume"),
        pl.col("quote_volume").sum().alias("quote_volume"),
        pl.col("trades_count").sum().alias("trades_count"),
        pl.len().alias("sub_bar_count")
    ]).sort("datetime")

    return resampled


def main():
    parser = argparse.ArgumentParser(description="Fast Parquet Reader & Dynamic Resampler")
    parser.add_argument("--data_dir", type=str, default="data/parquet_klines", help="Parquet data directory")
    parser.add_argument("--symbol", type=str, default="BTCUSDT", help="Symbol to query")
    parser.add_argument("--interval", type=str, default="15m", help="Target interval (15m, 1h, 4h, 1d)")
    parser.add_argument("--head", type=int, default=10, help="Print first N rows")

    args = parser.parse_args()

    t0 = time.time()
    print(f"📖 Loading 1m Parquet data for {args.symbol} from {args.data_dir}...")
    try:
        df = load_symbol_parquet(args.data_dir, args.symbol)
    except Exception as e:
        print(f"❌ Error: {e}")
        sys.exit(1)

    t_load = time.time() - t0
    total_1m_rows = len(df)
    print(f"✅ Loaded {total_1m_rows:,} 1m rows in {t_load*1000:.2f} ms")

    t1 = time.time()
    print(f"⚡ Resampling {total_1m_rows:,} rows -> {args.interval} timeframe...")
    resampled_df = resample_ohlcv(df, args.interval)
    t_resample = time.time() - t1

    print(f"✅ Resampled into {len(resampled_df):,} candles in {t_resample*1000:.2f} ms!")
    print("=" * 80)
    print(f"📊 Preview of Resampled {args.symbol} [{args.interval}] Candles:")
    print(resampled_df.head(args.head))
    print("=" * 80)


if __name__ == "__main__":
    main()
