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

    pub fn deposit(&self, user_id: &str, amount: f64) -> Result<f64, String> {
        let mut account = self.accounts.entry(user_id.to_string()).or_insert_with(|| {
            self.positions.entry(user_id.to_string()).or_insert_with(DashMap::new);
            Account::new(0.0)
        });
        account.wallet_balance += amount;
        account.margin_balance += amount;
        self.log_msg(format!("DEPOSIT: {} USDT credited to user {}", amount, user_id));
        Ok(account.wallet_balance)
    }

    pub fn set_balance(&self, user_id: &str, amount: f64) -> Result<f64, String> {
        let mut account = self.accounts.entry(user_id.to_string()).or_insert_with(|| {
            self.positions.entry(user_id.to_string()).or_insert_with(DashMap::new);
            Account::new(amount)
        });
        account.wallet_balance = amount;
        account.margin_balance = amount;
        self.log_msg(format!("SET_BALANCE: User {} wallet balance set to {} USDT", user_id, amount));
        Ok(account.wallet_balance)
    }

    pub fn cancel_order(&self, order_id: &str) -> bool {
        let mut removed = false;
        for mut entry in self.active_orders.iter_mut() {
            let symbol_orders = entry.value_mut();
            let orig_len = symbol_orders.len();
            symbol_orders.retain(|o| o.id != order_id);
            if symbol_orders.len() < orig_len {
                removed = true;
                self.log_msg(format!("CANCEL_ORDER: Order {} cancelled", order_id));
                break;
            }
        }
        removed
    }

    pub fn cancel_all_orders(&self, symbol_opt: Option<&str>) -> usize {
        let mut count = 0;
        if let Some(symbol) = symbol_opt {
            if let Some(mut symbol_orders) = self.active_orders.get_mut(symbol) {
                count = symbol_orders.len();
                symbol_orders.clear();
                self.log_msg(format!("CANCEL_ALL: Cleared {} orders for {}", count, symbol));
            }
        } else {
            for mut entry in self.active_orders.iter_mut() {
                count += entry.value().len();
                entry.value_mut().clear();
            }
            self.log_msg(format!("CANCEL_ALL: Cleared all {} orders across symbols", count));
        }
        count
    }

    pub fn close_all_positions(&self, user_id: &str) -> Result<usize, String> {
        let mut to_submit = Vec::new();
        if let Some(user_pos) = self.positions.get(user_id) {
            for pos_ref in user_pos.iter() {
                let pos = pos_ref.value();
                if pos.amount > 0.0 {
                    let rev_side = if pos.side == PositionSide::Long { OrderSide::Sell } else { OrderSide::Buy };
                    let rev_pos = pos.side.clone();
                    to_submit.push(Order {
                        id: format!("closeall_{}", std::time::SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()),
                        user_id: user_id.to_string(),
                        symbol: pos.symbol.clone(),
                        side: rev_side,
                        position_side: rev_pos,
                        order_type: OrderType::Market,
                        price: 0.0,
                        stop_price: 0.0,
                        amount: pos.amount,
                        leverage: pos.leverage,
                        executed: 0.0,
                        timestamp: 0,
                    });
                }
            }
        }

        let count = to_submit.len();
        for order in to_submit {
            let _ = self.submit_order(user_id, order);
        }
        self.log_msg(format!("CLOSE_ALL: Closed {} positions for {}", count, user_id));
        Ok(count)
    }

    pub fn submit_order(&self, user_id: &str, mut order: Order) -> Result<(), String> {
        let wallet_balance = self.accounts.get(user_id).map(|a| a.wallet_balance).ok_or("Account not found")?;
        
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64;
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

        // Margin check
        let cost = (order.amount * order.price) / order.leverage;
        if wallet_balance < cost && order.order_type == OrderType::Market && cost > 0.0 {
            // Optional warning or error
        }

        if order.order_type == OrderType::Market {
            if current_last_price > 0.0 {
                order.price = current_last_price;
                order.executed = order.amount;
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
        
        // Update PnL of active positions on last price if mark price isn't set yet
        if !self.mark_prices.contains_key(symbol) {
            for user_ref in self.positions.iter() {
                for mut pos in user_ref.value().iter_mut() {
                    if pos.symbol == symbol {
                        pos.update_pnl(last_price);
                    }
                }
            }
        }

        // Match pending orders
        if let Some(mut symbol_orders) = self.active_orders.get_mut(symbol) {
            let mut executed_orders = Vec::new();

            symbol_orders.retain_mut(|order| {
                let mut should_execute = false;
                
                match order.order_type {
                    OrderType::Limit => {
                        should_execute = match order.side {
                            OrderSide::Buy => last_price <= order.price,
                            OrderSide::Sell => last_price >= order.price,
                        };
                    }
                    OrderType::StopMarket | OrderType::StopLimit => {
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
                    OrderType::TrailingStop => {
                        // Dynamically update trailing stop price if distance is specified in order.price
                        let offset = order.price;
                        if offset > 0.0 {
                            match order.side {
                                OrderSide::Sell => {
                                    if order.stop_price == 0.0 || last_price - offset > order.stop_price {
                                        order.stop_price = last_price - offset;
                                    }
                                    should_execute = last_price <= order.stop_price;
                                }
                                OrderSide::Buy => {
                                    if order.stop_price == 0.0 || last_price + offset < order.stop_price {
                                        order.stop_price = last_price + offset;
                                    }
                                    should_execute = last_price >= order.stop_price;
                                }
                            }
                        } else {
                            should_execute = match order.side {
                                OrderSide::Buy => last_price >= order.stop_price,
                                OrderSide::Sell => last_price <= order.stop_price,
                            };
                        }
                    }
                    _ => {}
                }

                if should_execute {
                    order.executed = order.amount;
                    executed_orders.push(order.clone());
                    false // remove from active
                } else {
                    true // keep
                }
            });
            
            // Execute the triggered orders without holding the dashmap lock
            for executed_order in executed_orders {
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
        
        let user_positions = self.positions.get(&order.user_id).ok_or("User positions map not found")?;
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
            if position.amount > 0.0 {
                let closed_amount = order.amount.min(position.amount);
                
                // Calculate Realized PnL
                let realized_pnl = match position.side {
                    PositionSide::Long => (exec_price - position.entry_price) * closed_amount,
                    PositionSide::Short => (position.entry_price - exec_price) * closed_amount,
                };

                position.amount -= closed_amount;

                // Credit/Debit wallet_balance and margin_balance
                if let Some(mut account) = self.accounts.get_mut(&order.user_id) {
                    account.wallet_balance += realized_pnl;
                    account.margin_balance += realized_pnl;
                }

                // Save closed position record
                let side_str = format!("{:?}", position.side);
                let _ = self.storage.insert_closed_position(
                    &position.symbol,
                    &side_str,
                    closed_amount,
                    position.entry_price,
                    exec_price,
                    realized_pnl,
                    order.timestamp,
                );

                if position.amount <= 0.000001 {
                    position.amount = 0.0;
                    position.entry_price = 0.0;
                    position.liquidation_price = 0.0;
                    position.unrealized_pnl = 0.0;
                }
            }
        }

        if position.amount > 0.0 {
            let maintenance_margin = 0.005; // 0.5%
            match position.side {
                PositionSide::Long => {
                    position.liquidation_price = position.entry_price * (1.0 - (1.0 / position.leverage) + maintenance_margin);
                }
                PositionSide::Short => {
                    position.liquidation_price = position.entry_price * (1.0 + (1.0 / position.leverage) - maintenance_margin);
                }
            }
            let mark = self.mark_prices.get(&order.symbol).map(|v| *v).unwrap_or(exec_price);
            position.update_pnl(mark);
        }

        Ok(())
    }

    pub fn on_mark_price_update(&self, symbol: &str, mark_price: f64) {
        self.mark_prices.insert(symbol.to_string(), mark_price);
        
        // Update PnL and process liquidations for each user independently
        for user_ref in self.positions.iter() {
            let user_id = user_ref.key();
            let user_positions = user_ref.value();
            
            let mut total_upnl = 0.0;
            let mut to_liquidate = Vec::new();
            let mut user_liquidated_loss = 0.0;

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
                            user_liquidated_loss += loss;
                            self.log_msg(format!("LIQUIDATED! {} {} position closed at Mark Price: {}", user_id, symbol, mark_price));
                        }
                    }
                }
                
                if !to_liquidate.contains(pos_ref.key()) {
                    total_upnl += pos_ref.unrealized_pnl;
                }
            }
            
            for key in &to_liquidate {
                if let Some(mut p) = user_positions.get_mut(key) {
                    let side_str = format!("{:?}", p.side);
                    let loss_pnl = -((p.amount * p.entry_price) / p.leverage);
                    let _ = self.storage.insert_closed_position(
                        &p.symbol,
                        &side_str,
                        p.amount,
                        p.entry_price,
                        mark_price,
                        loss_pnl,
                        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64,
                    );
                    p.amount = 0.0;
                    p.unrealized_pnl = 0.0;
                    p.entry_price = 0.0;
                    p.liquidation_price = 0.0;
                }
            }

            // Update Account Balance per user
            if let Some(mut account) = self.accounts.get_mut(user_id) {
                if user_liquidated_loss > 0.0 {
                    account.wallet_balance -= user_liquidated_loss;
                }
                account.margin_balance = account.wallet_balance + total_upnl;
            }
        }
    }
}

