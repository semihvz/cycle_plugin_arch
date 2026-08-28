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
from concurrent.futures import ThreadPoolExecutor, as_completed
from run_strict_ats_backtest import backtest_parquet_symbol_strict, PARQUET_DIR

EXCEL_OUT_DIR = "/home/smhvz/Desktop/cycle-orc/data/strict_ats_trade_logs_excel"
os.makedirs(EXCEL_OUT_DIR, exist_ok=True)

def process_and_save_strict_symbol_excel(symbol):
    res = backtest_parquet_symbol_strict(symbol, ats_threshold_15m=1.5, ats_threshold_1m=0.5, position_size_usdt=100.0, initial_capital_usdt=100.0)
    if not res or not res['trades']:
        return None
        
    trades = res['trades']
    df = pd.DataFrame(trades)
    
    df['Giriş Tarihi'] = df['entry_time'].apply(lambda ts: datetime.datetime.fromtimestamp(ts/1000, datetime.timezone.utc).strftime('%Y-%m-%d %H:%M'))
    df['Çıkış Tarihi'] = df['exit_time'].apply(lambda ts: datetime.datetime.fromtimestamp(ts/1000, datetime.timezone.utc).strftime('%Y-%m-%d %H:%M'))
    
    cols_rename = {
        'symbol': 'Parite',
        'direction': 'Yön',
        'entry_price': 'Giriş Fiyatı',
        'tp_price': 'TP Fiyatı',
        'sl_price': 'SL Fiyatı',
        'exit_price': 'Çıkış Fiyatı',
        'result': 'Sonuç',
        'pnl_pct': 'Fiyat Değişimi (%)',
        'net_pnl_usdt': 'Net PnL ($)',
        'current_equity': 'Kasa Bakiyesi ($)'
    }
    
    df.rename(columns=cols_rename, inplace=True)
    cols_order = ['Parite', 'Giriş Tarihi', 'Çıkış Tarihi', 'Yön', 'Giriş Fiyatı', 'TP Fiyatı', 'SL Fiyatı', 'Çıkış Fiyatı', 'Sonuç', 'Fiyat Değişimi (%)', 'Net PnL ($)', 'Kasa Bakiyesi ($)']
    df = df[cols_order]
    
    out_file = os.path.join(EXCEL_OUT_DIR, f"{symbol}_islem_kayitlari.xlsx")
    df.to_excel(out_file, index=False)
    return symbol, len(trades), out_file, df

def main():
    symbols = [d for d in os.listdir(PARQUET_DIR) if d.endswith('USDT') and os.path.isdir(os.path.join(PARQUET_DIR, d))]
    symbols.sort()

    print("==========================================================================================")
    print(f"📊 SIKI ATS FİLTRELİ 653 PARİTENİN TÜM İŞLEM KAYITLARI EXCEL DOSYALARINA AKTARILIYOR")
    print(f"📁 Hedef Dizin: {EXCEL_OUT_DIR}")
    print("==========================================================================================")

    saved_count = 0
    total_all_trades = 0
    all_dfs = []

    with ThreadPoolExecutor(max_workers=16) as executor:
        futures = {executor.submit(process_and_save_strict_symbol_excel, sym): sym for sym in symbols}
        for future in as_completed(futures):
            sym = futures[future]
            try:
                res = future.result()
                if res:
                    s_name, t_count, out_f, df_single = res
                    saved_count += 1
                    total_all_trades += t_count
                    all_dfs.append(df_single)
                    if saved_count % 30 == 0 or saved_count == 1:
                        print(f"[{saved_count:3d}/653] ✅ {s_name:<14} -> {t_count:<3} işlem kaydedildi: {os.path.basename(out_f)}")
            except Exception as e:
                pass

    print("==========================================================================================")
    print(f"🎉 Toplam {saved_count} parite için {total_all_trades:,} adet işlem Excel dosyalarına başarıyla yazıldı!")

    if all_dfs:
        print("⏳ Tüm işlemler birleşik tek master Excel dosyası oluşturuluyor...")
        master_df = pd.concat(all_dfs, ignore_index=True)
        master_file = "/home/smhvz/Desktop/cycle-orc/strict_ats_1.5_all_trades_master.xlsx"
        master_df.to_excel(master_file, index=False)
        print(f"📂 BİRLEŞİK MASTER EXCEL DOSYASI: {master_file}")
    print("==========================================================================================")

if __name__ == "__main__":
    main()
