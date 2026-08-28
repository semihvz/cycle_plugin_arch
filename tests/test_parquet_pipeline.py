#!/usr/bin/env python3
"""
Integration test for Binance Bulk Parquet Pipeline
"""

import os
import subprocess
import pytest
import polars as pl

def test_parquet_download_and_resample():
    # Run downloader for a single month test sample
    cmd_dl = [
        "python3", "scripts/binance_bulk_parquet_downloader.py",
        "--symbols", "BTCUSDT",
        "--years", "2024",
        "--months", "1",
        "--output_dir", "data/parquet_klines"
    ]
    res_dl = subprocess.run(cmd_dl, capture_output=True, text=True)
    assert res_dl.returncode == 0, f"Downloader failed: {res_dl.stderr}"
    assert "Success: 1" in res_dl.stdout or "Cached: 1" in res_dl.stdout

    parquet_file = "data/parquet_klines/BTCUSDT/2024_01.parquet"
    assert os.path.exists(parquet_file)

    # Check Parquet schema & content
    df = pl.read_parquet(parquet_file)
    assert len(df) == 44640 # 31 days * 1440 mins = 44640 rows
    assert "open" in df.columns
    assert "close" in df.columns
    assert "volume" in df.columns

    # Test query & resample script
    cmd_qr = [
        "python3", "scripts/query_and_resample_parquet.py",
        "--symbol", "BTCUSDT",
        "--interval", "15m"
    ]
    res_qr = subprocess.run(cmd_qr, capture_output=True, text=True)
    assert res_qr.returncode == 0, f"Query script failed: {res_qr.stderr}"
    assert "Resampled into" in res_qr.stdout

if __name__ == "__main__":
    test_parquet_download_and_resample()
    print("✅ All parquet pipeline integration tests passed!")
