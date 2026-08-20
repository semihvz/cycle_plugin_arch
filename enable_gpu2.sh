#!/bin/bash
# -------------------------------------------------------------
# Script to re-enable GPU 2 by rescanning PCI bus
# -------------------------------------------------------------

if [ "$EUID" -ne 0 ]; then
  echo "Please run this script with root (sudo) privileges:"
  echo "  sudo $0"
  exit 1
fi

echo "Rescanning PCI Bus and Re-enabling GPU 2..."

echo 1 > /sys/bus/pci/rescan

echo ""
echo "=== ACTIVE GRAPHICS CARDS (DRM) ==="
ls -la /sys/class/drm/ | grep card
