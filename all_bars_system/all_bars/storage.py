"""
SQLite Veritabanı ve CSV Dışa Aktarım Modülü
"""

import csv
import os
import sqlite3
from .engine import format_ts


def save_to_sqlite(summary, db_path="output/tacusdt_all_bars_backtest.db"):
    """Backtest sonuçlarını ve 100-bar lookback mum verilerini SQLite veritabanına kaydeder."""
    os.makedirs(os.path.dirname(os.path.abspath(db_path)), exist_ok=True)

    if os.path.exists(db_path):
        os.remove(db_path)

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    cursor.execute("""
    CREATE TABLE closed_trades (
        trade_id INTEGER PRIMARY KEY,
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

    cursor.execute("""
    CREATE TABLE trade_lookback_bars (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        trade_id INTEGER NOT NULL,
        bar_offset INTEGER NOT NULL,
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

    cursor.execute("CREATE INDEX idx_trades_result ON closed_trades(result);")
    cursor.execute("CREATE INDEX idx_trades_entry ON closed_trades(entry_unix_ms);")
    cursor.execute("CREATE INDEX idx_lookback_trade ON trade_lookback_bars(trade_id);")

    trade_rows = []
    lookback_rows = []

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

        for idx, lb in enumerate(t["lookback_bars"]):
            offset = idx - len(t["lookback_bars"])
            lookback_rows.append((
                t["id"],
                offset,
                lb["open_time"],
                format_ts(lb["open_time"]),
                lb["open"],
                lb["high"],
                lb["low"],
                lb["close"],
                lb["volume"],
                lb["close_time"],
            ))

    cursor.executemany("""
    INSERT INTO closed_trades VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?);
    """, trade_rows)

    cursor.executemany("""
    INSERT INTO trade_lookback_bars (trade_id, bar_offset, open_time_ms, open_time_utc, open, high, low, close, volume, close_time_ms)
    VALUES (?,?,?,?,?,?,?,?,?,?);
    """, lookback_rows)

    conn.commit()
    conn.close()
    print(f"[BAŞARILI] Backtest verileri SQLite DB'ye kaydedildi: {db_path}")


def export_to_csv(summary, csv_path="output/tacusdt_all_bars_closed_trades.csv"):
    """İşlem kayıtlarını CSV formatında kaydeder."""
    os.makedirs(os.path.dirname(os.path.abspath(csv_path)), exist_ok=True)

    with open(csv_path, mode="w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow([
            "TradeID", "Symbol", "EntryTimeUTC", "EntryPrice", "100BarLow", "ATR14",
            "StopLoss", "TakeProfit", "ExitTimeUTC", "ExitPrice", "Result",
            "RiskUSDT", "RewardUSDT", "PnL_USDT", "PnL_Pct", "HoldingBars"
        ])
        for t in summary["trade_history"]:
            writer.writerow([
                t["id"], t["symbol"], t["entry_time_str"], f"{t['entry_price']:.5f}",
                f"{t['lowest_100_price']:.5f}", f"{t['atr_14']:.5f}", f"{t['stop_loss']:.5f}",
                f"{t['take_profit']:.5f}", t["exit_time_str"] or "OPEN",
                f"{t['exit_price']:.5f}" if t["exit_price"] else "---",
                t["status"], f"{t['risk_usdt']:.2f}", f"{t['target_reward_usdt']:.2f}",
                f"{t['pnl_usdt']:+.2f}", f"{t['pnl_pct']:+.2f}%", t["holding_bars"]
            ])
    print(f"[BAŞARILI] İşlem dökümü CSV dosyasına yazıldı: {csv_path}")
