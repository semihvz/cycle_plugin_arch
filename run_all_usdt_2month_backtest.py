#!/usr/bin/env python3
import subprocess
import json
import time
import math
import datetime
import sqlite3
import os
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed

DB_DIR = "/home/smhvz/Desktop/cycle-orc/data/multi_symbol_backtest"
os.makedirs(DB_DIR, exist_ok=True)
DB_PATH = os.path.join(DB_DIR, "all_usdt_backtest.db")

HEADERS = {
    'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36',
    'Accept': 'application/json',
    'Accept-Language': 'en-US,en;q=0.9',
}

def curl_json(url):
    cmd = ['curl', '-s', '-H', f'User-Agent: {HEADERS["User-Agent"]}', url]
    try:
        out = subprocess.check_output(cmd, timeout=10)
        return json.loads(out.decode('utf-8'))
    except Exception as e:
        return None

def get_top_volume_usdt_symbols(min_24h_vol_usd=20_000_000):
    url = 'https://fapi.binance.com/fapi/v1/ticker/24hr'
    tickers = curl_json(url)
    if not tickers or not isinstance(tickers, list):
        tickers = curl_json('https://api.binance.com/api/v3/ticker/24hr')
        
    symbol_vols = []
    if tickers and isinstance(tickers, list):
        for t in tickers:
            sym = t.get('symbol', '')
            vol = float(t.get('quoteVolume', 0))
            if sym.endswith('USDT') and vol >= min_24h_vol_usd and not sym.startswith('DEF') and not sym.startswith('BTCDOM') and '_' not in sym:
                symbol_vols.append((sym, vol))
                
    symbol_vols.sort(key=lambda x: x[1], reverse=True)
    return [s[0] for s in symbol_vols]

def fetch_binance_klines(symbol, interval, start_time, end_time, limit=1500):
    klines = []
    curr_start = start_time
    
    hosts = [
        ("https://fapi.binance.com", "/fapi/v1/klines"),
        ("https://data-api.binance.vision", "/api/v3/klines"),
        ("https://api.binance.com", "/api/v3/klines")
    ]
    
    attempts = 0
    while curr_start < end_time and attempts < 6:
        fetched_batch = False
        for base, path in hosts:
            url = f"{base}{path}?symbol={symbol}&interval={interval}&startTime={curr_start}&limit={limit}"
            data = curl_json(url)
            if data and isinstance(data, list):
                if len(data) == 0:
                    fetched_batch = True
                    break
                for row in data:
                    klines.append((
                        symbol, int(row[0]), float(row[1]), float(row[2]), float(row[3]),
                        float(row[4]), float(row[5]), int(row[6])
                    ))
                last_ts = int(data[-1][0])
                if last_ts <= curr_start:
                    fetched_batch = True
                    break
                curr_start = last_ts + 1
                fetched_batch = True
                attempts = 0
                time.sleep(0.01)
                break
                
        if not fetched_batch:
            attempts += 1
            time.sleep(0.15)
            
    return klines

# -----------------------------------------------------------------------------
# MSMP 2.0 ANALYZER ENGINE
# -----------------------------------------------------------------------------

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

def backtest_single_symbol(symbol, start_2m_ms, now_ms, position_size_usdt=100.0, initial_capital_usdt=1000.0):
    k15_tuples = fetch_binance_klines(symbol, "15m", start_2m_ms - (400 * 15 * 60 * 1000), now_ms)
    if not k15_tuples or len(k15_tuples) < 3000:
        return None
        
    klines_15m = [{
        'open_time': r[1], 'open': r[2], 'high': r[3], 'low': r[4],
        'close': r[5], 'volume': r[6], 'close_time': r[7]
    } for r in k15_tuples]
    
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
    
    cached_1m_dict = {}
    cached_1m_list = []
    
    def get_1m_window(timestamp_ms):
        nonlocal cached_1m_list, cached_1m_dict
        if timestamp_ms not in cached_1m_dict:
            f_start = timestamp_ms - (400 * 60 * 1000)
            f_end = timestamp_ms + (2880 * 60 * 1000)
            k1_tuples = fetch_binance_klines(symbol, "1m", f_start, f_end)
            cached_1m_list = [{
                'open_time': r[1], 'open': r[2], 'high': r[3], 'low': r[4],
                'close': r[5], 'volume': r[6], 'close_time': r[7]
            } for r in k1_tuples]
            cached_1m_dict = {k['open_time']: idx for idx, k in enumerate(cached_1m_list)}
            
        if timestamp_ms in cached_1m_dict:
            idx = cached_1m_dict[timestamp_ms]
            if idx >= 400:
                return cached_1m_list[idx-400:idx+1], cached_1m_list, idx
        return None, None, None

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
        
        window_1m, full_1m_list, idx_1m = get_1m_window(ts_15m)
        if window_1m is None:
            continue
            
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
        
        max_forward = min(len(full_1m_list), idx_1m + 1 + 2880)
        
        for f_idx in range(idx_1m + 1, max_forward):
            bar = full_1m_list[f_idx]
            
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
                last_bar = full_1m_list[max_forward - 1]
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
    print("🚀 YÜKSEK HACİMLİ USDT PARİTELERİ - SON 2 AYLIK MS ANALYZER MULTI-TIMEFRAME BACKTEST")
    print("==========================================================================================")
    
    symbols = get_top_volume_usdt_symbols(min_24h_vol_usd=20_000_000)
    # Take top 60 highest volume symbols for fast & comprehensive backtest
    symbols = symbols[:60]
    print(f"📌 Analiz Edilecek En Yüksek Hacimli (>20M$ 24h Vol) Parite Sayısı: {len(symbols)}")
    print("Örnek Pariteler:", symbols[:10])
    
    now_ms = int(time.time() * 1000)
    start_2m_ms = now_ms - (60 * 24 * 60 * 60 * 1000)
    
    results = []
    
    print("\n⏳ 12 Paralel Worker ile Pariteler İşleniyor...\n")
    with ThreadPoolExecutor(max_workers=12) as executor:
        future_to_sym = {
            executor.submit(backtest_single_symbol, sym, start_2m_ms, now_ms): sym for sym in symbols
        }
        count = 0
        for future in as_completed(future_to_sym):
            sym = future_to_sym[future]
            count += 1
            try:
                res = future.result()
                if res and res['total_trades'] > 0:
                    results.append(res)
                    print(f"[{count:2d}/{len(symbols)}] ✅ {sym:<12} | İşlem: {res['total_trades']:<3} | Win Rate: %{res['win_rate']:<5.1f} | Net PnL: ${res['total_pnl_usdt']:<+8.2f}", flush=True)
                else:
                    print(f"[{count:2d}/{len(symbols)}] ⚪ {sym:<12} | İşlem üretilmedi / Yetersiz 2 aylık veri", flush=True)
            except Exception as e:
                print(f"[{count:2d}/{len(symbols)}] ❌ {sym:<12} | Hata: {e}", flush=True)

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
    print("📊 YÜKSEK HACİMLİ PARİTELER TOPLU BACKTEST ÖZETİ (SABİT $100 USDT / AYRI KASA)")
    print("=" * 115)
    print(f"• İşlem Gören Parite Sayısı   : {total_symbols_traded} / {len(symbols)}")
    print(f"• Toplam Açılan İşlem Sayısı  : {grand_total_trades}")
    print(f"• Toplam Kazanılan İşlem     : {grand_wins} (Genel Win Rate: %{grand_win_rate:.2f})")
    print(f"• Toplam Kaybedilen İşlem    : {grand_losses}")
    print(f"• PORTFÖY TOPLAM NET KÂR ($)  : ${grand_total_net_pnl_usdt:+,.2f} USDT")
    print(f"• Parite Başı Ortalama Kâr   : ${grand_total_net_pnl_usdt / total_symbols_traded:+,.2f} USDT")
    print("=" * 115)
    
    print("\n🏆 EN ÇOK KÂR ETTİREN İLK 15 PARİTE:")
    print("-" * 95)
    print(f"{'#':<3} | {'Parite':<12} | {'İşlem Sayısı':<12} | {'Kazanma Oranı (%)':<18} | {'Net Kâr/Zarar ($)':<18} | {'Bitiş Bakiyesi ($)':<16}")
    print("-" * 95)
    for idx, r in enumerate(sorted_results[:15], 1):
        print(f"{idx:<3} | {r['symbol']:<12} | {r['total_trades']:<12} | %{r['win_rate']:<17.2f} | ${r['total_pnl_usdt']:<+17.2f} | ${r['ending_equity']:<16.2f}")
    print("-" * 95)

    print("\n⚠️ EN ÇOK ZARAR ETTİREN SON 10 PARİTE:")
    print("-" * 95)
    print(f"{'#':<3} | {'Parite':<12} | {'İşlem Sayısı':<12} | {'Kazanma Oranı (%)':<18} | {'Net Kâr/Zarar ($)':<18} | {'Bitiş Bakiyesi ($)':<16}")
    print("-" * 95)
    for idx, r in enumerate(sorted_results[-10:], 1):
        print(f"{idx:<3} | {r['symbol']:<12} | {r['total_trades']:<12} | %{r['win_rate']:<17.2f} | ${r['total_pnl_usdt']:<+17.2f} | ${r['ending_equity']:<16.2f}")
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
    
    with open("/home/smhvz/Desktop/cycle-orc/data/all_usdt_2m_backtest_report.json", "w") as f:
        json.dump(report_data, f, indent=2)

if __name__ == "__main__":
    main()
