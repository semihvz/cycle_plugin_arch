use std::sync::Arc;
use dashmap::DashMap;
use crate::models::{Account, Order, Position, OrderType, OrderSide, PositionSide};
use crate::storage::Storage;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct PaperEngine {
    pub accounts: DashMap<String, Account>,
    pub positions: DashMap<String, DashMap<String, Position>>, // user_id -> symbol_side -> Position
    pub active_orders: DashMap<String, Vec<Order>>, // symbol -> Orders
    pub latest_prices: DashMap<String, f64>,
    pub mark_prices: DashMap<String, f64>,
    pub storage: Arc<Storage>,
    pub system_messages: std::sync::Mutex<std::collections::VecDeque<String>>,
}

impl PaperEngine {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            accounts: DashMap::new(),
            positions: DashMap::new(),
            active_orders: DashMap::new(),
            latest_prices: DashMap::new(),
            mark_prices: DashMap::new(),
            storage,
            system_messages: std::sync::Mutex::new(std::collections::VecDeque::new()),
        }
    }

    pub fn log_msg(&self, msg: String) {
        if let Ok(mut msgs) = self.system_messages.lock() {
            msgs.push_back(format!("{} - {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs(), msg));
            if msgs.len() > 10 {
                msgs.pop_front();
            }
        }
    }

    pub fn create_account(&self, user_id: &str, initial_balance: f64) {
        self.accounts.insert(user_id.to_string(), Account::new(initial_balance));
        self.positions.insert(user_id.to_string(), DashMap::new());
    }

    pub fn submit_order(&self, user_id: &str, mut order: Order) -> Result<(), String> {
        let account = self.accounts.get(user_id).ok_or("Account not found")?;
        
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as i64;
        order.timestamp = now;
        order.user_id = user_id.to_string();

        let mut current_last_price = self.latest_prices.get(&order.symbol).map(|v| *v).unwrap_or(0.0);

        // HACK for testing: If no price feed is active but user manually provided a price in the form for a Market order,
        // we use that price as a simulated market price to allow testing the system.
        if current_last_price == 0.0 && order.price > 0.0 {
            current_last_price = order.price;
            self.latest_prices.insert(order.symbol.clone(), current_last_price);
            self.log_msg(format!("TEST MODU: Manuel girilen fiyat ({}) piyasa fiyatı kabul edildi.", current_last_price));
        }

        // Check margin for order (simplified)
        let cost = (order.amount * order.price) / order.leverage;
        if account.wallet_balance < cost {
            // return Err("Insufficient margin".into()); // Disabled strict check for paper simplicity
        }

        if order.order_type == OrderType::Market {
            if current_last_price > 0.0 {
                order.price = current_last_price;
                self.execute_order(&order, current_last_price)?;
                let _ = self.storage.insert_order(&order);
                self.log_msg(format!("Market order executed for {} at {}", order.symbol, current_last_price));
                return Ok(());
            } else {
                let err_msg = format!("No market price available to execute Market order for {}", order.symbol);
                self.log_msg(err_msg.clone());
                return Err(err_msg);
            }
        } else {
            // Limit and Stop orders
            let mut symbol_orders = self.active_orders.entry(order.symbol.clone()).or_insert_with(Vec::new);
            symbol_orders.push(order.clone());
            self.log_msg(format!("Pending order added for {}: {:?}", order.symbol, order.order_type));
        }

        self.storage.insert_order(&order).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn on_last_price_update(&self, symbol: &str, last_price: f64) {
        self.latest_prices.insert(symbol.to_string(), last_price);
        
        // Match pending orders
        if let Some(mut symbol_orders) = self.active_orders.get_mut(symbol) {
            let mut executed_orders = Vec::new();

            symbol_orders.retain(|order| {
                let mut should_execute = false;
                
                match order.order_type {
                    OrderType::Limit => {
                        should_execute = match order.side {
                            OrderSide::Buy => last_price <= order.price,
                            OrderSide::Sell => last_price >= order.price,
                        };
                    }
                    OrderType::StopMarket | OrderType::StopLimit => {
                        // Simplified trigger logic
                        should_execute = match order.side {
                            OrderSide::Buy => last_price >= order.stop_price,
                            OrderSide::Sell => last_price <= order.stop_price,
                        };
                    }
                    OrderType::TakeProfitMarket | OrderType::TakeProfitLimit => {
                        should_execute = match order.side {
                            OrderSide::Buy => last_price <= order.stop_price,
                            OrderSide::Sell => last_price >= order.stop_price,
                        };
                    }
                    _ => {}
                }

                if should_execute {
                    executed_orders.push(order.clone());
                    false // retain = false -> remove from active
                } else {
                    true // retain = true -> keep
                }
            });
            
            // Execute the triggered orders without holding the dashmap lock
            for mut executed_order in executed_orders {
                let exec_price = if executed_order.order_type == OrderType::Limit || executed_order.order_type == OrderType::StopLimit || executed_order.order_type == OrderType::TakeProfitLimit {
                    executed_order.price
                } else {
                    last_price // Market execution
                };
                
                let _ = self.execute_order(&executed_order, exec_price);
                let _ = self.storage.insert_order(&executed_order);
            }
        }
    }

    fn execute_order(&self, order: &Order, exec_price: f64) -> Result<(), String> {
        let pos_key = format!("{}_{:?}", order.symbol, order.position_side);
        
        let user_positions = self.positions.get(&order.user_id).unwrap();
        let mut position = user_positions.entry(pos_key).or_insert_with(|| {
            Position::new(order.symbol.clone(), order.position_side, order.leverage)
        });

        let is_increase = match (order.position_side, order.side) {
            (PositionSide::Long, OrderSide::Buy) => true,
            (PositionSide::Long, OrderSide::Sell) => false,
            (PositionSide::Short, OrderSide::Sell) => true,
            (PositionSide::Short, OrderSide::Buy) => false,
        };

        if is_increase {
            if position.amount > 0.0 {
                let total_cost = (position.amount * position.entry_price) + (order.amount * exec_price);
                position.amount += order.amount;
                position.entry_price = total_cost / position.amount;
                position.leverage = order.leverage;
            } else {
                position.amount = order.amount;
                position.entry_price = exec_price;
                position.leverage = order.leverage;
            }
        } else {
            // Decrease position (close/reduce)
            position.amount -= order.amount;
            if position.amount <= 0.000001 { // Handle floating point issues
                position.amount = 0.0;
                position.entry_price = 0.0;
                // Realized PNL would be calculated here in a full engine
            }
        }

        let maintenance_margin = 0.005; // 0.5%
        match position.side {
            PositionSide::Long => {
                position.liquidation_price = position.entry_price * (1.0 - (1.0 / position.leverage) + maintenance_margin);
            }
            PositionSide::Short => {
                position.liquidation_price = position.entry_price * (1.0 + (1.0 / position.leverage) - maintenance_margin);
            }
        }

        Ok(())
    }

    pub fn on_mark_price_update(&self, symbol: &str, mark_price: f64) {
        self.mark_prices.insert(symbol.to_string(), mark_price);
        
        let mut liquidated_loss = 0.0;
        let mut liquidated_user = "".to_string();

        // Update PnL for all positions with this symbol
        for user_ref in self.positions.iter() {
            let user_id = user_ref.key();
            let user_positions = user_ref.value();
            
            let mut total_upnl = 0.0;
            let mut to_liquidate = Vec::new();

            for mut pos_ref in user_positions.iter_mut() {
                if pos_ref.symbol == symbol {
                    pos_ref.update_pnl(mark_price);
                    
                    if pos_ref.amount > 0.0 {
                        let is_liquidated = match pos_ref.side {
                            PositionSide::Long => mark_price <= pos_ref.liquidation_price,
                            PositionSide::Short => mark_price >= pos_ref.liquidation_price,
                        };
                        
                        if is_liquidated {
                            to_liquidate.push(pos_ref.key().clone());
                            let loss = (pos_ref.amount * pos_ref.entry_price) / pos_ref.leverage;
                            liquidated_loss += loss;
                            liquidated_user = user_id.clone();
                            self.log_msg(format!("LIQUIDATED! {} {} position closed at Mark Price: {}", user_id, symbol, mark_price));
                        }
                    }
                }
                
                if !to_liquidate.contains(pos_ref.key()) {
                    total_upnl += pos_ref.unrealized_pnl;
                }
            }
            
            for key in to_liquidate {
                let mut p = user_positions.get_mut(&key).unwrap();
                p.amount = 0.0;
                p.unrealized_pnl = 0.0;
                p.entry_price = 0.0;
                p.liquidation_price = 0.0;
            }

            // Update Account Margin Balance
            if let Some(mut account) = self.accounts.get_mut(user_id) {
                if liquidated_loss > 0.0 && liquidated_user == *user_id {
                    account.wallet_balance -= liquidated_loss;
                    liquidated_loss = 0.0;
                }
                account.margin_balance = account.wallet_balance + total_upnl;
            }
        }
    }
}
