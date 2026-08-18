#!/usr/bin/env bash
# 🚀 CYCLE ORCHESTRATOR - AMAZON LINUX (AL2 / AL2023) SETUP & DEPLOYMENT SCRIPT
set -e

echo "=================================================================="
echo "   🚀 AMAZON LINUX ORCHESTRATOR KURULUM VE YAYGINLAŞTIRMA SİSTEMİ"
echo "=================================================================="

# 1. İşletim Sistemi Paketlerini Güncelle ve Gerekli Araçları Yükle
echo "📦 1. İşletim sistemi bağımlılıkları yükleniyor..."
if command -v dnf &> /dev/null; then
    sudo dnf update -y
    sudo dnf groupinstall "Development Tools" -y
    sudo dnf install -y gcc gcc-c++ openssl-devel sqlite-devel pkgconfig git
elif command -v yum &> /dev/null; then
    sudo yum update -y
    sudo yum groupinstall "Development Tools" -y
    sudo yum install -y gcc gcc-c++ openssl-devel sqlite-devel pkgconfig git
fi

# 2. Rust ve Cargo Kurulumu (Eğer Yüklü Değilse)
if ! command -v cargo &> /dev/null; then
    echo "🦀 2. Rust ve Cargo kuruluyor..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo "🦀 2. Rust ve Cargo zaten yüklü:"
    cargo --version
fi

# 3. Proje Dizinine Git ve Release Sürümünü Derle
echo "🔨 3. Proje ve C-ABI Mikro-Eklentileri Derleniyor (Release Mode)..."
cargo build --release

# 4. Derlenen İkili Dosya ve Eklentileri Kontrol Et
echo "✅ 4. Derleme Sonuçları Kontrol Ediliyor..."
ls -lh target/release/cycle-finance-breakout-system
ls -lh target/release/libplugin_*.so

echo "=================================================================="
echo "🎉 KURULUM TAMAMLANDI! Sistemi Çalıştırmak İçin:"
echo "   cargo run --release -p cycle-finance-breakout-system"
echo "=================================================================="
