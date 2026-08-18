#!/usr/bin/env bash
# 🚀 QEMU ÜZERİNDE GERÇEK AMAZON LINUX SANAL MAKİNE KURULUM VE ÇALIŞTIRMA BETİĞİ
set -e

WORK_DIR="qemu_amazon_linux"
IMAGE_NAME="al2023-kvm-2023.6.20250218.0-kernel-6.1-x86_64.xfs.gpt.qcow2"
IMAGE_URL="https://cdn.amazonlinux.com/al2023/os-images/2023.6.20250218.0/kvm/${IMAGE_NAME}"

echo "=================================================================="
echo "   🚀 QEMU ÜZERİNDE GERÇEK AMAZON LINUX 2023 KURULUM VE BAŞLATMA"
echo "=================================================================="

mkdir -p "$WORK_DIR"
cd "$WORK_DIR"

# 1. Amazon Linux 2023 KVM İmajını İndir
if [ ! -f "$IMAGE_NAME" ]; then
    echo "📥 Amazon Linux KVM QCOW2 imajı indiriliyor..."
    curl -O "$IMAGE_URL" || {
        echo "Alternatif Amazon Linux 2 imajına geçiliyor..."
        IMAGE_NAME="amzn2-kvm-2.0.20230504.0-x86_64.xfs.gpt.qcow2"
        IMAGE_URL="https://cdn.amazonlinux.com/os-images/2.0.20230504.0/kvm/${IMAGE_NAME}"
        curl -O "$IMAGE_URL"
    }
fi

# 2. Cloud-Init Yapılandırması (Kullanıcı: ec2-user, Şifre: amazon123)
echo "⚙️ Cloud-Init yapılandırma dosyaları oluşturuluyor..."
cat << 'EOF' > user-data
#cloud-config
user: ec2-user
password: amazon123
chpasswd: { expire: False }
ssh_pwauth: True
write_files:
  - path: /etc/ssh/sshd_config.d/01-permit-password.conf
    content: |
      PasswordAuthentication yes
runcmd:
  - systemctl restart sshd
EOF

cat << 'EOF' > meta-data
instance-id: qemu-amazon-linux-01
local-hostname: amazon-linux-qemu
EOF

# 3. Seed ISO (Cloud-Init imajı) Oluştur
echo "💿 Cloud-init Seed ISO oluşturuluyor..."
genisoimage -output seed.iso -volid cidata -joliet -rock user-data meta-data

# 4. Makine İçin Çalışma Diski Oluştur (Copy-On-Write Overlay)
if [ ! -f "amazon_linux_disk.qcow2" ]; then
    echo "💾 Sanal makine disk alanı hazırlanıyor..."
    qemu-img create -f qcow2 -b "$IMAGE_NAME" -F qcow2 amazon_linux_disk.qcow2 20G
fi

# 5. QEMU Sanal Makinesini Başlat (Port 2222 -> VM Port 22)
echo "=================================================================="
echo "🚀 QEMU Sanal Makinesi Başlatılıyor!"
echo "   Sanal Makine IP: localhost:2222"
echo "   SSH Kullanıcı: ec2-user | Şifre: amazon123"
echo "=================================================================="

KVM_FLAG=""
if [ -e /dev/kvm ] && [ -w /dev/kvm ]; then
    KVM_FLAG="-enable-kvm"
fi

qemu-system-x86_64 \
    $KVM_FLAG \
    -m 2048 \
    -smp 2 \
    -drive file=amazon_linux_disk.qcow2,if=virtio \
    -drive file=seed.iso,media=cdrom \
    -net nic,model=virtio \
    -net user,hostfwd=tcp::2222-:22 \
    -nographic
