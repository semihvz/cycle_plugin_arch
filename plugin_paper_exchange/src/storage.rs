use rusqlite::{Connection, Result, params};
use crate::models::{Order, Position};
use std::sync::{Arc, Mutex};

pub struct Storage {
    conn: Arc<Mutex<Connection>>,
}

impl Storage {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        
        conn.execute(
            "CREATE TABLE IF NOT EXISTS orders (
                id TEXT PRIMARY KEY,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                position_side TEXT NOT NULL,
                order_type TEXT NOT NULL,
                price REAL NOT NULL,
                amount REAL NOT NULL,
                executed REAL NOT NULL,
                timestamp INTEGER NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS closed_positions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                symbol TEXT NOT NULL,
                side TEXT NOT NULL,
                amount REAL NOT NULL,
                entry_price REAL NOT NULL,
                close_price REAL NOT NULL,
                realized_pnl REAL NOT NULL,
                timestamp INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert_order(&self, order: &Order) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO orders (id, symbol, side, position_side, order_type, price, amount, executed, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                order.id,
                order.symbol,
                format!("{:?}", order.side),
                format!("{:?}", order.position_side),
                format!("{:?}", order.order_type),
                order.price,
                order.amount,
                order.executed,
                order.timestamp
            ],
        )?;
        Ok(())
    }

    pub fn insert_closed_position(
        &self, 
        symbol: &str, 
        side: &str, 
        amount: f64, 
        entry_price: f64, 
        close_price: f64, 
        realized_pnl: f64, 
        timestamp: i64
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO closed_positions (symbol, side, amount, entry_price, close_price, realized_pnl, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                symbol,
                side,
                amount,
                entry_price,
                close_price,
                realized_pnl,
                timestamp
            ],
        )?;
        Ok(())
    }
}
