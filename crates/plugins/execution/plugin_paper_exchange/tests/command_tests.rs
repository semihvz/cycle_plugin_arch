#[cfg(test)]
mod command_tests {
    use std::sync::Arc;
    use plugin_paper_exchange::*;

    fn make_engine() -> Arc<PaperEngine> {
        let storage = Storage::new(":memory:").unwrap();
        let engine = PaperEngine::new(Arc::new(storage));
        engine.create_account("admin", 10000.0);
        Arc::new(engine)
    }

    #[test]
    fn test_deposit_and_set_balance() {
        let engine = make_engine();
        assert_eq!(engine.accounts.get("admin").unwrap().wallet_balance, 10000.0);

        let new_bal = engine.deposit("admin", 2500.0).unwrap();
        assert_eq!(new_bal, 12500.0);
        assert_eq!(engine.accounts.get("admin").unwrap().wallet_balance, 12500.0);

        let reset_bal = engine.set_balance("admin", 5000.0).unwrap();
        assert_eq!(reset_bal, 5000.0);
        assert_eq!(engine.accounts.get("admin").unwrap().wallet_balance, 5000.0);
    }

    #[test]
    fn test_cancel_order_and_cancel_all() {
        let engine = make_engine();
        engine.latest_prices.insert("BTCUSDT".into(), 60000.0);
        engine.latest_prices.insert("ETHUSDT".into(), 3000.0);

        let o1 = Order {
            id: "ord_1".into(),
            user_id: "admin".into(),
            symbol: "BTCUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Limit,
            price: 55000.0,
            stop_price: 0.0,
            amount: 0.5,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };

        let o2 = Order {
            id: "ord_2".into(),
            user_id: "admin".into(),
            symbol: "ETHUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Short,
            order_type: OrderType::Limit,
            price: 3500.0,
            stop_price: 0.0,
            amount: 2.0,
            leverage: 20.0,
            executed: 0.0,
            timestamp: 0,
        };

        engine.submit_order("admin", o1).unwrap();
        engine.submit_order("admin", o2).unwrap();

        assert_eq!(engine.active_orders.get("BTCUSDT").unwrap().len(), 1);
        assert_eq!(engine.active_orders.get("ETHUSDT").unwrap().len(), 1);

        // Cancel specific order
        let cancelled = engine.cancel_order("ord_1");
        assert!(cancelled);
        assert_eq!(engine.active_orders.get("BTCUSDT").unwrap().len(), 0);

        // Cancel all remaining orders
        let count = engine.cancel_all_orders(None);
        assert_eq!(count, 1);
        assert_eq!(engine.active_orders.get("ETHUSDT").unwrap().len(), 0);
    }

    #[test]
    fn test_close_all_positions() {
        let engine = make_engine();
        engine.latest_prices.insert("BTCUSDT".into(), 60000.0);
        engine.latest_prices.insert("ETHUSDT".into(), 3000.0);

        let o1 = Order {
            id: "m1".into(),
            user_id: "admin".into(),
            symbol: "BTCUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 0.1,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        let o2 = Order {
            id: "m2".into(),
            user_id: "admin".into(),
            symbol: "ETHUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Short,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 1.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };

        engine.submit_order("admin", o1).unwrap();
        engine.submit_order("admin", o2).unwrap();

        let pos = engine.positions.get("admin").unwrap();
        assert_eq!(pos.get("BTCUSDT_Long").unwrap().amount, 0.1);
        assert_eq!(pos.get("ETHUSDT_Short").unwrap().amount, 1.0);
        drop(pos);

        // Close all positions
        let closed_count = engine.close_all_positions("admin").unwrap();
        assert_eq!(closed_count, 2);

        let pos = engine.positions.get("admin").unwrap();
        assert_eq!(pos.get("BTCUSDT_Long").unwrap().amount, 0.0);
        assert_eq!(pos.get("ETHUSDT_Short").unwrap().amount, 0.0);
    }
}
