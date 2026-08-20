#!/bin/bash
# -------------------------------------------------------------
# Script to restore / online all CPU cores
# -------------------------------------------------------------

if [ "$EUID" -ne 0 ]; then
  echo "Please run this script with root (sudo) privileges:"
  echo "  sudo $0"
  exit 1
fi

echo "Re-enabling All CPU Cores..."

TOTAL_CPUS=$(nproc --all)

for i in $(seq 2 $((TOTAL_CPUS - 1))); do
  if [ -f "/sys/devices/system/cpu/cpu$i/online" ]; then
    echo 1 > "/sys/devices/system/cpu/cpu$i/online"
    echo "✓ CPU $i onlined"
  fi
done

echo ""
echo "=== STATUS ==="
echo "Active Cores: $(cat /sys/devices/system/cpu/online)"
