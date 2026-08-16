use plugin_paper_exchange::models::{Order, OrderSide, OrderType, PositionSide};
use plugin_paper_exchange::engine::PaperEngine;

#[test]
fn test_commands_working() {
    use std::sync::Arc;
    let storage = Arc::new(plugin_paper_exchange::storage::Storage::new(":memory:").unwrap());
    let mut engine = PaperEngine::new(storage);
    engine.create_account("admin", 10000.0);
    
    // 1. Send Buy Order (Long)
    let order_json = serde_json::json!({
        "id": "test_1",
        "user_id": "admin",
        "symbol": "BTCUSDT",
        "side": "Buy",
        "position_side": "Long",
        "order_type": "Limit",
        "price": 60000.0,
        "stop_price": 0.0,
        "amount": 0.1,
        "leverage": 20.0,
        "executed": 0.0,
        "timestamp": 0
    });
    let order: Order = serde_json::from_value(order_json).unwrap();
    engine.submit_order("admin", order).unwrap();
    
    // Engine should have 1 active order
    assert_eq!(engine.active_orders.get("BTCUSDT").unwrap().len(), 1);
    
    // 2. Trigger execution with last price
    engine.on_last_price_update("BTCUSDT", 59000.0);
    
    // Order should be executed, position opened
    assert_eq!(engine.active_orders.get("BTCUSDT").unwrap().len(), 0);
    {
        let pos = engine.positions.get("admin").unwrap();
        let btc_pos = pos.get("BTCUSDT_Long").unwrap();
        assert_eq!(btc_pos.amount, 0.1);
        assert_eq!(btc_pos.side, PositionSide::Long);
        assert_eq!(btc_pos.leverage, 20.0);
        assert!(btc_pos.liquidation_price > 0.0);
    }
    
    // 3. Send Close Command (emulated by TUI payload)
    // TUI creates a market order in opposite direction
    let close_order = Order {
        id: "test_2".to_string(),
        user_id: "admin".to_string(),
        symbol: "BTCUSDT".to_string(),
        side: OrderSide::Sell,
        position_side: PositionSide::Long,
        order_type: OrderType::Market,
        price: 0.0,
        stop_price: 0.0,
        amount: 0.1, // we know it's 0.1
        leverage: 20.0,
        executed: 0.0,
        timestamp: 0,
    };
    engine.submit_order("admin", close_order).unwrap();
    
    let pos_after = engine.positions.get("admin").unwrap();
    let btc_pos_after = pos_after.get("BTCUSDT_Long").unwrap();
    // Wait, the close order creates a Short position? 
    // Wait, no. If I submit a sell order with position_side Short, it will create a new position "BTCUSDT_Short" with 0.1 amount!
    // Binance futures hedging mode works like this.
    // If it's one-way mode, a Sell order closes the Long position.
    // Let's see how engine.rs handles closing.
}
