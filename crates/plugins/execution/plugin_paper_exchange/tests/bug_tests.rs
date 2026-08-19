#[cfg(test)]
mod bug_tests {
    use std::sync::Arc;
    use plugin_paper_exchange::*;

    fn make_engine() -> Arc<PaperEngine> {
        let storage = Storage::new(":memory:").unwrap();
        let engine = PaperEngine::new(Arc::new(storage));
        engine.create_account("test", 10000.0);
        Arc::new(engine)
    }

    // ============================================================
    // BUG TEST 1: Pozisyon kapatınca kâr wallet_balance'a yansıyor mu?
    // Long aç 100'den, fiyat 110'a çıksın, kapat → 10 USDT kâr olmalı
    // ============================================================
    #[test]
    fn test_bug_realized_pnl_not_credited() {
        let engine = make_engine();

        // Fiyat: 100
        engine.latest_prices.insert("BTCUSDT".into(), 100.0);

        // Long aç: 1 adet, 10x kaldıraç
        let order = Order {
            id: "open_1".into(),
            user_id: "test".into(),
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
        engine.submit_order("test", order).unwrap();

        // Fiyat 110'a çıktı
        engine.on_mark_price_update("BTCUSDT", 110.0);
        engine.on_last_price_update("BTCUSDT", 110.0);

        // unrealized PnL kontrol
        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("BTCUSDT_Long").unwrap();
        assert_eq!(p.unrealized_pnl, 10.0, "unrealized PnL 10 olmalı");
        drop(p);
        drop(pos);

        // Pozisyonu kapat: Sell 1 adet
        let close_order = Order {
            id: "close_1".into(),
            user_id: "test".into(),
            symbol: "BTCUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 1.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("test", close_order).unwrap();

        // Pozisyon kapandı mı?
        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("BTCUSDT_Long").unwrap();
        assert!(p.amount == 0.0, "Pozisyon kapanmış olmalı");
        drop(p);
        drop(pos);

        // ASIL TEST: wallet_balance 10010 olmalı (10000 + 10 kâr)
        let acc = engine.accounts.get("test").unwrap();
        println!("BUG TEST 1 - wallet_balance: {} (beklenen: 10010)", acc.wallet_balance);
        println!("BUG TEST 1 - margin_balance: {} (beklenen: 10010)", acc.margin_balance);

        // Eğer 10000 kalıyorsa → kâr kayıp, BUG!
        if acc.wallet_balance == 10000.0 {
            println!("❌ BUG BULUNDU: Realized PnL wallet_balance'a EKLENMIYOR! Kâr kayboldu.");
        }
        assert_eq!(acc.wallet_balance, 10010.0, "BUG: Realized PnL wallet_balance'a eklenmiyor!");
    }

    // ============================================================
    // BUG TEST 2: Zarar ile kapatınca wallet_balance düşüyor mu?
    // Long aç 100'den, fiyat 90'a düşsün, kapat → 10 USDT zarar
    // ============================================================
    #[test]
    fn test_bug_realized_loss_not_deducted() {
        let engine = make_engine();

        engine.latest_prices.insert("ETHUSDT".into(), 100.0);

        let order = Order {
            id: "loss_open".into(),
            user_id: "test".into(),
            symbol: "ETHUSDT".into(),
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
        engine.submit_order("test", order).unwrap();

        // Fiyat 90'a düştü
        engine.on_mark_price_update("ETHUSDT", 90.0);
        engine.on_last_price_update("ETHUSDT", 90.0);

        // Kapat
        let close = Order {
            id: "loss_close".into(),
            user_id: "test".into(),
            symbol: "ETHUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 1.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("test", close).unwrap();

        let acc = engine.accounts.get("test").unwrap();
        println!("BUG TEST 2 - wallet_balance: {} (beklenen: 9990)", acc.wallet_balance);

        if acc.wallet_balance == 10000.0 {
            println!("❌ BUG BULUNDU: Realized Loss wallet_balance'dan DÜŞÜLMÜYOR!");
        }
        assert_eq!(acc.wallet_balance, 9990.0, "BUG: Realized loss wallet_balance'dan düşülmüyor!");
    }

    // ============================================================
    // BUG TEST 3: Short pozisyon PnL doğru mu?
    // Short aç 100'den, fiyat 90'a düşsün → 10 USDT kâr
    // ============================================================
    #[test]
    fn test_short_pnl_correct() {
        let engine = make_engine();

        engine.latest_prices.insert("XRPUSDT".into(), 100.0);

        let order = Order {
            id: "short_1".into(),
            user_id: "test".into(),
            symbol: "XRPUSDT".into(),
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
        engine.submit_order("test", order).unwrap();

        engine.on_mark_price_update("XRPUSDT", 90.0);

        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("XRPUSDT_Short").unwrap();
        println!("BUG TEST 3 - Short PnL: {} (beklenen: 10)", p.unrealized_pnl);
        assert_eq!(p.unrealized_pnl, 10.0, "Short PnL hesaplaması yanlış!");
    }

    // ============================================================
    // BUG TEST 4: Likidasyon sonrası wallet_balance doğru mu?
    // 20x kaldıraçla Long aç, likidasyon fiyatına düşür
    // ============================================================
    #[test]
    fn test_liquidation_balance() {
        let engine = make_engine();

        engine.latest_prices.insert("SOLUSDT".into(), 100.0);

        let order = Order {
            id: "liq_1".into(),
            user_id: "test".into(),
            symbol: "SOLUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 10.0, // 10 adet × 100 = 1000 USDT notional, 20x → 50 USDT margin
            leverage: 20.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("test", order).unwrap();

        // Likidasyon fiyatı: 100 * (1 - 1/20 + 0.005) = 100 * 0.955 = 95.5
        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("SOLUSDT_Long").unwrap();
        let liq_price = p.liquidation_price;
        println!("BUG TEST 4 - Likidasyon fiyatı: {} (beklenen: 95.5)", liq_price);
        drop(p);
        drop(pos);

        // Mark fiyatı likidasyon altına düşür
        engine.on_mark_price_update("SOLUSDT", 95.0);

        let acc = engine.accounts.get("test").unwrap();
        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("SOLUSDT_Long").unwrap();

        println!("BUG TEST 4 - Likidasyon sonrası: wallet={}, margin={}, pos_amount={}", 
            acc.wallet_balance, acc.margin_balance, p.amount);

        // Pozisyon sıfırlanmış olmalı
        assert_eq!(p.amount, 0.0, "Likidasyon sonrası pozisyon sıfır olmalı");

        // Wallet balance düşmüş olmalı (margin kaybı = 10 * 100 / 20 = 50)
        let expected_balance = 10000.0 - 50.0; // 9950
        println!("BUG TEST 4 - wallet_balance: {} (beklenen: {})", acc.wallet_balance, expected_balance);

        if acc.wallet_balance != expected_balance {
            println!("⚠️ Likidasyon loss hesaplaması beklenenden farklı. Gerçek kayıp: {}", 10000.0 - acc.wallet_balance);
        }
    }

    // ============================================================
    // BUG TEST 5: Limit emir tetiklenmesi doğru mu?
    // Buy Limit 95'e koy, fiyat 94'e düşsün → tetiklenmeli
    // ============================================================
    #[test]
    fn test_limit_order_trigger() {
        let engine = make_engine();

        engine.latest_prices.insert("BNBUSDT".into(), 100.0);

        let limit = Order {
            id: "limit_1".into(),
            user_id: "test".into(),
            symbol: "BNBUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Limit,
            price: 95.0,
            stop_price: 0.0,
            amount: 2.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("test", limit).unwrap();

        // Fiyat henüz 97, tetiklenmemeli
        engine.on_last_price_update("BNBUSDT", 97.0);
        let pos = engine.positions.get("test").unwrap();
        let has_pos = pos.get("BNBUSDT_Long").map(|p| p.amount > 0.0).unwrap_or(false);
        assert!(!has_pos, "Fiyat 97'de limit 95 tetiklenmemeli!");
        drop(pos);

        // Fiyat 94'e düştü → limit tetiklenmeli
        engine.on_last_price_update("BNBUSDT", 94.0);
        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("BNBUSDT_Long").unwrap();
        println!("BUG TEST 5 - Limit tetikleme: amount={} entry={} (beklenen: amount=2, entry=95)", p.amount, p.entry_price);
        assert_eq!(p.amount, 2.0, "Limit emir tetiklenmiş olmalı");
        assert_eq!(p.entry_price, 95.0, "Limit emir fiyatından execute edilmeli");
    }

    // ============================================================
    // BUG TEST 6: StopMarket emir tetiklenmesi doğru mu?
    // Buy StopMarket stop=105'e koy, fiyat 106'ya çıksın → tetiklenmeli
    // ============================================================
    #[test]
    fn test_stop_market_trigger() {
        let engine = make_engine();

        engine.latest_prices.insert("DOTUSDT".into(), 100.0);

        let stop = Order {
            id: "stop_1".into(),
            user_id: "test".into(),
            symbol: "DOTUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::StopMarket,
            price: 0.0,
            stop_price: 105.0,
            amount: 3.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("test", stop).unwrap();

        // Fiyat 103, tetiklenmemeli
        engine.on_last_price_update("DOTUSDT", 103.0);
        let pos = engine.positions.get("test").unwrap();
        let has = pos.get("DOTUSDT_Long").map(|p| p.amount > 0.0).unwrap_or(false);
        assert!(!has, "Fiyat 103'te stop 105 tetiklenmemeli");
        drop(pos);

        // Fiyat 106 → tetiklenmeli
        engine.on_last_price_update("DOTUSDT", 106.0);
        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("DOTUSDT_Long").unwrap();
        println!("BUG TEST 6 - StopMarket: amount={} entry={} (beklenen: 3, market price 106)", p.amount, p.entry_price);
        assert_eq!(p.amount, 3.0, "Stop emir tetiklenmiş olmalı");
        // StopMarket → market fiyatından execute
        assert_eq!(p.entry_price, 106.0, "StopMarket piyasa fiyatından execute edilmeli");
    }

    // ============================================================
    // BUG TEST 7: Aynı sembolde birden fazla pozisyon ekleme (ortalama giriş)
    // 100'den 1 adet, sonra 110'dan 1 adet → ortalama 105 olmalı
    // ============================================================
    #[test]
    fn test_average_entry_price() {
        let engine = make_engine();

        engine.latest_prices.insert("AVAXUSDT".into(), 100.0);

        let o1 = Order {
            id: "avg_1".into(),
            user_id: "test".into(),
            symbol: "AVAXUSDT".into(),
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
        engine.submit_order("test", o1).unwrap();

        engine.on_last_price_update("AVAXUSDT", 110.0);

        let o2 = Order {
            id: "avg_2".into(),
            user_id: "test".into(),
            symbol: "AVAXUSDT".into(),
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
        engine.submit_order("test", o2).unwrap();

        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("AVAXUSDT_Long").unwrap();
        println!("BUG TEST 7 - Ortalama giriş: {} (beklenen: 105)", p.entry_price);
        assert_eq!(p.amount, 2.0);
        assert_eq!(p.entry_price, 105.0, "Ortalama giriş fiyatı yanlış!");
    }

    // ============================================================
    // BUG TEST 8: Kısmi kapatma doğru mu?
    // 2 adet açtın, 1 adet kapat → 1 adet kalmalı, entry değişmemeli
    // ============================================================
    #[test]
    fn test_partial_close() {
        let engine = make_engine();

        engine.latest_prices.insert("LINKUSDT".into(), 50.0);

        let open = Order {
            id: "part_open".into(),
            user_id: "test".into(),
            symbol: "LINKUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 2.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("test", open).unwrap();

        // Kısmi kapat: 1 adet
        engine.on_last_price_update("LINKUSDT", 55.0);

        let close = Order {
            id: "part_close".into(),
            user_id: "test".into(),
            symbol: "LINKUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 1.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("test", close).unwrap();

        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("LINKUSDT_Long").unwrap();
        println!("BUG TEST 8 - Kısmi kapatma: amount={} entry={} (beklenen: 1, 50)", p.amount, p.entry_price);
        assert_eq!(p.amount, 1.0, "Kısmi kapatma sonrası 1 adet kalmalı");
        assert_eq!(p.entry_price, 50.0, "Entry price değişmemeli");
    }

    // ============================================================
    // BUG TEST 9: TakeProfit emir doğru tetikleniyor mu?
    // Long pozisyon var, TakeProfit Sell stop=110, fiyat 111 → tetiklenmeli
    // ============================================================
    #[test]
    fn test_take_profit_trigger() {
        let engine = make_engine();

        engine.latest_prices.insert("ADAUSDT".into(), 100.0);

        // Önce pozisyon aç
        let open = Order {
            id: "tp_open".into(),
            user_id: "test".into(),
            symbol: "ADAUSDT".into(),
            side: OrderSide::Buy,
            position_side: PositionSide::Long,
            order_type: OrderType::Market,
            price: 0.0,
            stop_price: 0.0,
            amount: 5.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("test", open).unwrap();

        // TakeProfit Sell at 110
        let tp = Order {
            id: "tp_1".into(),
            user_id: "test".into(),
            symbol: "ADAUSDT".into(),
            side: OrderSide::Sell,
            position_side: PositionSide::Long,
            order_type: OrderType::TakeProfitMarket,
            price: 0.0,
            stop_price: 110.0,
            amount: 5.0,
            leverage: 10.0,
            executed: 0.0,
            timestamp: 0,
        };
        engine.submit_order("test", tp).unwrap();

        // Fiyat 105, tetiklenmemeli
        engine.on_last_price_update("ADAUSDT", 105.0);
        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("ADAUSDT_Long").unwrap();
        assert_eq!(p.amount, 5.0, "105'te TP tetiklenmemeli");
        drop(p);
        drop(pos);

        // Fiyat 111 → TakeProfit Sell tetiklenmeli
        engine.on_last_price_update("ADAUSDT", 111.0);
        let pos = engine.positions.get("test").unwrap();
        let p = pos.get("ADAUSDT_Long").unwrap();
        println!("BUG TEST 9 - TP sonrası: amount={} (beklenen: 0)", p.amount);
        assert_eq!(p.amount, 0.0, "TakeProfit tetiklendikten sonra pozisyon kapanmalı");
    }

    // ============================================================
    // BUG TEST 10: Çoklu likidasyon - iki farklı kullanıcı aynı anda
    // ============================================================
    #[test]
    fn test_multi_user_liquidation() {
        let engine = make_engine();
        engine.create_account("user_a", 5000.0);
        engine.create_account("user_b", 5000.0);

        engine.latest_prices.insert("MATICUSDT".into(), 100.0);

        // Her iki kullanıcı da aynı sembolde long aç
        for user in &["user_a", "user_b"] {
            let o = Order {
                id: format!("multi_{}", user),
                user_id: user.to_string(),
                symbol: "MATICUSDT".into(),
                side: OrderSide::Buy,
                position_side: PositionSide::Long,
                order_type: OrderType::Market,
                price: 0.0,
                stop_price: 0.0,
                amount: 10.0,
                leverage: 20.0,
                executed: 0.0,
                timestamp: 0,
            };
            engine.submit_order(user, o).unwrap();
        }

        // Likidasyon seviyesinin altına düşür
        engine.on_mark_price_update("MATICUSDT", 94.0);

        let acc_a = engine.accounts.get("user_a").unwrap();
        let acc_b = engine.accounts.get("user_b").unwrap();

        println!("BUG TEST 10 - user_a wallet: {} (beklenen: 4950)", acc_a.wallet_balance);
        println!("BUG TEST 10 - user_b wallet: {} (beklenen: 4950)", acc_b.wallet_balance);

        // Her iki kullanıcının da bakiyesi düşmüş olmalı
        let a_lost = acc_a.wallet_balance < 5000.0;
        let b_lost = acc_b.wallet_balance < 5000.0;

        if !a_lost || !b_lost {
            println!("❌ BUG BULUNDU: Çoklu likidasyon düzgün çalışmıyor! a_lost={} b_lost={}", a_lost, b_lost);
        }
        assert!(a_lost, "user_a likide olmuş olmalı");
        assert!(b_lost, "user_b likide olmuş olmalı");
    }
}
