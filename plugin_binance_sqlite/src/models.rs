use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarkPriceRecord {
    pub symbol: String,
    pub mark_price: f64,
    pub index_price: f64,
    pub funding_rate: f64,
    pub next_funding_time: i64,
    pub event_time: i64,
    pub local_recv_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BestPriceRecord {
    pub symbol: String,
    pub best_bid: f64,
    pub best_bid_qty: f64,
    pub best_ask: f64,
    pub best_ask_qty: f64,
    pub event_time: i64,
    pub local_recv_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub symbol: String,
    pub trade_id: i64,
    pub price: f64,
    pub quantity: f64,
    pub buyer_is_maker: bool,
    pub event_time: i64,
    pub local_recv_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidationRecord {
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    pub price: f64,
    pub average_price: f64,
    pub original_qty: f64,
    pub filled_qty: f64,
    pub event_time: i64,
    pub local_recv_time_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepthRecord {
    pub symbol: String,
    pub bids_json: String,
    pub asks_json: String,
    pub last_update_id: i64,
    pub event_time: i64,
    pub local_recv_time_ms: i64,
}
