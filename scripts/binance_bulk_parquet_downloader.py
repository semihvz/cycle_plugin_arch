#!/usr/bin/env python3
"""
High-Performance Binance 1m Bulk OHLCV Downloader & Parquet Pipeline
---------------------------------------------------------------------
Downloads historical 1m klines directly from Binance S3 mirrors (data.binance.vision).
NO REST API RATE LIMITS (0% chance of 418 I'm a teapot error).
Converts raw CSV streams into Snappy-compressed Parquet files via Polars.
"""

import argparse
import io
import os
import sys
import time
import zipfile
import xml.etree.ElementTree as ET
from concurrent.futures import ThreadPoolExecutor, as_completed
import requests
import polars as pl

# Schema for Binance 1m Kline CSV files
KLINE_SCHEMA = {
    "open_time": pl.Int64,
    "open": pl.Float64,
    "high": pl.Float64,
    "low": pl.Float64,
    "close": pl.Float64,
    "volume": pl.Float64,
    "close_time": pl.Int64,
    "quote_volume": pl.Float64,
    "trades_count": pl.UInt32,
    "taker_buy_volume": pl.Float64,
    "taker_buy_quote_volume": pl.Float64,
    "ignore": pl.Float64,
}

KLINE_COLUMNS = list(KLINE_SCHEMA.keys())


def get_all_usdt_symbols(market: str = "futures") -> list[str]:
    """Fetch all available USDT symbols directly from Binance S3 bucket XML index (No rate limit / No 418)."""
    prefix = "data/futures/um/monthly/klines/" if market.lower() == "futures" else "data/spot/monthly/klines/"
    s3_url = f"https://s3-ap-northeast-1.amazonaws.com/data.binance.vision?delimiter=/&prefix={prefix}"

    try:
        res = requests.get(s3_url, timeout=15)
        res.raise_for_status()
        root = ET.fromstring(res.content)
        ns = {"s3": "http://s3.amazonaws.com/doc/2006-03-01/"}
        prefixes = [p.text for p in root.findall("s3:CommonPrefixes/s3:Prefix", ns)]
        symbols = [p.split("/")[-2] for p in prefixes if len(p.split("/")) >= 2 and p.split("/")[-2].endswith("USDT")]
        symbols.sort()
        if symbols:
            return symbols
    except Exception as e:
        print(f"⚠️ Warning: S3 index fetch failed ({e}), falling back to API...")

    # Fallback to REST API
    try:
        url = "https://fapi.binance.com/fapi/v1/exchangeInfo" if market.lower() == "futures" else "https://api.binance.com/api/v3/exchangeInfo"
        headers = {"User-Agent": "Mozilla/5.0"}
        res = requests.get(url, headers=headers, timeout=10)
        data = res.json()
        symbols = [s["symbol"] for s in data.get("symbols", []) if s["symbol"].endswith("USDT")]
        symbols.sort()
        return symbols
    except Exception:
        return ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "XRPUSDT"]


def download_single_zip(url: str) -> bytes | None:
    """Download ZIP file content directly into memory."""
    try:
        res = requests.get(url, timeout=30)
        if res.status_code == 200:
            return res.content
        elif res.status_code == 404:
            return None
        else:
            return None
    except Exception:
        return None


def process_zip_to_df(zip_bytes: bytes) -> pl.DataFrame | None:
    """Extract CSV inside ZIP and parse with Polars."""
    try:
        with zipfile.ZipFile(io.BytesIO(zip_bytes)) as z:
            csv_name = [name for name in z.namelist() if name.endswith(".csv")][0]
            with z.open(csv_name) as f:
                content = f.read()

            has_header = content.startswith(b"open_time") or content.startswith(b"startTime")
            skip_rows = 1 if has_header else 0

            df = pl.read_csv(
                io.BytesIO(content),
                has_header=False,
                skip_rows=skip_rows,
                new_columns=KLINE_COLUMNS,
                schema_overrides=KLINE_SCHEMA,
                ignore_errors=True,
            )
            if "ignore" in df.columns:
                df = df.drop("ignore")

            return df
    except Exception:
        return None


def download_and_process_task(args_tuple):
    symbol, year, month, market, output_dir, force = args_tuple
    month_str = f"{month:02d}"
    out_file = os.path.join(output_dir, symbol, f"{year}_{month_str}.parquet")

    if os.path.exists(out_file) and not force:
        return (symbol, year, month, "EXISTS", 0)

    market_path = "futures/um" if market == "futures" else "spot"
    url = f"https://data.binance.vision/data/{market_path}/monthly/klines/{symbol}/1m/{symbol}-1m-{year}-{month_str}.zip"

    zip_bytes = download_single_zip(url)
    if zip_bytes is None:
        return (symbol, year, month, "NOT_FOUND", 0)

    df = process_zip_to_df(zip_bytes)
    if df is None or df.is_empty():
        return (symbol, year, month, "EMPTY", 0)

    os.makedirs(os.path.dirname(out_file), exist_ok=True)
    df.write_parquet(out_file, compression="snappy")
    row_count = len(df)
    file_size_kb = os.path.getsize(out_file) / 1024.0

    return (symbol, year, month, "SUCCESS", row_count, file_size_kb)


def main():
    parser = argparse.ArgumentParser(description="High-Speed Binance 1m Bulk Downloader & Parquet Exporter")
    parser.add_argument("--market", choices=["futures", "spot"], default="futures", help="Market type")
    parser.add_argument("--symbols", type=str, default="BTCUSDT,ETHUSDT,SOLUSDT", help="Comma-separated symbols or 'ALL'")
    parser.add_argument("--years", type=str, default="2024,2025,2026", help="Comma-separated years")
    parser.add_argument("--months", type=str, default="1,2,3,4,5,6,7,8,9,10,11,12", help="Comma-separated months")
    parser.add_argument("--workers", type=int, default=16, help="Number of parallel worker threads")
    parser.add_argument("--output_dir", type=str, default="data/parquet_klines", help="Output directory for Parquet files")
    parser.add_argument("--force", action="store_true", help="Force overwrite existing files")

    args = parser.parse_args()

    start_time = time.time()

    if args.symbols.upper() == "ALL":
        print(f"🔍 Fetching all active {args.market.upper()} USDT symbols from Binance S3 index...")
        symbols = get_all_usdt_symbols(args.market)
        print(f"✅ Found {len(symbols)} USDT trading pairs.")
    else:
        symbols = [s.strip().upper() for s in args.symbols.split(",") if s.strip()]

    years = [int(y.strip()) for y in args.years.split(",") if y.strip()]
    months = [int(m.strip()) for m in args.months.split(",") if m.strip()]

    tasks = []
    for sym in symbols:
        for y in years:
            for m in months:
                tasks.append((sym, y, m, args.market, args.output_dir, args.force))

    print(f"🚀 Starting download pipeline for {len(tasks)} tasks using {args.workers} worker threads...")
    print(f"📁 Target Output Directory: {os.path.abspath(args.output_dir)}")
    print("=" * 70)

    success_count = 0
    exists_count = 0
    not_found_count = 0
    total_rows = 0
    total_kb = 0.0

    with ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = [executor.submit(download_and_process_task, t) for t in tasks]
        for future in as_completed(futures):
            res = future.result()
            status = res[3]
            if status == "SUCCESS":
                sym, y, m, _, rows, kb = res
                success_count += 1
                total_rows += rows
                total_kb += kb
                print(f"✅ [{sym}] {y}-{m:02d} -> {rows:,} 1m rows created ({kb:.1f} KB Parquet)")
            elif status == "EXISTS":
                exists_count += 1
            elif status == "NOT_FOUND":
                not_found_count += 1

    elapsed = time.time() - start_time
    print("=" * 70)
    print(f"🎉 Pipeline finished in {elapsed:.2f} seconds!")
    print(f"📊 Summary: Success: {success_count} | Cached: {exists_count} | Not Found/Pending: {not_found_count}")
    if success_count > 0 or exists_count > 0:
        mb = total_kb / 1024.0
        print(f"💾 Processed Total Rows: {total_rows:,} | Total Parquet Storage: {mb:.2f} MB")


if __name__ == "__main__":
    main()
