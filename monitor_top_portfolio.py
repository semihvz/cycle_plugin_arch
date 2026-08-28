#!/usr/bin/env python3
"""
Çoklu Pozisyon Portföy Denetleyici ve Canlı PnL Takipçisi (Multi-Position Live Monitor)
--------------------------------------------------------------------------------------
En yüksek sinyali veren Top 3 LONG ve Top 3 SHORT pozisyonunu (Toplam 6 pozisyon)
her 5 saniyede bir eşzamanlı takip eder, bireysel ve toplam PnL durumunu raporlar.
"""

import os
import sys
import time
import json
import argparse
import datetime
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed


def fetch_current_ticker(symbol):
    """Binance Futures anlık mark/kapanış fiyatını çeker."""
    url = f"https://fapi.binance.com/fapi/v1/ticker/price?symbol={symbol}"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=5) as resp:
                data = json.loads(resp.read().decode('utf-8'))
            return float(data['price'])
        except Exception:
            time.sleep(0.5)
    return None


class MultiPositionMonitor:
    def __init__(self, positions, check_interval_sec=5):
        self.positions = positions  # List of dicts
        self.check_interval = int(check_interval_sec)
        self.start_time = datetime.datetime.now(datetime.timezone.utc)
        self.is_active = True
        self.check_count = 0

        base_dir = "/home/smhvz/Desktop/cycle-orc"
        self.log_file = os.path.join(base_dir, "top_portfolio_monitor.log")

    def print_header(self):
        print("==========================================================================================")
        print("🛡️ ÇOKLU PORTFÖY CANLI DENETLEME VE PNL TAKİP SİSTEMİ (TOP 3 LONG & TOP 3 SHORT)")
        print(f"🕒 Takip Başlatma Zamanı: {self.start_time.strftime('%Y-%m-%d %H:%M:%S UTC')}")
        print("==========================================================================================")
        print(f"{'Sıra':<4} | {'Sembol':<12} | {'Yön':<6} | {'Giriş Fiyatı':<12} | {'Stop Loss (SL)':<14} | {'Take Profit (TP)':<14} | {'Poz. Büyüklüğü':<14}")
        print("-" * 90)
        for i, pos in enumerate(self.positions):
            direction = "🟢 LONG" if pos['side'] == "LONG" else "🔴 SHORT"
            print(f"{i+1:<4} | {pos['symbol']:<12} | {direction:<6} | {pos['entry']:<12.6f} | {pos['sl']:<14.6f} | {pos['tp']:<14.6f} | ${pos['size']:.2f} USDT")
        print("------------------------------------------------------------------------------------------\n")

    def audit_cycle(self):
        self.check_count += 1
        curr_time = datetime.datetime.now(datetime.timezone.utc)
        elapsed_delta = curr_time - self.start_time
        elapsed_mins = int(elapsed_delta.total_seconds() // 60)

        # Paralel fiyat çekme
        price_map = {}
        active_symbols = [p['symbol'] for p in self.positions if p['active']]

        if not active_symbols:
            print("\n🎉 Tüm pozisyonlar kapandı! Portföy denetimi tamamlandı.")
            self.is_active = False
            return

        with ThreadPoolExecutor(max_workers=10) as executor:
            futures = {executor.submit(fetch_current_ticker, sym): sym for sym in active_symbols}
            for future in as_completed(futures):
                sym = futures[future]
                price = future.result()
                if price:
                    price_map[sym] = price

        total_pnl_usdt = 0.0
        total_pos_value = 0.0

        print(f"\n⏱️ [{curr_time.strftime('%H:%M:%S UTC')}] Portföy Denetimi #{self.check_count} (Geçen Süre: {elapsed_mins} dk):")
        print(f"{'Sembol':<12} | {'Yön':<6} | {'Anlık Fiyat':<14} | {'Anlık PnL ($)':<16} | {'PnL (%)':<10} | {'TP Kalan':<10} | {'SL Kalan':<10} | {'Durum':<14}")
        print("-" * 108)

        for pos in self.positions:
            if not pos['active']:
                print(f"{pos['symbol']:<12} | {pos['side']:<6} | {'KAPANDI':<14} | {pos['final_pnl_usdt']:<+16.4f} | {pos['final_pnl_pct']:<+9.2f}% | {'---':<10} | {'---':<10} | {pos['close_reason']:<14}")
                total_pnl_usdt += pos['final_pnl_usdt']
                total_pos_value += pos['size']
                continue

            sym = pos['symbol']
            curr_price = price_map.get(sym, pos['entry'])

            # PnL hesaplama
            if pos['side'] == "LONG":
                pnl_pct = ((curr_price - pos['entry']) / pos['entry']) * 100.0
                pnl_usdt = pos['size'] * (pnl_pct / 100.0)
                dist_tp_pct = ((pos['tp'] - curr_price) / curr_price) * 100.0
                dist_sl_pct = ((curr_price - pos['sl']) / curr_price) * 100.0
            else: # SHORT
                pnl_pct = ((pos['entry'] - curr_price) / pos['entry']) * 100.0
                pnl_usdt = pos['size'] * (pnl_pct / 100.0)
                dist_tp_pct = ((curr_price - pos['tp']) / curr_price) * 100.0
                dist_sl_pct = ((pos['sl'] - curr_price) / curr_price) * 100.0

            total_pnl_usdt += pnl_usdt
            total_pos_value += pos['size']

            # TP/SL Kontrolü
            close_reason = None
            if pos['side'] == "LONG":
                if curr_price >= pos['tp']:
                    close_reason = "🎯 TP (WIN)"
                elif curr_price <= pos['sl']:
                    close_reason = "🛑 SL (LOSS)"
            else:
                if curr_price <= pos['tp']:
                    close_reason = "🎯 TP (WIN)"
                elif curr_price >= pos['sl']:
                    close_reason = "🛑 SL (LOSS)"

            if close_reason:
                pos['active'] = False
                pos['close_reason'] = close_reason
                pos['final_pnl_usdt'] = pnl_usdt
                pos['final_pnl_pct'] = pnl_pct
                status_str = f"🎉 {close_reason}"
            else:
                status_str = "🟢 AÇIK"

            pnl_str = f"{pnl_usdt:+.4f} USDT"
            dir_icon = "🟢 LONG" if pos['side'] == "LONG" else "🔴 SHORT"
            print(f"{sym:<12} | {dir_icon:<6} | {curr_price:<14.6f} | {pnl_str:<16} | %{pnl_pct:<+8.2f} | %{dist_tp_pct:<8.2f} | %{dist_sl_pct:<8.2f} | {status_str:<14}")

        total_pnl_pct = (total_pnl_usdt / total_pos_value) * 100.0 if total_pos_value > 0 else 0.0
        pnl_symbol = "🟢 +" if total_pnl_usdt >= 0 else "🔴 "

        print("-" * 95)
        print(f"💰 PORTFÖY TOPLAM PNL: {pnl_symbol}{total_pnl_usdt:+.4f} USDT (%{total_pnl_pct:+.2f}) | Toplam Pozisyon: ${total_pos_value:.2f} USDT")
        print("------------------------------------------------------------------------------------------")

        # Loglama
        log_entry = {
            'timestamp': curr_time.strftime('%Y-%m-%d %H:%M:%S'),
            'total_pnl_usdt': round(total_pnl_usdt, 4),
            'total_pnl_pct': round(total_pnl_pct, 2),
            'prices': price_map
        }
        with open(self.log_file, "a") as f:
            f.write(json.dumps(log_entry) + "\n")

    def start_monitoring(self, iterations=None):
        self.print_header()
        count = 0
        try:
            while self.is_active:
                self.audit_cycle()
                if not self.is_active:
                    break
                count += 1
                if iterations and count >= iterations:
                    print(f"\n📌 {iterations} denetim döngüsü tamamlandı.")
                    break
                time.sleep(self.check_interval)
        except KeyboardInterrupt:
            print("\n🛑 Portföy izleme kullanıcı tarafından durduruldu.")


def main():
    parser = argparse.ArgumentParser(description="Top 3 LONG & Top 3 SHORT Portföy İzleyici")
    parser.add_argument("--interval", type=int, default=5, help="Saniye cinsinden denetim aralığı (Varsayılan: 5s)")
    parser.add_argument("--iterations", type=int, default=None, help="Çalıştırılacak döngü sayısı (Opsiyonel)")
    args = parser.parse_args()

    # Top 3 LONG ve Top 3 SHORT Pozisyonları
    portfolio_positions = [
        # TOP 3 LONG POSITIONS
        {'symbol': 'FLOCKUSDT', 'side': 'LONG', 'entry': 0.031980, 'sl': 0.031733, 'tp': 0.032474, 'size': 50.0, 'active': True, 'close_reason': None, 'final_pnl_usdt': 0.0, 'final_pnl_pct': 0.0},
        {'symbol': 'VTHOUSDT',  'side': 'LONG', 'entry': 0.000391, 'sl': 0.000389, 'tp': 0.000395, 'size': 50.0, 'active': True, 'close_reason': None, 'final_pnl_usdt': 0.0, 'final_pnl_pct': 0.0},
        {'symbol': 'FILUSDT',   'side': 'LONG', 'entry': 0.679800, 'sl': 0.674100, 'tp': 0.691200, 'size': 50.0, 'active': True, 'close_reason': None, 'final_pnl_usdt': 0.0, 'final_pnl_pct': 0.0},
        # TOP 3 SHORT POSITIONS
        {'symbol': 'LINKUSDT',     'side': 'SHORT', 'entry': 11.345000, 'sl': 11.433000, 'tp': 11.169000, 'size': 50.0, 'active': True, 'close_reason': None, 'final_pnl_usdt': 0.0, 'final_pnl_pct': 0.0},
        {'symbol': '1000SHIBUSDT', 'side': 'SHORT', 'entry': 0.005149,   'sl': 0.005194,  'tp': 0.005060,  'size': 50.0, 'active': True, 'close_reason': None, 'final_pnl_usdt': 0.0, 'final_pnl_pct': 0.0},
        {'symbol': 'MOCAUSDT',     'side': 'SHORT', 'entry': 0.008451,   'sl': 0.008524,  'tp': 0.008306,  'size': 50.0, 'active': True, 'close_reason': None, 'final_pnl_usdt': 0.0, 'final_pnl_pct': 0.0},
    ]

    monitor = MultiPositionMonitor(portfolio_positions, check_interval_sec=args.interval)
    monitor.start_monitoring(iterations=args.iterations)


if __name__ == "__main__":
    main()
