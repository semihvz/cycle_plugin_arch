#!/usr/bin/env python3
"""
All-Bars Backtest System - Multi-Symbol Full Historical Data Aggregator
-----------------------------------------------------------------------
Tüm Binance Futures USDT pariteleri için 1h zaman diliminde geçmiş tüm zaman
aralıklarındaki (paginated) mum verilerini indirir, Every-Bar backtestini koşturur
ve tüm sonuçları tek bir SQLite veritabanında toplar.

Disk Alanı Güvenliği:
- Her sembol öncesinde disk alanını denetler. Kullanılabilir alan 2.0 GB altına düşerse
  işlemi güvenli bir şekilde durdurur ve veritabanını kapatır.
"""

import argparse
import datetime
import json
import os
import shutil
import sqlite3
import sys
import time
import urllib.request

# Package path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from all_bars.engine import run_all_bars_backtest, format_ts


def check_disk_space(path=".", min_free_gb=2.0):
    """Disk alanını kontrol eder. Belirtilen GB sınırından azsa False döner."""
    try:
        stat = shutil.disk_usage(path)
        free_gb = stat.free / (1024 ** 3)
        return free_gb >= min_free_gb, free_gb
    except Exception:
        return True, 999.0


def get_all_usdt_futures_symbols():
    """Binance Futures borsasındaki tüm aktif USDT perpetual paritelerini döndürür."""
    url = "https://fapi.binance.com/fapi/v1/exchangeInfo"
    req = urllib.request.Request(
        url,
        headers={
            "User-Agent": (
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
            )
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read().decode("utf-8"))

        symbols = []
        for s in data.get("symbols", []):
            if (
                s.get("quoteAsset") == "USDT"
                and s.get("status") == "TRADING"
                and s.get("contractType") == "PERPETUAL"
            ):
                symbols.append(s["symbol"])
        return sorted(symbols)
    except Exception as e:
        print(f"[HATA] exchangeInfo sorgulama hatası: {e}")
        return ["BTCUSDT", "ETHUSDT", "SOLUSDT", "AVAXUSDT", "DOGEUSDT", "ADAUSDT", "TACUSDT"]


def fetch_all_historical_klines(symbol, interval="1h", max_chunks=4):
    """
    Binance Futures REST API'den bir sembole ait geçmiş zaman aralıklarını
    sayfalandırarak geriye dönük çeker.
    """
    all_bars_map = {}
    end_time = None

    for _ in range(max_chunks):
        url = f"https://fapi.binance.com/fapi/v1/klines?symbol={symbol}&interval={interval}&limit=1500"
        if end_time:
            url += f"&endTime={end_time}"

        req = urllib.request.Request(
            url,
            headers={
                "User-Agent": (
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 "
                    "(KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
                )
            },
        )

        try:
            with urllib.request.urlopen(req, timeout=12) as resp:
                raw_data = json.loads(resp.read().decode("utf-8"))
        except Exception:
            time.sleep(0.5)
            break

        if not raw_data or not isinstance(raw_data, list):
            break

        for row in raw_data:
            open_time = int(row[0])
            all_bars_map[open_time] = {
                "open_time": open_time,
                "open": float(row[1]),
                "high": float(row[2]),
                "low": float(row[3]),
                "close": float(row[4]),
                "volume": float(row[5]),
                "close_time": int(row[6]),
            }

        earliest_time = min(raw_data, key=lambda x: int(x[0]))[0]
        if end_time is not None and earliest_time >= end_time:
            break
        end_time = int(earliest_time) - 1

        if len(raw_data) < 1500:
            break

        time.sleep(0.05)

    sorted_bars = [all_bars_map[k] for k in sorted(all_bars_map.keys())]
    return sorted_bars


def init_aggregate_db(db_path):
    """Veritabanı var mı kontrol eder, yoksa oluşturup şemayı kurar."""
    os.makedirs(os.path.dirname(os.path.abspath(db_path)), exist_ok=True)

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    cursor.execute("""
    CREATE TABLE IF NOT EXISTS symbol_summaries (
        symbol TEXT PRIMARY KEY,
        interval TEXT NOT NULL,
        total_bars INTEGER NOT NULL,
        total_trades INTEGER NOT NULL,
        winning_trades INTEGER NOT NULL,
        losing_trades INTEGER NOT NULL,
        open_trades INTEGER NOT NULL,
        win_rate_pct REAL NOT NULL,
        net_pnl_usdt REAL NOT NULL,
        profit_factor REAL NOT NULL,
        max_drawdown_usdt REAL NOT NULL,
        max_drawdown_pct REAL NOT NULL,
        avg_trade_pnl_usdt REAL NOT NULL,
        last_updated_utc TEXT NOT NULL
    );
    """)

    cursor.execute("""
    CREATE TABLE IF NOT EXISTS closed_trades (
        global_trade_id INTEGER PRIMARY KEY AUTOINCREMENT,
        trade_id INTEGER NOT NULL,
        symbol TEXT NOT NULL,
        entry_time_utc TEXT NOT NULL,
        entry_unix_ms INTEGER NOT NULL,
        exit_time_utc TEXT,
        exit_unix_ms INTEGER,
        entry_price REAL NOT NULL,
        lowest_100_price REAL NOT NULL,
        atr_14 REAL NOT NULL,
        stop_loss_price REAL NOT NULL,
        take_profit_price REAL NOT NULL,
        exit_price REAL,
        position_size_usdt REAL NOT NULL,
        risk_usdt REAL NOT NULL,
        target_reward_usdt REAL NOT NULL,
        result TEXT NOT NULL,
        pnl_usdt REAL NOT NULL,
        pnl_percent REAL NOT NULL,
        holding_bars INTEGER NOT NULL
    );
    """)

    cursor.execute("CREATE INDEX IF NOT EXISTS idx_trades_symbol ON closed_trades(symbol);")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_trades_result ON closed_trades(result);")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_trades_entry ON closed_trades(entry_unix_ms);")

    conn.commit()
    return conn


def save_symbol_to_db(conn, summary, save_lookback_bars=False):
    """Bir sembole ait backtest özetini ve kapalı işlemlerini veritabanına ekler."""
    cursor = conn.cursor()
    now_str = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d %H:%M:%S UTC")

    cursor.execute("""
    INSERT OR REPLACE INTO symbol_summaries VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?);
    """, (
        summary["symbol"],
        summary["interval"],
        summary["total_bars"],
        summary["total_trades"],
        summary["winning_trades"],
        summary["losing_trades"],
        summary["open_trades"],
        summary["win_rate_pct"],
        summary["total_net_pnl_usdt"],
        summary["profit_factor"],
        summary["max_drawdown_usdt"],
        summary["max_drawdown_pct"],
        summary["avg_trade_pnl_usdt"],
        now_str,
    ))

    trade_rows = []
    for t in summary["trade_history"]:
        trade_rows.append((
            t["id"],
            t["symbol"],
            t["entry_time_str"],
            t["entry_time"],
            t["exit_time_str"],
            t["exit_time"],
            t["entry_price"],
            t["lowest_100_price"],
            t["atr_14"],
            t["stop_loss"],
            t["take_profit"],
            t["exit_price"],
            t["position_size_usdt"],
            t["risk_usdt"],
            t["target_reward_usdt"],
            t["status"],
            t["pnl_usdt"],
            t["pnl_pct"],
            t["holding_bars"],
        ))

    cursor.executemany("""
    INSERT INTO closed_trades (
        trade_id, symbol, entry_time_utc, entry_unix_ms, exit_time_utc, exit_unix_ms,
        entry_price, lowest_100_price, atr_14, stop_loss_price, take_profit_price,
        exit_price, position_size_usdt, risk_usdt, target_reward_usdt, result,
        pnl_usdt, pnl_percent, holding_bars
    ) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?);
    """, trade_rows)

    conn.commit()


def main():
    parser = argparse.ArgumentParser(
        description="All Binance Futures USDT Pairs Every-Bar Backtest Aggregator with Disk Space Safety"
    )
    parser.add_argument("--interval", type=str, default="1h", help="Mum zaman dilimi (ör: 1h)")
    parser.add_argument(
        "--max-chunks",
        type=int,
        default=4,
        help="Sembol başına geriye dönük çekilecek mum paket sayısı",
    )
    parser.add_argument("--pos-size", type=float, default=50.0, help="Sabit pozisyon büyüklüğü (USDT)")
    parser.add_argument(
        "--min-free-gb",
        type=float,
        default=2.0,
        help="Minimum boş disk alanı sınırı (GB). Altına düşerse durur.",
    )
    parser.add_argument(
        "--db-path",
        type=str,
        default=os.path.join(os.path.dirname(__file__), "output", "all_usdt_futures_1h_backtest.db"),
        help="Toplu SQLite DB veritabanı kayıt yolu",
    )
    args = parser.parse_args()

    print("==========================================================================================")
    print("🔥 BINANCE FUTURES TÜM USDT PARİTELERİ (1h) BACKTEST & DİSK KORUMALI VERİTABANI SERVİSİ")
    print("==========================================================================================")
    print(f"Hedef Veritabanı Yolu : {args.db_path}")
    print(f"Disk Alanı Sınırı    : Minimum {args.min_free_gb:.2f} GB boş alan korunacak.\n")

    symbols = get_all_usdt_futures_symbols()
    print(f"[BİLGİ] Toplam {len(symbols)} aktif Binance Futures USDT paritesi bulundu.")

    conn = init_aggregate_db(args.db_path)

    total_processed_symbols = 0
    total_aggregate_trades = 0
    total_aggregate_pnl = 0.0
    stopped_due_to_disk = False

    print("-" * 100)
    for idx, sym in enumerate(symbols, 1):
        # Disk alanı denetimi
        safe, free_gb = check_disk_space(args.db_path, min_free_gb=args.min_free_gb)
        if not safe:
            print(f"\n[⚠️ DİSK UYARISI] Kalan boş disk alanı: {free_gb:.2f} GB (Sınır: {args.min_free_gb:.2f} GB).")
            print("[⚠️ KORUMA AKTİF] Disk dolmasını önlemek için veri toplama durduruluyor!")
            stopped_due_to_disk = True
            break

        bars = fetch_all_historical_klines(sym, interval=args.interval, max_chunks=args.max_chunks)
        if len(bars) <= 100:
            print(f"[{idx}/{len(symbols)}] {sym:<12} -> Yetersiz mum ({len(bars)} adet). Atlanıyor.")
            continue

        summary = run_all_bars_backtest(
            symbol=sym,
            interval=args.interval,
            bars=bars,
            lookback=100,
            fixed_pos_size=args.pos_size,
        )

        save_symbol_to_db(conn, summary)

        total_processed_symbols += 1
        total_aggregate_trades += summary["total_trades"]
        total_aggregate_pnl += summary["total_net_pnl_usdt"]

        print(
            f"[{idx:<3}/{len(symbols)}] {sym:<12} | Bar: {summary['total_bars']:<5} | "
            f"İşlem: {summary['total_trades']:<4} | WinRate: {summary['win_rate_pct']:>5.1f}% | "
            f"Net PnL: {summary['total_net_pnl_usdt']:>+9.2f} USDT | Boş Disk: {free_gb:.2f} GB"
        )

    conn.close()

    print("\n" + "=" * 100)
    if stopped_due_to_disk:
        print("🛑 DİSK ALANI SINIRINA ULAŞILDIĞI İÇİN SİSTEM GÜVENLİ BİR ŞEKİLDE DURDURULDU.")
    else:
        print("🎉 TÜM PARİTELERİN BACKTEST İŞLEMİ VE VERİTABANI KAYDI TAMAMLANDI!")
    print("=" * 100)
    print(f"İşlenen Sembol Sayısı         : {total_processed_symbols} adet")
    print(f"Veritabanına Yazılan İşlem   : {total_aggregate_trades} adet")
    print(f"Toplam Net Kâr / Zarar (PnL) : {total_aggregate_pnl:+.2f} USDT")
    print(f"Veritabanı Dosyası Yolu     : {args.db_path}")
    print("=" * 100 + "\n")


if __name__ == "__main__":
    main()
