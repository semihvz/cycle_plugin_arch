#!/bin/bash
# -------------------------------------------------------------
# Script to offline CPU cores 2 through 15 (keeping cpu0 and cpu1 active)
# -------------------------------------------------------------

if [ "$EUID" -ne 0 ]; then
  echo "Please run this script with root (sudo) privileges:"
  echo "  sudo $0"
  exit 1
fi

echo "Offlining CPU Cores (Leaving only CPU 0 and 1 active)..."

TOTAL_CPUS=$(nproc --all)
OFFLINED=0

for i in $(seq 2 $((TOTAL_CPUS - 1))); do
  if [ -f "/sys/devices/system/cpu/cpu$i/online" ]; then
    echo 0 > "/sys/devices/system/cpu/cpu$i/online"
    echo "✓ CPU $i offlined"
    OFFLINED=$((OFFLINED + 1))
  fi
done

echo ""
echo "=== STATUS ==="
echo "Active Cores: $(cat /sys/devices/system/cpu/online)"
echo "Offlined Core Count: $OFFLINED"
