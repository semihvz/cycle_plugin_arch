"""
Terminal Raporlama ve Görselleştirme Modülü
"""


def print_report(summary):
    """Terminalde detaylı ASCII raporu basar."""
    print("\n" + "=" * 100)
    print("📈 ALL-BARS BACKTEST SİSTEMİ - DETAYLI PERFORMANS RAPORU")
    print("=" * 100)
    print(f"Sembol / Zaman Dilimi    : {summary['symbol']} / {summary['interval']}")
    print(f"Toplam Mum (Bar) Sayısı  : {summary['total_bars']} adet")
    print(f"Açılan Toplam İşlem Sayısı: {summary['total_trades']} adet (100-bar lookback sonrası)")
    print(f"Sabit Pozisyon Büyüklüğü  : {summary['fixed_position_size_usdt']:.2f} USDT")
    print("-" * 100)
    print(f"Kazanılan İşlemler (WIN)  : {summary['winning_trades']} adet")
    print(f"Kaybedilen İşlemler (LOSS): {summary['losing_trades']} adet")
    print(f"Halen Açık İşlemler (OPEN): {summary['open_trades']} adet")
    print(f"Kazanma Oranı (Win Rate)  : {summary['win_rate_pct']:.2f}%")
    print(f"Net Toplam Kâr/Zarar (PnL): {summary['total_net_pnl_usdt']:+.2f} USDT")
    print(f"Profit Factor (Kâr Oranı) : {summary['profit_factor']:.2f}")
    print(
        f"Maksimum Çekilme (Max DD) : {summary['max_drawdown_usdt']:.2f} USDT "
        f"({summary['max_drawdown_pct']:.2f}%)"
    )
    print(f"İşlem Başına Ort. PnL     : {summary['avg_trade_pnl_usdt']:+.2f} USDT")
    print("-" * 100)

    trades = summary["trade_history"]
    print(f"İşlem Geçmişi Dökümü (Toplam {len(trades)} İşlem):")
    show_count = 10
    for t in trades[:show_count]:
        exit_str = t["exit_time_str"] or "Halen Açık"
        exit_p = f"{t['exit_price']:.5f}" if t["exit_price"] else "---"
        print(
            f"  • Trade #{t['id']:<4} | Giriş: {t['entry_time_str']} | Price: {t['entry_price']:>8.5f} | "
            f"SL: {t['stop_loss']:>8.5f} | TP: {t['take_profit']:>8.5f} | Çıkış: {exit_str} | "
            f"ExitP: {exit_p:>8} | Result: {t['status']:<4} | PnL: {t['pnl_usdt']:>+6.2f} USDT "
            f"(Barlar: {t['holding_bars']:>2})"
        )
    if len(trades) > show_count * 2:
        print(f"  ... (Aradaki {len(trades) - show_count * 2} işlem gizlendi) ...")
        for t in trades[-show_count:]:
            exit_str = t["exit_time_str"] or "Halen Açık"
            exit_p = f"{t['exit_price']:.5f}" if t["exit_price"] else "---"
            print(
                f"  • Trade #{t['id']:<4} | Giriş: {t['entry_time_str']} | Price: {t['entry_price']:>8.5f} | "
                f"SL: {t['stop_loss']:>8.5f} | TP: {t['take_profit']:>8.5f} | Çıkış: {exit_str} | "
                f"ExitP: {exit_p:>8} | Result: {t['status']:<4} | PnL: {t['pnl_usdt']:>+6.2f} USDT "
                f"(Barlar: {t['holding_bars']:>2})"
            )
    print("=" * 100 + "\n")
