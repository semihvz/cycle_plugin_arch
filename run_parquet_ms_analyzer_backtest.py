#!/usr/bin/env python3
import glob
import os
import sys
import math
import time
import datetime
import json
import pandas as pd
import numpy as np
from concurrent.futures import ProcessPoolExecutor, as_completed

PARQUET_DIR = "/home/smhvz/Desktop/cycle-orc/data/parquet_klines"

# -----------------------------------------------------------------------------
# MSMP 2.0 ANALYZER ENGINE (PURE PYTHON / NUMPY IMPLEMENTATION OF RUST ENGINE)
# -----------------------------------------------------------------------------

def calc_atr_14(klines):
    if len(klines) < 2:
        return 0.0
    highs = np.array([k['high'] for k in klines])
    lows = np.array([k['low'] for k in klines])
    closes = np.array([k['close'] for k in klines])
    
    tr0 = highs[1:] - lows[1:]
    tr1 = np.abs(highs[1:] - closes[:-1])
    tr2 = np.abs(lows[1:] - closes[:-1])
    trs = np.maximum(tr0, np.maximum(tr1, tr2))
    
    if len(trs) == 0:
        return 0.0
    period = min(14, len(trs))
    first_atr = np.mean(trs[:period])
    multiplier = 2.0 / (period + 1)
    atr = first_atr
    for tr in trs[period:]:
        atr = (tr - atr) * multiplier + atr
    return float(atr)

def linear_regression(values):
    n = len(values)
    if n < 2:
        return 0.0, 0.0, 0.0
    x = np.arange(n, dtype=np.float64)
    y = np.array(values, dtype=np.float64)
    
    x_mean = (n - 1) / 2.0
    y_mean = np.mean(y)
    
    dx = x - x_mean
    dy = y - y_mean
    
    ss_xy = np.sum(dx * dy)
    ss_xx = np.sum(dx * dx)
    ss_yy = np.sum(dy * dy)
    
    if ss_xx == 0:
        return 0.0, float(y_mean), 0.0
    slope = ss_xy / ss_xx
    intercept = y_mean - slope * x_mean
    r_squared = 0.0 if ss_yy == 0 else float((ss_xy * ss_xy) / (ss_xx * ss_yy))
    return float(slope), float(intercept), r_squared

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

# -----------------------------------------------------------------------------
# PARQUET DATA LOADER & 15M RESAMPLER
# -----------------------------------------------------------------------------

def load_symbol_parquet_2m(symbol):
    s_dir = os.path.join(PARQUET_DIR, symbol)
    if not os.path.exists(s_dir):
        return None, None
        
    files = sorted(glob.glob(os.path.join(s_dir, '*.parquet')))
    if not files or len(files) < 2:
        return None, None
        
    # Take the last 2 available month parquet files
    target_files = files[-2:]
    
    dfs = []
    for f in target_files:
        try:
            df = pd.read_parquet(f, columns=['open_time', 'open', 'high', 'low', 'close', 'volume', 'close_time'])
            dfs.append(df)
        except Exception:
            continue
            
    if not dfs:
        return None, None
        
    df_all = pd.concat(dfs, ignore_index=True)
    df_all.sort_values(by='open_time', inplace=True)
    df_all.drop_duplicates(subset=['open_time'], inplace=True)
    df_all.reset_index(drop=True, inplace=True)
    
    if len(df_all) < 5000:
        return None, None
        
    # Convert to list of dicts for fast 1m access
    records_1m = df_all.to_dict('records')
    
    # RESAMPLE 1M TO 15M KLINES
    df_all['group_15m'] = df_all['open_time'] // 900000 # 15 minutes = 900,000 ms
    
    agg_dict = {
        'open_time': 'first',
        'open': 'first',
        'high': 'max',
        'low': 'min',
        'close': 'last',
        'volume': 'sum',
        'close_time': 'last'
    }
    
    df_15m = df_all.groupby('group_15m').agg(agg_dict).reset_index(drop=True)
    records_15m = df_15m.to_dict('records')
    
    return records_1m, records_15m

def backtest_parquet_symbol(symbol, position_size_usdt=100.0, initial_capital_usdt=1000.0):
    records_1m, records_15m = load_symbol_parquet_2m(symbol)
    if not records_1m or not records_15m or len(records_15m) < 500:
        return None
        
    m1_dict = {k['open_time']: idx for idx, k in enumerate(records_1m)}
    
    start_idx_15m = 400
    if len(records_15m) <= start_idx_15m:
        return None
        
    trades = []
    active_trade = None
    current_equity = initial_capital_usdt
    peak_equity = initial_capital_usdt
    max_drawdown_usdt = 0.0
    
    for i in range(start_idx_15m, len(records_15m)):
        current_15m = records_15m[i]
        ts_15m = current_15m['open_time']
        
        if active_trade is not None:
            if ts_15m < active_trade['exit_time']:
                continue
            else:
                active_trade = None
                
        window_15m = records_15m[i-400:i+1]
        ats_15m = get_ats(window_15m)
        
        if abs(ats_15m) < 0.1:
            continue
            
        direction = "LONG" if ats_15m > 0 else "SHORT"
        
        if ts_15m not in m1_dict:
            continue
        idx_1m = m1_dict[ts_15m]
        if idx_1m < 400:
            continue
            
        window_1m = records_1m[idx_1m-400:idx_1m+1]
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
        
        max_forward = min(len(records_1m), idx_1m + 1 + 2880) # max 48h
        
        for f_idx in range(idx_1m + 1, max_forward):
            bar = records_1m[f_idx]
            
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
            if max_forward > idx_1m + 1:
                last_bar = records_1m[max_forward - 1]
                exit_price = last_bar['close']
                exit_time = last_bar['open_time']
                pnl_raw = ((exit_price - entry_price) / entry_price) if direction == "LONG" else ((entry_price - exit_price) / entry_price)
                result = "WIN" if pnl_raw > 0 else "LOSS"
            else:
                continue
            
        pnl_pct = ((exit_price - entry_price) / entry_price * 100.0) if direction == "LONG" else ((entry_price - exit_price) / entry_price * 100.0)
        
        gross_pnl_usdt = position_size_usdt * (pnl_pct / 100.0)
        taker_fee_usdt = position_size_usdt * 0.0008
        net_pnl_usdt = gross_pnl_usdt - taker_fee_usdt
        
        current_equity += net_pnl_usdt
        if current_equity > peak_equity:
            peak_equity = current_equity
        dd_usdt = peak_equity - current_equity
        if dd_usdt > max_drawdown_usdt:
            max_drawdown_usdt = dd_usdt
            
        trade_data = {
            'symbol': symbol,
            'entry_time': ts_15m,
            'exit_time': exit_time,
            'direction': direction,
            'entry_price': entry_price,
            'tp_price': tp_price,
            'sl_price': sl_price,
            'exit_price': exit_price,
            'result': result,
            'pnl_pct': pnl_pct,
            'net_pnl_usdt': net_pnl_usdt,
            'current_equity': current_equity
        }
        trades.append(trade_data)
        active_trade = trade_data
        
    total_trades = len(trades)
    wins = [t for t in trades if t['net_pnl_usdt'] > 0]
    losses = [t for t in trades if t['net_pnl_usdt'] <= 0]
    win_count = len(wins)
    loss_count = len(losses)
    win_rate = (win_count / total_trades * 100.0) if total_trades > 0 else 0.0
    total_pnl_usdt = sum(t['net_pnl_usdt'] for t in trades)
    
    return {
        'symbol': symbol,
        'trades': trades,
        'total_trades': total_trades,
        'win_count': win_count,
        'loss_count': loss_count,
        'win_rate': win_rate,
        'total_pnl_usdt': total_pnl_usdt,
        'ending_equity': current_equity,
        'max_drawdown_usdt': max_drawdown_usdt
    }

def main():
    print("==========================================================================================")
    print("🚀 YEREL PARQUET VERİSETİ İLE TÜM USDT PARİTELERİ - 2 AYLIK MS ANALYZER BACKTEST")
    print("📌 Veri Dizini: /home/smhvz/Desktop/cycle-orc/data/parquet_klines")
    print("==========================================================================================")
    
    symbols = [d for d in os.listdir(PARQUET_DIR) if d.endswith('USDT') and os.path.isdir(os.path.join(PARQUET_DIR, d))]
    symbols.sort()
    
    print(f"📌 Yerel Dizinde Bulunan Toplam USDT Parite Sayısı: {len(symbols)}")
    print("⏳ Parquet dosyaları okunuyor ve 15m mumlar üretilerek paralel analiz ediliyor...\n")
    
    results = []
    
    # ProcessPoolExecutor for CPU-bound pandas & numpy analytics
    with ProcessPoolExecutor(max_workers=os.cpu_count() or 8) as executor:
        future_to_sym = {
            executor.submit(backtest_parquet_symbol, sym): sym for sym in symbols
        }
        count = 0
        for future in as_completed(future_to_sym):
            sym = future_to_sym[future]
            count += 1
            try:
                res = future.result()
                if res and res['total_trades'] > 0:
                    results.append(res)
                    print(f"[{count:3d}/{len(symbols)}] ✅ {sym:<14} | İşlem: {res['total_trades']:<3} | Win Rate: %{res['win_rate']:<5.1f} | Net PnL: ${res['total_pnl_usdt']:<+8.2f}", flush=True)
                else:
                    if count % 20 == 0 or count == len(symbols):
                        print(f"[{count:3d}/{len(symbols)}] ... İşleniyor ...", flush=True)
            except Exception as e:
                print(f"[{count:3d}/{len(symbols)}] ❌ {sym:<14} | Hata: {e}", flush=True)

    if not results:
        print("❌ Hiçbir paritede işlem üretilemedi!")
        return

    total_symbols_traded = len(results)
    grand_total_trades = sum(r['total_trades'] for r in results)
    grand_wins = sum(r['win_count'] for r in results)
    grand_losses = sum(r['loss_count'] for r in results)
    grand_win_rate = (grand_wins / grand_total_trades * 100.0) if grand_total_trades > 0 else 0.0
    
    grand_total_net_pnl_usdt = sum(r['total_pnl_usdt'] for r in results)
    
    sorted_results = sorted(results, key=lambda x: x['total_pnl_usdt'], reverse=True)
    
    print("\n" + "=" * 115)
    print("📊 YEREL PARQUET TÜM USDT PARİTELERİ TOPLU BACKTEST ÖZETİ (SABİT $100 USDT / AYRI KASA)")
    print("=" * 115)
    print(f"• İşlem Gören Parite Sayısı   : {total_symbols_traded} / {len(symbols)}")
    print(f"• Toplam Açılan İşlem Sayısı  : {grand_total_trades}")
    print(f"• Toplam Kazanılan İşlem     : {grand_wins} (Genel Win Rate: %{grand_win_rate:.2f})")
    print(f"• Toplam Kaybedilen İşlem    : {grand_losses}")
    print(f"• PORTFÖY TOPLAM NET KÂR ($)  : ${grand_total_net_pnl_usdt:+,.2f} USDT")
    print(f"• Parite Başı Ortalama Kâr   : ${grand_total_net_pnl_usdt / total_symbols_traded:+,.2f} USDT")
    print("=" * 115)
    
    print("\n🏆 EN ÇOK KÂR ETTİREN İLK 20 PARİTE:")
    print("-" * 95)
    print(f"{'#':<3} | {'Parite':<14} | {'İşlem Sayısı':<12} | {'Kazanma Oranı (%)':<18} | {'Net Kâr/Zarar ($)':<18} | {'Bitiş Bakiyesi ($)':<16}")
    print("-" * 95)
    for idx, r in enumerate(sorted_results[:20], 1):
        print(f"{idx:<3} | {r['symbol']:<14} | {r['total_trades']:<12} | %{r['win_rate']:<17.2f} | ${r['total_pnl_usdt']:<+17.2f} | ${r['ending_equity']:<16.2f}")
    print("-" * 95)

    print("\n⚠️ EN ÇOK ZARAR ETTİREN SON 15 PARİTE:")
    print("-" * 95)
    print(f"{'#':<3} | {'Parite':<14} | {'İşlem Sayısı':<12} | {'Kazanma Oranı (%)':<18} | {'Net Kâr/Zarar ($)':<18} | {'Bitiş Bakiyesi ($)':<16}")
    print("-" * 95)
    for idx, r in enumerate(sorted_results[-15:], 1):
        print(f"{idx:<3} | {r['symbol']:<14} | {r['total_trades']:<12} | %{r['win_rate']:<17.2f} | ${r['total_pnl_usdt']:<+17.2f} | ${r['ending_equity']:<16.2f}")
    print("-" * 95)

    report_data = {
        'total_symbols_traded': total_symbols_traded,
        'grand_total_trades': grand_total_trades,
        'grand_wins': grand_wins,
        'grand_losses': grand_losses,
        'grand_win_rate': grand_win_rate,
        'grand_total_net_pnl_usdt': grand_total_net_pnl_usdt,
        'symbol_summary': [{
            'symbol': r['symbol'],
            'total_trades': r['total_trades'],
            'win_rate': r['win_rate'],
            'total_pnl_usdt': r['total_pnl_usdt'],
            'ending_equity': r['ending_equity']
        } for r in sorted_results]
    }
    
    with open("/home/smhvz/Desktop/cycle-orc/data/parquet_usdt_2m_backtest_report.json", "w") as f:
        json.dump(report_data, f, indent=2)

if __name__ == "__main__":
    main()
