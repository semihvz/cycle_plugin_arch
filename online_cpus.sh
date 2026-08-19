#!/bin/bash
# -------------------------------------------------------------
# Script to restore / online all CPU cores
# -------------------------------------------------------------

if [ "$EUID" -ne 0 ]; then
  echo "Lütfen bu betiği root (sudo) yetkisi ile çalıştırın:"
  echo "  sudo $0"
  exit 1
fi

echo "Tüm CPU Çekirdekleri Yeniden Açılıyor..."

TOTAL_CPUS=$(nproc --all)

for i in $(seq 2 $((TOTAL_CPUS - 1))); do
  if [ -f "/sys/devices/system/cpu/cpu$i/online" ]; then
    echo 1 > "/sys/devices/system/cpu/cpu$i/online"
    echo "✓ CPU $i açıldı (online)"
  fi
done

echo ""
echo "=== DURUM ==="
echo "Aktif Çekirdekler: $(cat /sys/devices/system/cpu/online)"
