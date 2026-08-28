#!/usr/bin/env python3
import urllib.request
import json
import time
import math
import datetime
import sqlite3
import os
import sys

DB_PATH = "/home/smhvz/Desktop/cycle-orc/data/magma_backtest.db"
os.makedirs(os.path.dirname(DB_PATH), exist_ok=True)

def init_db():
    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()
    cur.execute("""
        CREATE TABLE IF NOT EXISTS klines_15m (
            open_time INTEGER PRIMARY KEY,
            open REAL, high REAL, low REAL, close REAL, volume REAL, close_time INTEGER
        )
    """)
    cur.execute("""
        CREATE TABLE IF NOT EXISTS klines_1m (
            open_time INTEGER PRIMARY KEY,
            open REAL, high REAL, low REAL, close REAL, volume REAL, close_time INTEGER
        )
    """)
    conn.commit()
    conn.close()

def load_data():
    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()
    
    cur.execute("SELECT open_time, open, high, low, close, volume, close_time FROM klines_15m ORDER BY open_time ASC")
    rows_15m = cur.fetchall()
    
    cur.execute("SELECT open_time, open, high, low, close, volume, close_time FROM klines_1m ORDER BY open_time ASC")
    rows_1m = cur.fetchall()
    
    conn.close()
    
    klines_15m = [{
        'open_time': r[0], 'open': r[1], 'high': r[2], 'low': r[3],
        'close': r[4], 'volume': r[5], 'close_time': r[6]
    } for r in rows_15m]
    
    klines_1m = [{
        'open_time': r[0], 'open': r[1], 'high': r[2], 'low': r[3],
        'close': r[4], 'volume': r[5], 'close_time': r[6]
    } for r in rows_1m]
    
    return klines_15m, klines_1m

def calc_atr_14(klines):
    if len(klines) < 2:
        return 0.0
    trs = []
    for i in range(1, len(klines)):
        high = klines[i]['high']
        low = klines[i]['low']
        prev_close = klines[i-1]['close']
        tr = max(high - low, abs(high - prev_close), abs(low - prev_close))
        trs.append(tr)
    if not trs:
        return 0.0
    period = min(14, len(trs))
    first_atr = sum(trs[:period]) / period
    multiplier = 2.0 / (period + 1)
    atr = first_atr
    for tr in trs[period:]:
        atr = (tr - atr) * multiplier + atr
    return atr

def linear_regression(values):
    n = len(values)
    if n < 2:
        return 0.0, 0.0, 0.0
    x_mean = (n - 1) / 2.0
    y_mean = sum(values) / n
    ss_xy = 0.0
    ss_xx = 0.0
    ss_yy = 0.0
    for i, y in enumerate(values):
        dx = i - x_mean
        dy = y - y_mean
        ss_xy += dx * dy
        ss_xx += dx * dx
        ss_yy += dy * dy
    if ss_xx == 0:
        return 0.0, y_mean, 0.0
    slope = ss_xy / ss_xx
    intercept = y_mean - slope * x_mean
    r_squared = 0.0 if ss_yy == 0 else (ss_xy * ss_xy) / (ss_xx * ss_yy)
    return slope, intercept, r_squared

def analyze_trend(klines, atr):
    if not klines or atr <= 0:
        return 0.0
    n = min(len(klines), 50)
    recent = klines[-n:]
    log_prices = [math.log(k['close']) for k in recent]
    slope, _, r_squared = linear_regression(log_prices)
    price_slope = slope * recent[-1]['close']
    raw_score = (price_slope / atr) * 10.0 * r_squared
    return max(-10.0, min(10.0, raw_score))

def get_ats(klines):
    if len(klines) < 10:
        return 0.0
    atr = calc_atr_14(klines)
    len_k = len(klines)
    core_limit = min(100, len_k)
    amp_limit = min(400, len_k)
    acute_limit = min(96, len_k)
    
    core_klines = klines[len_k - core_limit:]
    amp_klines = klines[len_k - amp_limit:]
    acute_klines = klines[len_k - acute_limit:]
    
    score_core = analyze_trend(core_klines, atr)
    score_amp = analyze_trend(amp_klines, atr)
    score_acute = analyze_trend(acute_klines, atr)
    
    ats = (score_core * 0.40) + (score_amp * 0.30) + (score_acute * 0.30)
    return ats

def extract_pivot_levels(klines, atr):
    threshold = atr * 0.25
    pivots = []
    if len(klines) < 7:
        return pivots
    window = 3
    for i in range(window, len(klines) - window):
        is_sh_a = all(klines[i]['high'] >= klines[i-j]['high'] and klines[i]['high'] >= klines[i+j]['high'] for j in range(1, window + 1)) and (klines[i]['high'] - klines[i]['low']) >= threshold
        is_sl_a = all(klines[i]['low'] <= klines[i-j]['low'] and klines[i]['low'] <= klines[i+j]['low'] for j in range(1, window + 1)) and (klines[i]['high'] - klines[i]['low']) >= threshold
        
        if is_sh_a:
            pivots.append({'price': klines[i]['high'], 'type': 'SH'})
        if is_sl_a:
            pivots.append({'price': klines[i]['low'], 'type': 'SL'})
            
        is_sh_b = all(klines[i]['close'] >= klines[i-j]['close'] and klines[i]['close'] >= klines[i+j]['close'] for j in range(1, window + 1)) and abs(klines[i]['close'] - klines[i]['open']) >= threshold * 0.5
        is_sl_b = all(klines[i]['close'] <= klines[i-j]['close'] and klines[i]['close'] <= klines[i+j]['close'] for j in range(1, window + 1)) and abs(klines[i]['close'] - klines[i]['open']) >= threshold * 0.5
        
        if is_sh_b:
            pivots.append({'price': klines[i]['close'], 'type': 'SH'})
        if is_sl_b:
            pivots.append({'price': klines[i]['close'], 'type': 'SL'})
    return pivots

def get_tp_sl_levels(klines_15m, direction, current_price):
    atr = calc_atr_14(klines_15m)
    pivots = extract_pivot_levels(klines_15m, atr)
    
    if direction == "LONG":
        sl_candidates = [p['price'] for p in pivots if p['type'] == 'SL' and p['price'] < current_price]
        sh_candidates = [p['price'] for p in pivots if p['type'] == 'SH' and p['price'] > current_price]
        
        if sl_candidates:
            raw_sl = max(sl_candidates)
            if (current_price - raw_sl) < (current_price * 0.003):
                sl_price = current_price - max(current_price * 0.005, 1.5 * atr)
            else:
                sl_price = raw_sl
        else:
            sl_price = current_price - max(current_price * 0.008, 1.5 * atr)
            
        sl_dist = current_price - sl_price
        
        if sh_candidates:
            raw_tp = min(sh_candidates)
            if (raw_tp - current_price) < (1.5 * sl_dist):
                tp_price = current_price + (2.0 * sl_dist)
            else:
                tp_price = raw_tp
        else:
            tp_price = current_price + (2.0 * sl_dist)
            
        return tp_price, sl_price

    else: # SHORT
        sh_candidates = [p['price'] for p in pivots if p['type'] == 'SH' and p['price'] > current_price]
        sl_candidates = [p['price'] for p in pivots if p['type'] == 'SL' and p['price'] < current_price]
        
        if sh_candidates:
            raw_sl = min(sh_candidates)
            if (raw_sl - current_price) < (current_price * 0.003):
                sl_price = current_price + max(current_price * 0.005, 1.5 * atr)
            else:
                sl_price = raw_sl
        else:
            sl_price = current_price + max(current_price * 0.008, 1.5 * atr)
            
        sl_dist = sl_price - current_price
        
        if sl_candidates:
            raw_tp = max(sl_candidates)
            if (current_price - raw_tp) < (1.5 * sl_dist):
                tp_price = current_price - (2.0 * sl_dist)
            else:
                tp_price = raw_tp
        else:
            tp_price = current_price - (2.0 * sl_dist)
            
        return tp_price, sl_price

def run_backtest_fixed_size(position_size_usdt=100.0, initial_capital_usdt=1000.0):
    klines_15m, klines_1m = load_data()
    print(f"📊 Veri Yüklendi: 15m -> {len(klines_15m)} mum | 1m -> {len(klines_1m)} mum")
    print(f"💰 Sabit Pozisyon Büyüklüğü: ${position_size_usdt:.2f} USDT | Başlangıç Sermayesi: ${initial_capital_usdt:.2f} USDT")
    
    m1_dict = {k['open_time']: idx for idx, k in enumerate(klines_1m)}
    
    now_ms = int(time.time() * 1000)
    start_2m_ms = now_ms - (60 * 24 * 60 * 60 * 1000)
    
    start_idx_15m = 400
    for idx, k in enumerate(klines_15m):
        if k['open_time'] >= start_2m_ms:
            start_idx_15m = max(400, idx)
            break
            
    trades = []
    active_trade = None
    current_equity = initial_capital_usdt
    peak_equity = initial_capital_usdt
    max_drawdown_usdt = 0.0
    max_drawdown_pct = 0.0
    
    for i in range(start_idx_15m, len(klines_15m)):
        current_15m = klines_15m[i]
        ts_15m = current_15m['open_time']
        
        if active_trade is not None:
            if ts_15m < active_trade['exit_time']:
                continue
            else:
                active_trade = None
                
        window_15m = klines_15m[i-400:i+1]
        ats_15m = get_ats(window_15m)
        
        if abs(ats_15m) < 0.1:
            continue
            
        direction = "LONG" if ats_15m > 0 else "SHORT"
        
        if ts_15m not in m1_dict:
            continue
        idx_1m = m1_dict[ts_15m]
        if idx_1m < 400:
            continue
            
        window_1m = klines_1m[idx_1m-400:idx_1m+1]
        ats_1m = get_ats(window_1m)
        
        if direction == "LONG" and ats_1m <= 0:
            continue
        if direction == "SHORT" and ats_1m >= 0:
            continue
            
        entry_price = current_15m['close']
        tp_price, sl_price = get_tp_sl_levels(window_15m, direction, entry_price)
        
        exit_price = None
        exit_time = None
        result = None
        duration_mins = 0
        
        max_forward = min(len(klines_1m), idx_1m + 1 + 2880) # max 48h
        
        for f_idx in range(idx_1m + 1, max_forward):
            bar = klines_1m[f_idx]
            duration_mins += 1
            
            if direction == "LONG":
                if bar['low'] <= sl_price and bar['high'] >= tp_price:
                    result = "LOSS"
                    exit_price = sl_price
                    exit_time = bar['open_time']
                    break
                elif bar['low'] <= sl_price:
                    result = "LOSS"
                    exit_price = sl_price
                    exit_time = bar['open_time']
                    break
                elif bar['high'] >= tp_price:
                    result = "WIN"
                    exit_price = tp_price
                    exit_time = bar['open_time']
                    break
            else: # SHORT
                if bar['high'] >= sl_price and bar['low'] <= tp_price:
                    result = "LOSS"
                    exit_price = sl_price
                    exit_time = bar['open_time']
                    break
                elif bar['high'] >= sl_price:
                    result = "LOSS"
                    exit_price = sl_price
                    exit_time = bar['open_time']
                    break
                elif bar['low'] <= tp_price:
                    result = "WIN"
                    exit_price = tp_price
                    exit_time = bar['open_time']
                    break
                    
        if result is None:
            last_bar = klines_1m[max_forward - 1]
            exit_price = last_bar['close']
            exit_time = last_bar['open_time']
            pnl_raw = ((exit_price - entry_price) / entry_price) if direction == "LONG" else ((entry_price - exit_price) / entry_price)
            result = "WIN" if pnl_raw > 0 else "LOSS"
            
        pnl_pct = ((exit_price - entry_price) / entry_price * 100.0) if direction == "LONG" else ((entry_price - exit_price) / entry_price * 100.0)
        
        # FIXED POSITION SIZE 100 USDT PnL CALCULATIONS
        gross_pnl_usdt = position_size_usdt * (pnl_pct / 100.0)
        taker_fee_usdt = position_size_usdt * 0.0008 # 0.08% taker fee per roundtrip
        net_pnl_usdt = gross_pnl_usdt - taker_fee_usdt
        
        current_equity += net_pnl_usdt
        if current_equity > peak_equity:
            peak_equity = current_equity
        dd_usdt = peak_equity - current_equity
        dd_pct = (dd_usdt / peak_equity * 100.0) if peak_equity > 0 else 0.0
        
        if dd_usdt > max_drawdown_usdt:
            max_drawdown_usdt = dd_usdt
        if dd_pct > max_drawdown_pct:
            max_drawdown_pct = dd_pct
            
        trade_data = {
            'trade_id': len(trades) + 1,
            'entry_time': ts_15m,
            'entry_date': datetime.datetime.fromtimestamp(ts_15m/1000, datetime.timezone.utc).strftime('%Y-%m-%d %H:%M'),
            'exit_time': exit_time,
            'exit_date': datetime.datetime.fromtimestamp(exit_time/1000, datetime.timezone.utc).strftime('%Y-%m-%d %H:%M'),
            'direction': direction,
            'entry_price': entry_price,
            'tp_price': tp_price,
            'sl_price': sl_price,
            'exit_price': exit_price,
            'ats_15m': ats_15m,
            'ats_1m': ats_1m,
            'result': result,
            'pnl_pct': pnl_pct,
            'net_pnl_usdt': net_pnl_usdt,
            'current_equity': current_equity,
            'duration_mins': duration_mins
        }
        
        trades.append(trade_data)
        active_trade = trade_data

    # SUMMARY CALCULATIONS
    total_trades = len(trades)
    wins = [t for t in trades if t['net_pnl_usdt'] > 0]
    losses = [t for t in trades if t['net_pnl_usdt'] <= 0]
    
    win_count = len(wins)
    loss_count = len(losses)
    win_rate = (win_count / total_trades * 100.0) if total_trades > 0 else 0.0
    
    total_net_pnl_usdt = sum(t['net_pnl_usdt'] for t in trades)
    gross_profit_usdt = sum(t['net_pnl_usdt'] for t in wins)
    gross_loss_usdt = abs(sum(t['net_pnl_usdt'] for t in losses))
    profit_factor = (gross_profit_usdt / gross_loss_usdt) if gross_loss_usdt > 0 else (gross_profit_usdt if gross_profit_usdt > 0 else 0.0)
    
    avg_win_usdt = (gross_profit_usdt / win_count) if win_count > 0 else 0.0
    avg_loss_usdt = (gross_loss_usdt / loss_count) if loss_count > 0 else 0.0
    
    print("\n" + "=" * 110)
    print(f"📈 MAGMAUSDT (SABİT {position_size_usdt:.0f} USDT POZİSYON BOYUTU) 2 AYLIK MULTI-TIMEFRAME BACKTEST RAPORU")
    print("=" * 110)
    print(f"• Pozisyon Başı İşlem Büyüklüğü  : ${position_size_usdt:.2f} USDT")
    print(f"• Başlangıç Toplam Bakiyesi     : ${initial_capital_usdt:.2f} USDT")
    print(f"• Bitiş Bakiyesi               : ${current_equity:.2f} USDT")
    print(f"• Toplam Net Kâr / Zarar ($)   : ${total_net_pnl_usdt:+,.2f} USDT (Sermaye Oranı: %{(total_net_pnl_usdt/initial_capital_usdt*100.0):+.2f})")
    print(f"• Toplam İşlem Sayısı          : {total_trades}")
    print(f"• Başarılı (Kazançlı)          : {win_count}  (Win Rate: %{win_rate:.2f})")
    print(f"• Başarısız (Kayıplı)         : {loss_count}")
    print(f"• Profit Factor                : {profit_factor:.2f}")
    print(f"• Ortalama İşlem Başı Kazanç   : +${avg_win_usdt:.2f} USDT")
    print(f"• Ortalama İşlem Başı Kayıp    : -${avg_loss_usdt:.2f} USDT")
    print(f"• Maksimum Drawdown (Düşüş)    : -${max_drawdown_usdt:.2f} USDT (Peak Bakiye Üzerinden %{max_drawdown_pct:.2f})")
    print("=" * 110)
    
    print("\n📋 İŞLEM DETAY LİSTESİ (SON 20 İŞLEM - FIXED $100 USDT):")
    print("-" * 125)
    print(f"{'#':<4} | {'Giriş Tarihi':<16} | {'Yön':<6} | {'Giriş Fiyatı':<11} | {'TP Fiyatı':<11} | {'SL Fiyatı':<11} | {'15m ATS':<7} | {'1m ATS':<7} | {'Sonuç':<6} | {'Net PnL ($)':<12} | {'Bakiye ($)':<12}")
    print("-" * 125)
    
    for t in trades[-20:]:
        res_str = "🟢 KAZANÇ" if t['net_pnl_usdt'] > 0 else "🔴 KAYIP"
        print(f"{t['trade_id']:<4} | {t['entry_date']:<16} | {t['direction']:<6} | {t['entry_price']:<11.4f} | {t['tp_price']:<11.4f} | {t['sl_price']:<11.4f} | {t['ats_15m']:<+7.2f} | {t['ats_1m']:<+7.2f} | {res_str:<6} | ${t['net_pnl_usdt']:<+11.2f} | ${t['current_equity']:<11.2f}")
    print("=" * 125)

if __name__ == "__main__":
    run_backtest_fixed_size(position_size_usdt=100.0, initial_capital_usdt=1000.0)
