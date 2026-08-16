use serde::{Deserialize, Serialize};

fn default_leverage() -> f64 { 20.0 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum PositionSide {
    Long,
    Short,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Copy)]
pub enum OrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
    TakeProfitMarket,
    TakeProfitLimit,
    TrailingStop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    pub id: String,
    pub user_id: String,
    pub symbol: String,
    pub side: OrderSide,
    pub position_side: PositionSide, // Hedge mode requires this
    pub order_type: OrderType,
    pub price: f64,
    pub stop_price: f64, // Used for stop and take profit orders
    pub amount: f64,
    #[serde(default = "default_leverage")]
    pub leverage: f64,
    pub executed: f64,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub symbol: String,
    pub side: PositionSide,
    pub amount: f64,
    pub entry_price: f64,
    pub leverage: f64,
    pub liquidation_price: f64,
    pub unrealized_pnl: f64,
}

impl Position {
    pub fn new(symbol: String, side: PositionSide, leverage: f64) -> Self {
        Self {
            symbol,
            side,
            amount: 0.0,
            entry_price: 0.0,
            leverage,
            liquidation_price: 0.0,
            unrealized_pnl: 0.0,
        }
    }

    pub fn update_pnl(&mut self, mark_price: f64) {
        if self.amount == 0.0 {
            self.unrealized_pnl = 0.0;
            return;
        }

        match self.side {
            PositionSide::Long => {
                self.unrealized_pnl = (mark_price - self.entry_price) * self.amount;
            }
            PositionSide::Short => {
                self.unrealized_pnl = (self.entry_price - mark_price) * self.amount;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    pub wallet_balance: f64,
    pub margin_balance: f64,
}

impl Account {
    pub fn new(initial_balance: f64) -> Self {
        Self {
            wallet_balance: initial_balance,
            margin_balance: initial_balance,
        }
    }
}
