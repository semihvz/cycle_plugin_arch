"""
Every-Bar Backtest Simülasyon Motoru ve Performans Metrikleri
"""

import datetime
from .indicators import calculate_atr


def format_ts(ts_ms):
    """Unix timestamp (ms) -> UTC okunabilir zaman dizgisi."""
    if not ts_ms:
        return "---"
    sec = ts_ms // 1000
    dt = datetime.datetime.fromtimestamp(sec, tz=datetime.timezone.utc)
    return dt.strftime("%Y-%m-%d %H:%M:%S UTC")


def run_all_bars_backtest(
    symbol="TACUSDT",
    interval="1h",
    bars=None,
    lookback=100,
    fixed_pos_size=50.0,
):
    """
    Her mumda (Every-Bar) işlem açılıp kapatılan backtest simülasyonunu çalıştırır.
    """
    if bars is None:
        bars = []

    atr_series = calculate_atr(bars, 14)
    trade_history = []

    if len(bars) <= lookback:
        return {
            "symbol": symbol,
            "interval": interval,
            "total_bars": len(bars),
            "total_trades": 0,
            "winning_trades": 0,
            "losing_trades": 0,
            "open_trades": 0,
            "win_rate_pct": 0.0,
            "fixed_position_size_usdt": fixed_pos_size,
            "total_net_pnl_usdt": 0.0,
            "profit_factor": 0.0,
            "max_drawdown_usdt": 0.0,
            "max_drawdown_pct": 0.0,
            "avg_trade_pnl_usdt": 0.0,
            "trade_history": [],
            "bars": bars,
        }

    trade_id = 1

    for i in range(lookback, len(bars)):
        entry_bar = bars[i]
        entry_price = entry_bar["open"]
        entry_time = entry_bar["open_time"]

        window_100 = bars[i - lookback : i]
        lowest_100 = min(b["low"] for b in window_100)

        atr_val = atr_series[i - 1] if i > 0 else atr_series[i]
        atr_val = max(atr_val, 0.00001)

        raw_sl = lowest_100 - (2.0 * atr_val)
        sl_dist = max(entry_price - raw_sl, entry_price * 0.005)
        stop_loss = entry_price - sl_dist
        take_profit = entry_price + (2.0 * sl_dist)  # 1:2 R:R

        risk_ratio = sl_dist / entry_price
        risk_usdt = fixed_pos_size * risk_ratio
        reward_usdt = 2.0 * risk_usdt

        closed = False
        exit_index = None
        exit_time = None
        exit_price = None
        status = "OPEN"
        pnl_pct = 0.0
        pnl_usdt = 0.0
        holding_bars = 0

        for k in range(i, len(bars)):
            sim_bar = bars[k]
            holding_bars = k - i + 1

            if sim_bar["low"] <= stop_loss and sim_bar["high"] >= take_profit:
                closed = True
                exit_index = k
                exit_time = sim_bar["close_time"]
                exit_price = stop_loss
                status = "LOSS"
                pnl_usdt = -risk_usdt
                pnl_pct = -risk_ratio * 100.0
                break
            elif sim_bar["high"] >= take_profit:
                closed = True
                exit_index = k
                exit_time = sim_bar["close_time"]
                exit_price = take_profit
                status = "WIN"
                pnl_usdt = reward_usdt
                pnl_pct = 2.0 * risk_ratio * 100.0
                break
            elif sim_bar["low"] <= stop_loss:
                closed = True
                exit_index = k
                exit_time = sim_bar["close_time"]
                exit_price = stop_loss
                status = "LOSS"
                pnl_usdt = -risk_usdt
                pnl_pct = -risk_ratio * 100.0
                break

        if not closed:
            holding_bars = len(bars) - i

        trade_history.append({
            "id": trade_id,
            "symbol": symbol,
            "entry_index": i,
            "entry_time": entry_time,
            "entry_time_str": format_ts(entry_time),
            "entry_price": entry_price,
            "lowest_100_price": lowest_100,
            "atr_14": atr_val,
            "stop_loss": stop_loss,
            "take_profit": take_profit,
            "position_size_usdt": fixed_pos_size,
            "risk_usdt": risk_usdt,
            "target_reward_usdt": reward_usdt,
            "exit_index": exit_index,
            "exit_time": exit_time,
            "exit_time_str": format_ts(exit_time) if exit_time else None,
            "exit_price": exit_price,
            "pnl_pct": pnl_pct,
            "pnl_usdt": pnl_usdt,
            "holding_bars": holding_bars,
            "status": status,
            "lookback_bars": window_100,
        })
        trade_id += 1

    total_trades = len(trade_history)
    winning_trades = sum(1 for t in trade_history if t["status"] == "WIN")
    losing_trades = sum(1 for t in trade_history if t["status"] == "LOSS")
    open_trades = sum(1 for t in trade_history if t["status"] == "OPEN")

    closed_count = total_trades - open_trades
    win_rate_pct = (winning_trades / closed_count * 100.0) if closed_count > 0 else 0.0

    total_net_pnl_usdt = sum(t["pnl_usdt"] for t in trade_history)
    gross_wins = sum(t["pnl_usdt"] for t in trade_history if t["pnl_usdt"] > 0)
    gross_losses = sum(abs(t["pnl_usdt"]) for t in trade_history if t["pnl_usdt"] < 0)
    profit_factor = (gross_wins / gross_losses) if gross_losses > 0 else gross_wins

    peak = 0.0
    max_dd_usdt = 0.0
    max_dd_pct = 0.0
    running_eq = 0.0

    for t in trade_history:
        running_eq += t["pnl_usdt"]
        if running_eq > peak:
            peak = running_eq
        dd = peak - running_eq
        if dd > max_dd_usdt:
            max_dd_usdt = dd
            if peak > 0:
                max_dd_pct = (dd / peak) * 100.0

    avg_trade_pnl = (total_net_pnl_usdt / total_trades) if total_trades > 0 else 0.0

    return {
        "symbol": symbol,
        "interval": interval,
        "total_bars": len(bars),
        "total_trades": total_trades,
        "winning_trades": winning_trades,
        "losing_trades": losing_trades,
        "open_trades": open_trades,
        "win_rate_pct": win_rate_pct,
        "fixed_position_size_usdt": fixed_pos_size,
        "total_net_pnl_usdt": total_net_pnl_usdt,
        "profit_factor": profit_factor,
        "max_drawdown_usdt": max_dd_usdt,
        "max_drawdown_pct": max_dd_pct,
        "avg_trade_pnl_usdt": avg_trade_pnl,
        "trade_history": trade_history,
        "bars": bars,
    }
