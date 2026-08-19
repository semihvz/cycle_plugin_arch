#[cfg(test)]
mod all_scenarios_tests {
    use std::sync::Arc;
    use plugin_paper_exchange::*;

    fn make_engine() -> Arc<PaperEngine> {
        let storage = Storage::new(":memory:").unwrap();
        let engine = PaperEngine::new(Arc::new(storage));
        engine.create_account("trader1", 10000.0);
        Arc::new(engine)
    }

    // -------------------------------------------------------------
    // 1. Market Orders (Long & Short)
    // -------------------------------------------------------------
    #[test]
    fn test_market_buy_long_and_market_sell_short() {
        let engine = make_engine();
        engine.latest_prices.insert("BTCUSDT".into(), 50000.0);

        // Long Market Order
        let long_order = Order {
            id: "m_long".into(),
            user_id: "trader1".into(),
            symbol: "BTCUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 0.5,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("trader1", long_order).unwrap();

        let pos = engine.positions.get("trader1").unwrap();
        let long_pos = pos.get("BTCUSDT_Long").unwrap();
        assert_eq!(long_pos.amount, 0.5);
        assert_eq!(long_pos.entry_price, 50000.0);
        assert_eq!(long_pos.leverage, 10.0);
        drop(long_pos);
        drop(pos);

        // Short Market Order
        let short_order = Order {
            id: "m_short".into(),
            user_id: "trader1".into(),
            symbol: "BTCUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Short,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 1.0,
            leverage: 20.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("trader1", short_order).unwrap();

        let pos = engine.positions.get("trader1").unwrap();
        let short_pos = pos.get("BTCUSDT_Short").unwrap();
        assert_eq!(short_pos.amount, 1.0);
        assert_eq!(short_pos.entry_price, 50000.0);
        assert_eq!(short_pos.leverage, 20.0);
    }

    // -------------------------------------------------------------
    // 2. Limit Orders Matching Logic
    // -------------------------------------------------------------
    #[test]
    fn test_limit_orders_buy_and_sell() {
        let engine = make_engine();
        engine.latest_prices.insert("ETHUSDT".into(), 3000.0);

        // Buy Limit @ 2900 (should stay active when price is 3000)
        let buy_limit = Order {
            id: "l_buy".into(),
            user_id: "trader1".into(),
            symbol: "ETHUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Limit,
            price: 2900.0,
            stop_price: 0.0,
            amount: 2.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("trader1", buy_limit).unwrap();

        assert_eq!(engine.active_orders.get("ETHUSDT").unwrap().len(), 1);

        // Price updates to 2950 (not triggered yet)
        engine.on_last_price_update("ETHUSDT", 2950.0);
        assert_eq!(engine.active_orders.get("ETHUSDT").unwrap().len(), 1);

        // Price drops to 2890 (triggered!)
        engine.on_last_price_update("ETHUSDT", 2890.0);
        assert_eq!(engine.active_orders.get("ETHUSDT").unwrap().len(), 0);

        let pos = engine.positions.get("trader1").unwrap();
        let p = pos.get("ETHUSDT_Long").unwrap();
        assert_eq!(p.amount, 2.0);
        assert_eq!(p.entry_price, 2900.0);
    }

    // -------------------------------------------------------------
    // 3. Stop Market & Stop Limit Orders
    // -------------------------------------------------------------
    #[test]
    fn test_stop_orders() {
        let engine = make_engine();
        engine.latest_prices.insert("SOLUSDT".into(), 100.0);

        // Stop Market Buy @ Stop Price 105
        let stop_market = Order {
            id: "sm_1".into(),
            user_id: "trader1".into(),
            symbol: "SOLUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::StopMarket,
            price: 0.0,
            stop_price: 105.0,
            amount: 10.0,
            leverage: 5.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("trader1", stop_market).unwrap();

        // Price 104 -> not triggered
        engine.on_last_price_update("SOLUSDT", 104.0);
        assert_eq!(engine.active_orders.get("SOLUSDT").unwrap().len(), 1);

        // Price 106 -> triggered at market price 106
        engine.on_last_price_update("SOLUSDT", 106.0);
        assert_eq!(engine.active_orders.get("SOLUSDT").unwrap().len(), 0);

        let pos = engine.positions.get("trader1").unwrap();
        let p = pos.get("SOLUSDT_Long").unwrap();
        assert_eq!(p.amount, 10.0);
        assert_eq!(p.entry_price, 106.0);
    }

    // -------------------------------------------------------------
    // 4. Trailing Stop Orders
    // -------------------------------------------------------------
    #[test]
    fn test_trailing_stop_orders() {
        let engine = make_engine();
        engine.latest_prices.insert("BTCUSDT".into(), 60000.0);

        // Open Long position first
        let open_long = Order {
            id: "open_t".into(),
            user_id: "trader1".into(),
            symbol: "BTCUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 1.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("trader1", open_long).unwrap();

        // Trailing stop Sell with 1000 USDT trailing offset (price field = 1000)
        let trailing_order = Order {
            id: "ts_1".into(),
            user_id: "trader1".into(),
            symbol: "BTCUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Long,
            order_type: OrderType::TrailingStop,
            price: 1000.0, // offset
            stop_price: 0.0,
            amount: 1.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("trader1", trailing_order).unwrap();

        // Price increases to 62000 -> trailing stop price becomes 61000
        engine.on_last_price_update("BTCUSDT", 62000.0);
        let active = engine.active_orders.get("BTCUSDT").unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].stop_price, 61000.0);
        drop(active);

        // Price increases to 65000 -> trailing stop price becomes 64000
        engine.on_last_price_update("BTCUSDT", 65000.0);
        let active = engine.active_orders.get("BTCUSDT").unwrap();
        assert_eq!(active[0].stop_price, 64000.0);
        drop(active);

        // Price drops to 64500 -> stop_price stays at 64000, order still active
        engine.on_last_price_update("BTCUSDT", 64500.0);
        assert_eq!(engine.active_orders.get("BTCUSDT").unwrap().len(), 1);

        // Price drops to 63900 (<= 64000) -> order triggers!
        engine.on_last_price_update("BTCUSDT", 63900.0);
        assert_eq!(engine.active_orders.get("BTCUSDT").unwrap().len(), 0);

        // Check closed position PnL (63900 - 60000 = +3900)
        let acc = engine.accounts.get("trader1").unwrap();
        assert_eq!(acc.wallet_balance, 13900.0);
    }

    // -------------------------------------------------------------
    // 5. Over-closing Protection
    // -------------------------------------------------------------
    #[test]
    fn test_overclosing_protection() {
        let engine = make_engine();
        engine.latest_prices.insert("ADAUSDT".into(), 1.0);

        // Open Long 100 ADA @ 1.0
        let open_order = Order {
            id: "o_1".into(),
            user_id: "trader1".into(),
            symbol: "ADAUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 100.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("trader1", open_order).unwrap();

        // Close order with amount 200 (overclose attempt) @ 1.5
        engine.on_last_price_update("ADAUSDT", 1.5);
        let close_order = Order {
            id: "c_over".into(),
            user_id: "trader1".into(),
            symbol: "ADAUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 200.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("trader1", close_order).unwrap();

        let pos = engine.positions.get("trader1").unwrap();
        let p = pos.get("ADAUSDT_Long").unwrap();
        // Amount should be 0, NOT negative!
        assert_eq!(p.amount, 0.0);

        // Realized PnL should be calculated on 100 ADA: (1.5 - 1.0) * 100 = +50 USDT
        let acc = engine.accounts.get("trader1").unwrap();
        assert_eq!(acc.wallet_balance, 10050.0);
    }

    // -------------------------------------------------------------
    // 6. Short Position Liquidation & Maintenance Margin
    // -------------------------------------------------------------
    #[test]
    fn test_short_liquidation() {
        let engine = make_engine();
        engine.latest_prices.insert("NEARUSDT".into(), 10.0);

        // Short 100 NEAR @ 10.0 with 10x leverage
        // Margin = 100 * 10 / 10 = 100 USDT
        // Liquidation price = 10 * (1 + 1/10 - 0.005) = 10 * 1.095 = 10.95
        let short_order = Order {
            id: "short_liq".into(),
            user_id: "trader1".into(),
            symbol: "NEARUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Short,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 100.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("trader1", short_order).unwrap();

        let pos = engine.positions.get("trader1").unwrap();
        let p = pos.get("NEARUSDT_Short").unwrap();
        assert!((p.liquidation_price - 10.95).abs() < 1e-5);
        drop(p);
        drop(pos);

        // Price rises to 11.0 (>= 10.95) -> Short Liquidated!
        engine.on_mark_price_update("NEARUSDT", 11.0);

        let pos = engine.positions.get("trader1").unwrap();
        let p = pos.get("NEARUSDT_Short").unwrap();
        assert_eq!(p.amount, 0.0);
        drop(p);
        drop(pos);

        // Initial wallet balance was 10000. Loss deducted = 100 USDT margin
        let acc = engine.accounts.get("trader1").unwrap();
        assert_eq!(acc.wallet_balance, 9900.0);
    }

    // -------------------------------------------------------------
    // 7. SQLite Closed Positions & History Logging
    // -------------------------------------------------------------
    #[test]
    fn test_sqlite_logging() {
        let storage = Arc::new(Storage::new(":memory:").unwrap());
        let engine = PaperEngine::new(storage.clone());
        engine.create_account("trader1", 5000.0);

        engine.latest_prices.insert("DOTUSDT".into(), 20.0);

        // Open Long 10 DOT @ 20.0
        let open_order = Order {
            id: "sql_1".into(),
            user_id: "trader1".into(),
            symbol: "DOTUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 10.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 1000,
        };
        engine.submit_order("trader1", open_order).unwrap();

        // Close Long 10 DOT @ 25.0
        engine.on_last_price_update("DOTUSDT", 25.0);
        let close_order = Order {
            id: "sql_2".into(),
            user_id: "trader1".into(),
            symbol: "DOTUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 10.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 2000,
        };
        engine.submit_order("trader1", close_order).unwrap();

        // Verify SQLite insert
        assert_eq!(engine.accounts.get("trader1").unwrap().wallet_balance, 5050.0);
    }
}
