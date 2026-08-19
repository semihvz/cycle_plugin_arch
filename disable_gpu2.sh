#!/bin/bash
# -------------------------------------------------------------
# Script to disable GPU 2 (Integrated AMD Radeon Vega APU - 0000:07:00.0)
# -------------------------------------------------------------

if [ "$EUID" -ne 0 ]; then
  echo "Lütfen bu betiği root (sudo) yetkisi ile çalıştırın:"
  echo "  sudo $0"
  exit 1
fi

echo "GPU 2 (0000:07:00.0 / card2) Pasife Alınıyor..."

if [ -e "/sys/bus/pci/drivers/amdgpu/unbind" ]; then
  echo "0000:07:00.0" > /sys/bus/pci/drivers/amdgpu/unbind 2>/dev/null
fi

if [ -d "/sys/bus/pci/devices/0000:07:00.0" ]; then
  echo 1 > /sys/bus/pci/devices/0000:07:00.0/remove
fi

echo ""
echo "=== AKTİF EKRAN KARTLARI (DRM) ==="
ls -la /sys/class/drm/ | grep card
