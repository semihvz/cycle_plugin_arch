#!/bin/bash
# -------------------------------------------------------------
# Script to re-enable GPU 2 by rescanning PCI bus
# -------------------------------------------------------------

if [ "$EUID" -ne 0 ]; then
  echo "Lütfen bu betiği root (sudo) yetkisi ile çalıştırın:"
  echo "  sudo $0"
  exit 1
fi

echo "PCI Veriyolu Taranıyor ve GPU 2 Yeniden Etkinleştiriliyor..."

echo 1 > /sys/bus/pci/rescan

echo ""
echo "=== AKTİF EKRAN KARTLARI (DRM) ==="
ls -la /sys/class/drm/ | grep card
