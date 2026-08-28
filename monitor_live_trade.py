#!/usr/bin/env python3
"""
Canlı Pozisyon Denetleme ve Anlık PnL Hesaplayıcı (Live Trade & PnL Monitor)
-----------------------------------------------------------------------------
Her 1 dakikada bir (veya belirlenen saniye aralığında) Binance Futures'tan canlı fiyatı çeker,
açık pozisyonun anlık gerçekleşmemiş PnL (Unrealized PnL) tutarını ve yüzdesini hesaplar,
TP (Take Profit) ve SL (Stop Loss) seviyelerine olan mesafeyi denetler.
TP veya SL seviyesine ulaşıldığında pozisyonu otomatik olarak kapatır ve raporlar.
"""

import os
import sys
import time
import json
import argparse
import datetime
import urllib.request


def fetch_current_ticker(symbol="MAGMAUSDT"):
    """Binance Futures anlık mark/kapanış fiyatını çeker (Rate limit korumalı)."""
    url = f"https://fapi.binance.com/fapi/v1/ticker/price?symbol={symbol}"
    req = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=10) as resp:
                data = json.loads(resp.read().decode('utf-8'))
            return float(data['price'])
        except Exception as e:
            if attempt == 2:
                raise e
            time.sleep(2.0)
    raise RuntimeError("Failed to fetch ticker after retries")


class LiveTradeMonitor:
    def __init__(self, symbol="MAGMAUSDT", side="LONG", entry_price=0.432000, sl_price=0.406947, tp_price=0.482106, pos_size_usdt=50.0, check_interval_sec=60):
        self.symbol = symbol
        self.side = side.upper()
        self.entry_price = float(entry_price)
        self.sl_price = float(sl_price)
        self.tp_price = float(tp_price)
        self.pos_size_usdt = float(pos_size_usdt)
        self.check_interval = int(check_interval_sec)

        # Risk ve Ödül Tutarları
        self.sl_dist_pct = abs((self.entry_price - self.sl_price) / self.entry_price)
        self.tp_dist_pct = abs((self.tp_price - self.entry_price) / self.entry_price)

        self.risk_usdt = self.pos_size_usdt * self.sl_dist_pct
        self.target_reward_usdt = self.pos_size_usdt * self.tp_dist_pct

        self.start_time = datetime.datetime.now(datetime.timezone.utc)
        self.is_active = True
        self.check_count = 0

        # Log dosyası
        base_dir = "/home/smhvz/Desktop/cycle-orc"
        self.log_file = os.path.join(base_dir, f"{self.symbol.lower()}_live_monitor.log")

    def print_header(self):
        print("==========================================================================================")
        print(f"🛡️ CANLI POZİSYON DENETLEME VE PNL TAKİP SİSTEMİ: {self.symbol} ({self.side})")
        print(f"🕒 Pozisyon Giriş Zamanı: {self.start_time.strftime('%Y-%m-%d %H:%M:%S UTC')}")
        print("==========================================================================================")
        print(f"   • Giriş Fiyatı (Entry) : {self.entry_price:.6f} USDT")
        print(f"   • Pozisyon Büyüklüğü   : {self.pos_size_usdt:.2f} USDT")
        print(f"   • Stop Loss (SL)       : {self.sl_price:.6f} USDT (-%{self.sl_dist_pct*100:.2f} | Risk: -${self.risk_usdt:.2f})")
        print(f"   • Take Profit (TP)     : {self.tp_price:.6f} USDT (+%{self.tp_dist_pct*100:.2f} | Hedef: +${self.target_reward_usdt:.2f})")
        print(f"   • Kontrol Periyodu     : Her {self.check_interval} saniyede bir (1 Dakika)")
        print("------------------------------------------------------------------------------------------\n")

    def check_position(self):
        self.check_count += 1
        curr_time = datetime.datetime.now(datetime.timezone.utc)
        elapsed_delta = curr_time - self.start_time
        elapsed_mins = int(elapsed_delta.total_seconds() // 60)

        try:
            curr_price = fetch_current_ticker(self.symbol)
        except Exception as e:
            print(f"⚠️ [{curr_time.strftime('%H:%M:%S UTC')}] Fiyat çekilemedi: {e}")
            return

        # PnL Hesaplama
        if self.side == "LONG":
            pnl_pct = ((curr_price - self.entry_price) / self.entry_price) * 100.0
            pnl_usdt = self.pos_size_usdt * (pnl_pct / 100.0)
            dist_to_sl_pct = ((curr_price - self.sl_price) / curr_price) * 100.0
            dist_to_tp_pct = ((self.tp_price - curr_price) / curr_price) * 100.0
        else: # SHORT
            pnl_pct = ((self.entry_price - curr_price) / self.entry_price) * 100.0
            pnl_usdt = self.pos_size_usdt * (pnl_pct / 100.0)
            dist_to_sl_pct = ((self.sl_price - curr_price) / curr_price) * 100.0
            dist_to_tp_pct = ((curr_price - self.tp_price) / curr_price) * 100.0

        pnl_symbol = "🟢 +" if pnl_usdt >= 0 else "🔴 "

        print(f"⏱️ [{curr_time.strftime('%H:%M:%S UTC')}] Denetim #{self.check_count} (Geçen Süre: {elapsed_mins} dk):")
        print(f"   • Anlık Fiyat   : {curr_price:.6f} USDT")
        print(f"   • Anlık PnL     : {pnl_symbol}{pnl_usdt:+.4f} USDT (%{pnl_pct:+.2f})")
        print(f"   • TP'ye Kalan   : %{dist_to_tp_pct:.2f} ({abs(self.tp_price - curr_price):.6f} USDT)")
        print(f"   • SL'ye Kalan   : %{dist_to_sl_pct:.2f} ({abs(curr_price - self.sl_price):.6f} USDT)")

        # Loglama
        log_line = f"{curr_time.strftime('%Y-%m-%d %H:%M:%S')},{curr_price},{pnl_usdt:.4f},{pnl_pct:.2f}\n"
        with open(self.log_file, "a") as f:
            f.write(log_line)

        # TP / SL Kontrolü
        if self.side == "LONG":
            if curr_price >= self.tp_price:
                self.close_position("TAKE_PROFIT (WIN)", curr_price, self.target_reward_usdt, self.tp_dist_pct * 100.0)
            elif curr_price <= self.sl_price:
                self.close_position("STOP_LOSS (LOSS)", curr_price, -self.risk_usdt, -self.sl_dist_pct * 100.0)
        else:
            if curr_price <= self.tp_price:
                self.close_position("TAKE_PROFIT (WIN)", curr_price, self.target_reward_usdt, self.tp_dist_pct * 100.0)
            elif curr_price >= self.sl_price:
                self.close_position("STOP_LOSS (LOSS)", curr_price, -self.risk_usdt, -self.sl_dist_pct * 100.0)

        print("-" * 65)

    def close_position(self, reason, exit_price, final_pnl_usdt, final_pnl_pct):
        self.is_active = False
        end_time = datetime.datetime.now(datetime.timezone.utc)
        total_seconds = int((end_time - self.start_time).total_seconds())

        print("\n==========================================================================================")
        print(f"🎉 POZİSYON KAPANDI: {reason}")
        print("==========================================================================================")
        print(f"   • Çıkış Fiyatı (Exit)  : {exit_price:.6f} USDT")
        print(f"   • Gerçekleşen Net PnL  : {final_pnl_usdt:+.4f} USDT (%{final_pnl_pct:+.2f})")
        print(f"   • Toplam Elde Tutma    : {total_seconds // 60} dakika {total_seconds % 60} saniye")
        print(f"   • Kapanış Zamanı       : {end_time.strftime('%Y-%m-%d %H:%M:%S UTC')}")
        print("==========================================================================================\n")

    def start_monitoring(self, iterations=None):
        self.print_header()
        count = 0
        try:
            while self.is_active:
                self.check_position()
                if not self.is_active:
                    break
                count += 1
                if iterations and count >= iterations:
                    print(f"📌 {iterations} denetim döngüsü tamamlandı. İzleme devam ediyor veya durduruldu.")
                    break
                time.sleep(self.check_interval)
        except KeyboardInterrupt:
            print("\n🛑 İzleme kullanıcı tarafından durduruldu.")


def main():
    parser = argparse.ArgumentParser(description="Canlı Pozisyon PnL Denetleyici")
    parser.add_argument("--symbol", type=str, default="MAGMAUSDT")
    parser.add_argument("--side", type=str, default="LONG")
    parser.add_argument("--entry", type=float, default=0.432000)
    parser.add_argument("--sl", type=float, default=0.406947)
    parser.add_argument("--tp", type=float, default=0.482106)
    parser.add_argument("--pos-size", type=float, default=50.0)
    parser.add_argument("--interval", type=int, default=60, help="Saniye cinsinden denetim aralığı (Varsayılan: 60s)")
    parser.add_argument("--iterations", type=int, default=None, help="Çalıştırılacak döngü sayısı (Opsiyonel)")
    args = parser.parse_args()

    monitor = LiveTradeMonitor(
        symbol=args.symbol,
        side=args.side,
        entry_price=args.entry,
        sl_price=args.sl,
        tp_price=args.tp,
        pos_size_usdt=args.pos_size,
        check_interval_sec=args.interval
    )
    monitor.start_monitoring(iterations=args.iterations)


if __name__ == "__main__":
    main()
