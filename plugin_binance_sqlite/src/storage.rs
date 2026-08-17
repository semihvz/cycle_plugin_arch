use rusqlite::{params, Connection, Result};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use crate::models::*;

pub struct StorageStats {
    pub mark_price_count: AtomicU64,
    pub best_price_count: AtomicU64,
    pub trade_count: AtomicU64,
    pub liquidation_count: AtomicU64,
    pub depth_count: AtomicU64,
    pub last_insert_time_ms: AtomicU64,
}

impl Default for StorageStats {
    fn default() -> Self {
        Self {
            mark_price_count: AtomicU64::new(0),
            best_price_count: AtomicU64::new(0),
            trade_count: AtomicU64::new(0),
            liquidation_count: AtomicU64::new(0),
            depth_count: AtomicU64::new(0),
            last_insert_time_ms: AtomicU64::new(0),
        }
    }
}

pub struct SqliteStorage {
    conn: Arc<Mutex<Connection>>,
    pub stats: Arc<StorageStats>,
    pub db_path: String,
}

impl SqliteStorage {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;

        // Performance tuning for real-time writes
        let _ = conn.execute_batch("
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA temp_store = MEMORY;
        ");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS mark_prices (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                mark_price REAL NOT NULL,
                index_price REAL NOT NULL,
                funding_rate REAL NOT NULL,
                next_funding_time INTEGER NOT NULL,
                event_time INTEGER NOT NULL,
                local_recv_time_ms INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS best_prices (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                best_bid REAL NOT NULL,
                best_bid_qty REAL NOT NULL,
                best_ask REAL NOT NULL,
                best_ask_qty REAL NOT NULL,
                event_time INTEGER NOT NULL,
                local_recv_time_ms INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS trades (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                trade_id INTEGER NOT NULL,
                price REAL NOT NULL,
                quantity REAL NOT NULL,
                buyer_is_maker INTEGER NOT NULL,
                event_time INTEGER NOT NULL,
                local_recv_time_ms INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS liquidations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                order_type TEXT NOT NULL,
                price REAL NOT NULL,
                average_price REAL NOT NULL,
                original_qty REAL NOT NULL,
                filled_qty REAL NOT NULL,
                event_time INTEGER NOT NULL,
                local_recv_time_ms INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS depth (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                bids_json TEXT NOT NULL,
                asks_json TEXT NOT NULL,
                last_update_id INTEGER NOT NULL,
                event_time INTEGER NOT NULL,
                local_recv_time_ms INTEGER NOT NULL
            )",
            [],
        )?;

        // Create indexes for fast queries
        conn.execute("CREATE INDEX IF NOT EXISTS idx_mark_prices_symbol_time ON mark_prices (symbol, local_recv_time_ms)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_best_prices_symbol_time ON best_prices (symbol, local_recv_time_ms)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_trades_symbol_time ON trades (symbol, local_recv_time_ms)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_liquidations_symbol_time ON liquidations (symbol, local_recv_time_ms)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_depth_symbol_time ON depth (symbol, local_recv_time_ms)", [])?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            stats: Arc::new(StorageStats::default()),
            db_path: db_path.to_string(),
        })
    }

    fn update_timestamp(&self, recv_ms: i64) {
        if recv_ms > 0 {
            self.stats.last_insert_time_ms.store(recv_ms as u64, Ordering::Relaxed);
        }
    }

    pub fn insert_mark_price(&self, rec: &MarkPriceRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO mark_prices (symbol, mark_price, index_price, funding_rate, next_funding_time, event_time, local_recv_time_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                rec.symbol,
                rec.mark_price,
                rec.index_price,
                rec.funding_rate,
                rec.next_funding_time,
                rec.event_time,
                rec.local_recv_time_ms
            ],
        )?;
        self.stats.mark_price_count.fetch_add(1, Ordering::Relaxed);
        self.update_timestamp(rec.local_recv_time_ms);
        Ok(())
    }

    pub fn insert_best_price(&self, rec: &BestPriceRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO best_prices (symbol, best_bid, best_bid_qty, best_ask, best_ask_qty, event_time, local_recv_time_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                rec.symbol,
                rec.best_bid,
                rec.best_bid_qty,
                rec.best_ask,
                rec.best_ask_qty,
                rec.event_time,
                rec.local_recv_time_ms
            ],
        )?;
        self.stats.best_price_count.fetch_add(1, Ordering::Relaxed);
        self.update_timestamp(rec.local_recv_time_ms);
        Ok(())
    }

    pub fn insert_trade(&self, rec: &TradeRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO trades (symbol, trade_id, price, quantity, buyer_is_maker, event_time, local_recv_time_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                rec.symbol,
                rec.trade_id,
                rec.price,
                rec.quantity,
                if rec.buyer_is_maker { 1 } else { 0 },
                rec.event_time,
                rec.local_recv_time_ms
            ],
        )?;
        self.stats.trade_count.fetch_add(1, Ordering::Relaxed);
        self.update_timestamp(rec.local_recv_time_ms);
        Ok(())
    }

    pub fn insert_liquidation(&self, rec: &LiquidationRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO liquidations (symbol, side, order_type, price, average_price, original_qty, filled_qty, event_time, local_recv_time_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                rec.symbol,
                rec.side,
                rec.order_type,
                rec.price,
                rec.average_price,
                rec.original_qty,
                rec.filled_qty,
                rec.event_time,
                rec.local_recv_time_ms
            ],
        )?;
        self.stats.liquidation_count.fetch_add(1, Ordering::Relaxed);
        self.update_timestamp(rec.local_recv_time_ms);
        Ok(())
    }

    pub fn insert_depth(&self, rec: &DepthRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO depth (symbol, bids_json, asks_json, last_update_id, event_time, local_recv_time_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                rec.symbol,
                rec.bids_json,
                rec.asks_json,
                rec.last_update_id,
                rec.event_time,
                rec.local_recv_time_ms
            ],
        )?;
        self.stats.depth_count.fetch_add(1, Ordering::Relaxed);
        self.update_timestamp(rec.local_recv_time_ms);
        Ok(())
    }

    pub fn get_file_size_bytes(&self) -> u64 {
        std::fs::metadata(&self.db_path).map(|m| m.len()).unwrap_or(0)
    }
}
