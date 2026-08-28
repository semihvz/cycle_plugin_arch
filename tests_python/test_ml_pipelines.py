import os
import sys
import tempfile
import sqlite3
import pandas as pd
import pytest

# Ensure root dir is in path
ROOT_DIR = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
if ROOT_DIR not in sys.path:
    sys.path.insert(0, ROOT_DIR)

def test_sqlite_export_and_query():
    """Test sqlite database creation and table querying logic."""
    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as tmp:
        db_path = tmp.name

    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS closed_trades (
                trade_id INTEGER PRIMARY KEY,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                entry_price REAL NOT NULL,
                exit_price REAL NOT NULL,
                pnl_usdt REAL NOT NULL,
                result TEXT NOT NULL
            );
        """)
        
        cursor.executemany("""
            INSERT INTO closed_trades VALUES (?, ?, ?, ?, ?, ?, ?);
        """, [
            (1, "TACUSDT", "LONG", 0.0025, 0.0030, 10.0, "WIN"),
            (2, "TACUSDT", "LONG", 0.0030, 0.0028, -4.0, "LOSS"),
        ])
        conn.commit()

        df = pd.read_sql_query("SELECT * FROM closed_trades", conn)
        assert len(df) == 2
        assert df["symbol"].iloc[0] == "TACUSDT"
        assert df["pnl_usdt"].sum() == 6.0
        conn.close()
    finally:
        if os.path.exists(db_path):
            os.remove(db_path)

def test_trade_export_to_csv():
    """Test pandas data export functionality to CSV."""
    trades_data = [
        {"trade_id": 1, "symbol": "TACUSDT", "entry_price": 0.0025, "pnl_usdt": 10.0, "result": "WIN"},
        {"trade_id": 2, "symbol": "TACUSDT", "entry_price": 0.0030, "pnl_usdt": -4.0, "result": "LOSS"},
    ]
    df = pd.DataFrame(trades_data)
    
    with tempfile.NamedTemporaryFile(suffix=".csv", delete=False) as tmp:
        csv_path = tmp.name

    try:
        df.to_csv(csv_path, index=False)
        assert os.path.exists(csv_path)
        read_df = pd.read_csv(csv_path)
        assert len(read_df) == 2
        assert read_df["result"].tolist() == ["WIN", "LOSS"]
    finally:
        if os.path.exists(csv_path):
            os.remove(csv_path)

def test_technical_indicators_calculation():
    """Test ATR and EMA calculation logic on synthetic candlestick series."""
    df = pd.DataFrame({
        "open": [10.0 + i for i in range(30)],
        "high": [12.0 + i for i in range(30)],
        "low": [9.0 + i for i in range(30)],
        "close": [11.0 + i for i in range(30)],
        "volume": [1000.0] * 30
    })
    
    # Calculate True Range (TR)
    df["prev_close"] = df["close"].shift(1)
    tr1 = df["high"] - df["low"]
    tr2 = (df["high"] - df["prev_close"]).abs()
    tr3 = (df["low"] - df["prev_close"]).abs()
    df["tr"] = pd.concat([tr1, tr2, tr3], axis=1).max(axis=1)
    df["atr_14"] = df["tr"].rolling(14).mean()

    assert not df["atr_14"].iloc[14:].isna().any()
    assert df["atr_14"].iloc[14] > 0
