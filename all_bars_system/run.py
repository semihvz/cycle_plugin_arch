#!/usr/bin/env python3
"""
All-Bars Backtest System - Main CLI Entrypoint
----------------------------------------------
Kullanım:
  python3 run.py --symbol TACUSDT --interval 1h --limit 1500
"""

import argparse
import os
import sys

# Bulunduğu klasörü Python path'ine ekle
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from all_bars.fetcher import fetch_klines
from all_bars.engine import run_all_bars_backtest
from all_bars.storage import save_to_sqlite, export_to_csv
from all_bars.reporter import print_report


def main():
    parser = argparse.ArgumentParser(description="All-Bars Backtest Standalone Python System")
    parser.add_argument("--symbol", type=str, default="TACUSDT", help="İşlem çifti (ör: TACUSDT)")
    parser.add_argument("--interval", type=str, default="1h", help="Mum zaman dilimi (ör: 1h, 15m)")
    parser.add_argument("--limit", type=int, default=1500, help="Çekilecek mum sayısı (max: 1500)")
    parser.add_argument("--pos-size", type=float, default=50.0, help="Sabit pozisyon büyüklüğü (USDT)")
    parser.add_argument(
        "--db-path",
        type=str,
        default=os.path.join(os.path.dirname(__file__), "output", "tacusdt_all_bars_backtest.db"),
        help="SQLite DB kayıt yolu",
    )
    parser.add_argument(
        "--csv-path",
        type=str,
        default=os.path.join(os.path.dirname(__file__), "output", "tacusdt_all_bars_closed_trades.csv"),
        help="CSV döküm yolu",
    )
    parser.add_argument("--no-db", action="store_true", help="SQLite kaydını atla")
    parser.add_argument("--no-csv", action="store_true", help="CSV dökümünü atla")
    args = parser.parse_args()

    print(f"[BİLGİ] {args.symbol} {args.interval} verileri Binance Futures'tan indiriliyor (Limit: {args.limit})...")
    bars = fetch_klines(args.symbol, args.interval, args.limit)
    print(f"[BİLGİ] Toplam {len(bars)} mum çekildi. Backtest çalıştırılıyor...")

    summary = run_all_bars_backtest(
        symbol=args.symbol,
        interval=args.interval,
        bars=bars,
        lookback=100,
        fixed_pos_size=args.pos_size,
    )

    print_report(summary)

    if not args.no_db:
        save_to_sqlite(summary, db_path=args.db_path)

    if not args.no_csv:
        export_to_csv(summary, csv_path=args.csv_path)


if __name__ == "__main__":
    main()
