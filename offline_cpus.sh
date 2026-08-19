#!/bin/bash
# -------------------------------------------------------------
# Script to offline CPU cores 2 through 15 (keeping cpu0 and cpu1 active)
# -------------------------------------------------------------

if [ "$EUID" -ne 0 ]; then
  echo "Lütfen bu betiği root (sudo) yetkisi ile çalıştırın:"
  echo "  sudo $0"
  exit 1
fi

echo "CPU Çekirdekleri Pasife Alınıyor (Sadece CPU 0 ve 1 Aktif Bırakılıyor)..."

TOTAL_CPUS=$(nproc --all)
OFFLINED=0

for i in $(seq 2 $((TOTAL_CPUS - 1))); do
  if [ -f "/sys/devices/system/cpu/cpu$i/online" ]; then
    echo 0 > "/sys/devices/system/cpu/cpu$i/online"
    echo "✓ CPU $i kapatıldı (offline)"
    OFFLINED=$((OFFLINED + 1))
  fi
done

echo ""
echo "=== DURUM ==="
echo "Aktif Çekirdekler: $(cat /sys/devices/system/cpu/online)"
echo "Kapatılan Çekirdek Sayısı: $OFFLINED"
