#!/bin/sh
# ==========================================================================
#  Caiman OS Installer v1.2.0
#  Named after the Cuban crocodile. Built for the cloud.
#  Supports: x86_64, aarch64 (ARM) -- any PC, Mac Mini, Raspberry Pi
# ==========================================================================

set -e

G='\033[38;2;22;163;74m'
W='\033[38;2;248;250;252m'
D='\033[38;2;71;85;105m'
R='\033[38;2;220;38;38m'
A='\033[38;2;217;119;6m'
C='\033[38;2;0;212;255m'
NC='\033[0m'

VERSION="1.2.0"

step() { echo -e "\n${G}  == $1${NC}"; }
ok()   { echo -e "  ${G}[OK]${NC} $1"; }
info() { echo -e "  ${C}[..]${NC} $1"; }
warn() { echo -e "  ${A}[!!]${NC} $1"; }
die()  { echo -e "  ${R}[XX]${NC} $1"; exit 1; }
ask()  { printf "${G}  ?${NC} $1 "; }

# ── Welcome screen ────────────────────────────────────────────────────────
clear
echo ""
echo -e "${G}     ______      _                    ____  ____${NC}"
echo -e "${G}    / ____/___ _(_)___ ___  ____ ___ / __ \/ ___/${NC}"
echo -e "${G}   / /   / __ \`/ / __ \`__ \/ __ \`__ / / / /\__ \ ${NC}"
echo -e "${G}  / /___/ /_/ / / / / / / / / / / / / /_/ /___/ /${NC}"
echo -e "${G}  \____/\__,_/_/_/ /_/ /_/_/ /_/ /_/\____//____/ ${NC}"
echo ""
echo -e "  ${C}v${VERSION}${NC}  ${D}Named after the Cuban crocodile.${NC}"
echo -e "  ${D}Open source hypervisor for everyone.${NC}"
echo ""
echo -e "  ${D}Supports: x86_64 -- aarch64 (ARM) -- any PC${NC}"
echo -e "${G}  ==========================================================${NC}"
echo ""

sleep 1

# ── Hardware detection ────────────────────────────────────────────────────
step "Detecting hardware"

ARCH=$(uname -m)
ok "Architecture: $ARCH"

# CPU
CPU_MODEL=$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo "Unknown CPU")
CPU_CORES=$(nproc 2>/dev/null || echo 1)
ok "CPU: $CPU_MODEL ($CPU_CORES cores)"

# RAM
RAM_MIB=$(awk '/MemTotal/{print int($2/1024)}' /proc/meminfo)
RAM_GIB=$((RAM_MIB / 1024))
if [ "$RAM_MIB" -lt 2048 ]; then
    warn "Only ${RAM_GIB}GiB RAM -- minimum 2GiB recommended"
else
    ok "RAM: ${RAM_GIB}GiB"
fi

# KVM
if [ -c /dev/kvm ] || grep -qE "vmx|svm" /proc/cpuinfo 2>/dev/null; then
    ok "KVM virtualization: supported"
    KVM_AVAILABLE=1
else
    warn "KVM not detected -- will run in demo mode"
    KVM_AVAILABLE=0
fi

# Storage
TOTAL_DISK=0
for disk in /sys/block/sd* /sys/block/nvme* /sys/block/vd* /sys/block/mmcblk*; do
    [ -e "$disk" ] || continue
    SIZE_GB=$(cat "$disk/size" 2>/dev/null | awk '{printf "%.0f", $1*512/1024/1024/1024}')
    [ "${SIZE_GB:-0}" -gt 0 ] || continue
    TOTAL_DISK=$((TOTAL_DISK + SIZE_GB))
done
ok "Storage: ${TOTAL_DISK}GB total"

echo ""
echo -e "  ${D}Hardware summary: ${CPU_CORES} cores / ${RAM_GIB}GiB RAM / ${TOTAL_DISK}GB disk${NC}"

# ── Disk selection ────────────────────────────────────────────────────────
step "Select installation disk"

echo ""
DISKS=""
IDX=1
for disk in /sys/block/sd* /sys/block/nvme* /sys/block/vd* /sys/block/mmcblk*; do
    [ -e "$disk" ] || continue
    NAME=$(basename "$disk")
    SIZE_GB=$(cat "$disk/size" 2>/dev/null | awk '{printf "%.0f", $1*512/1024/1024/1024}')
    [ "${SIZE_GB:-0}" -gt 0 ] || continue
    MODEL=$(cat "$disk/device/model" 2>/dev/null | xargs 2>/dev/null || echo "")
    ROTATIONAL=$(cat "$disk/queue/rotational" 2>/dev/null || echo "1")
    TYPE=$([ "$ROTATIONAL" = "0" ] && echo "SSD/NVMe" || echo "HDD")
    echo -e "  ${W}[$IDX]${NC} /dev/$NAME  ${G}${SIZE_GB}GB${NC}  $TYPE  $MODEL"
    DISKS="$DISKS /dev/$NAME"
    IDX=$((IDX + 1))
done

echo ""
ask "Select disk [1]:"
read -r DISK_NUM
DISK_NUM=${DISK_NUM:-1}
TARGET_DISK=$(echo "$DISKS" | tr ' ' '\n' | grep -v '^$' | sed -n "${DISK_NUM}p")
[ -n "$TARGET_DISK" ] || die "Invalid selection"

echo ""
warn "ALL DATA ON $TARGET_DISK WILL BE ERASED"
ask "Type 'yes' to confirm:"
read -r CONFIRM
[ "$CONFIRM" = "yes" ] || { echo "  Aborted."; exit 0; }

# ── Network ───────────────────────────────────────────────────────────────
step "Network configuration"

# Auto-detect interface
IFACES=$(ip -o link show 2>/dev/null | grep -v lo | awk -F': ' '{print $2}' | cut -d@ -f1)
PRIMARY=$(echo "$IFACES" | head -1)

echo ""
echo -e "  ${D}Detected interfaces: $(echo $IFACES | tr '\n' ' ')${NC}"
echo ""

ask "Primary interface [$PRIMARY]:"
read -r IFACE
IFACE=${IFACE:-$PRIMARY}

echo ""
echo -e "  ${W}[1]${NC} DHCP ${D}(recommended -- automatic)${NC}"
echo -e "  ${W}[2]${NC} Static IP ${D}(for servers)${NC}"
echo ""
ask "IP mode [1]:"
read -r IP_MODE
IP_MODE=${IP_MODE:-1}

if [ "$IP_MODE" = "2" ]; then
    ask "IP address (e.g. 192.168.1.100/24):"
    read -r STATIC_IP
    ask "Gateway:"
    read -r GATEWAY
    ask "DNS [1.1.1.1]:"
    read -r DNS
    DNS=${DNS:-1.1.1.1}
fi

# ── VM Network mode ───────────────────────────────────────────────────────
step "VM Network mode"

echo ""
echo -e "  ${W}[1]${NC} NAT ${D}(recommended -- VMs share host IP, works everywhere)${NC}"
echo -e "  ${W}[2]${NC} Bridge ${D}(VMs get their own LAN IP, visible on network)${NC}"
echo -e "  ${W}[3]${NC} Isolated ${D}(VMs only talk to each other, no internet)${NC}"
echo ""
ask "VM network mode [1]:"
read -r NET_MODE_NUM
case "${NET_MODE_NUM:-1}" in
    2) NET_MODE="bridge" ;;
    3) NET_MODE="none" ;;
    *) NET_MODE="nat" ;;
esac
ok "VM network: $NET_MODE"

# ── System config ─────────────────────────────────────────────────────────
step "System configuration"

ask "Hostname [caiman-node-01]:"
read -r HOSTNAME
HOSTNAME=${HOSTNAME:-caiman-node-01}

ask "Admin password:"
read -rs PASSWORD
echo ""
ask "Confirm password:"
read -rs PASSWORD2
echo ""
[ "$PASSWORD" = "$PASSWORD2" ] || die "Passwords do not match"

# ── Cluster mode ──────────────────────────────────────────────────────────
step "Deployment mode"

echo ""
echo -e "  ${W}[1]${NC} Standalone ${D}(single node -- perfect for recycled PCs)${NC}"
echo -e "  ${W}[2]${NC} Join cluster ${D}(add this node to existing Caiman cluster)${NC}"
echo ""
ask "Mode [1]:"
read -r CLUSTER_MODE
CLUSTER_MODE=${CLUSTER_MODE:-1}

if [ "$CLUSTER_MODE" = "2" ]; then
    ask "Cluster API URL (e.g. https://192.168.1.10:8765):"
    read -r CLUSTER_URL
    ask "Cluster join token (caim_...):"
    read -r JOIN_TOKEN
fi

# ── Summary ───────────────────────────────────────────────────────────────
echo ""
echo -e "${G}  ==========================================================${NC}"
echo -e "${G}  Installation summary${NC}"
echo -e "${G}  ==========================================================${NC}"
echo ""
echo -e "  ${D}Disk:${NC}       $TARGET_DISK"
echo -e "  ${D}Hostname:${NC}   $HOSTNAME"
echo -e "  ${D}Network:${NC}    $IFACE -- $([ "$IP_MODE" = "2" ] && echo "$STATIC_IP" || echo "DHCP")"
echo -e "  ${D}VM Net:${NC}     $NET_MODE"
echo -e "  ${D}KVM:${NC}        $([ "$KVM_AVAILABLE" = "1" ] && echo "enabled" || echo "demo mode")"
echo -e "  ${D}Mode:${NC}       $([ "$CLUSTER_MODE" = "2" ] && echo "Join $CLUSTER_URL" || echo "Standalone")"
echo -e "  ${D}Version:${NC}    Caiman OS $VERSION"
echo ""
ask "Start installation? [y/N]:"
read -r START
[ "$START" = "y" ] || [ "$START" = "Y" ] || { echo "  Aborted."; exit 0; }

# ── Install ───────────────────────────────────────────────────────────────
step "Installing Caiman OS"
echo ""

# 1. Partition
info "Partitioning $TARGET_DISK..."
parted -s "$TARGET_DISK" mklabel gpt
parted -s "$TARGET_DISK" mkpart ESP fat32 1MiB 512MiB
parted -s "$TARGET_DISK" set 1 esp on
parted -s "$TARGET_DISK" mkpart primary ext4 512MiB 100%
sleep 1

if echo "$TARGET_DISK" | grep -qE "nvme|mmcblk"; then
    BOOT_PART="${TARGET_DISK}p1"
    ROOT_PART="${TARGET_DISK}p2"
else
    BOOT_PART="${TARGET_DISK}1"
    ROOT_PART="${TARGET_DISK}2"
fi
ok "Partitioned: $BOOT_PART (EFI) + $ROOT_PART (root)"

# 2. Format
info "Formatting..."
mkfs.fat -F32 "$BOOT_PART" -n CAIMAN-BOOT >/dev/null 2>&1
mkfs.ext4 -q -L caiman-root "$ROOT_PART"
ok "Formatted ext4 + FAT32"

# 3. Mount
mkdir -p /mnt/caiman
mount "$ROOT_PART" /mnt/caiman
mkdir -p /mnt/caiman/boot/efi
mount "$BOOT_PART" /mnt/caiman/boot/efi
ok "Mounted"

# 4. Base system
info "Installing base system..."
if [ -f /media/cdrom/caiman-rootfs.tar.gz ]; then
    tar -xzf /media/cdrom/caiman-rootfs.tar.gz -C /mnt/caiman/ >/dev/null 2>&1
else
    apk --root /mnt/caiman --initdb add alpine-base >/dev/null 2>&1 || true
fi
ok "Base system installed"

# 5. Binaries
info "Installing Caiman OS binaries..."
mkdir -p /mnt/caiman/usr/local/bin
mkdir -p /mnt/caiman/etc/caiman
mkdir -p /mnt/caiman/var/run/caiman
mkdir -p /mnt/caiman/var/lib/caiman/{disks,kernels,ipam}

for bin in caiman-vmm caiman-api caiman-cni caiman-drs caiman-bts \
           caiman-mcp caiman-storage caiman-gpu caiman-livemig; do
    if [ -f "/media/cdrom/bin/$bin" ]; then
        cp "/media/cdrom/bin/$bin" "/mnt/caiman/usr/local/bin/$bin"
        chmod +x "/mnt/caiman/usr/local/bin/$bin"
    fi
done
ok "Binaries installed"

# 6. Generate JWT secret
info "Generating security credentials..."
JWT_SECRET=$(cat /dev/urandom | tr -dc 'a-f0-9' | head -c 64)
echo "$JWT_SECRET" > /mnt/caiman/etc/caiman/jwt-secret
chmod 600 /mnt/caiman/etc/caiman/jwt-secret
ok "JWT secret generated"

# 7. System config
info "Configuring system..."
echo "$HOSTNAME" > /mnt/caiman/etc/hostname

cat > /mnt/caiman/etc/hosts << EOF
127.0.0.1   localhost
127.0.1.1   $HOSTNAME
::1         localhost ip6-localhost ip6-loopback
EOF

mkdir -p /mnt/caiman/etc/network
if [ "$IP_MODE" = "2" ]; then
    cat > /mnt/caiman/etc/network/interfaces << EOF
auto lo
iface lo inet loopback

auto $IFACE
iface $IFACE inet static
    address $STATIC_IP
    gateway $GATEWAY
    dns-nameservers ${DNS:-1.1.1.1}
EOF
else
    cat > /mnt/caiman/etc/network/interfaces << EOF
auto lo
iface lo inet loopback

auto $IFACE
iface $IFACE inet dhcp
EOF
fi

# 8. Caiman config
cat > /mnt/caiman/etc/caiman/config.toml << EOF
[node]
hostname  = "$HOSTNAME"
arch      = "$ARCH"
role      = "$([ "$CLUSTER_MODE" = "2" ] && echo "worker" || echo "standalone")"
version   = "$VERSION"
kvm       = $KVM_AVAILABLE

[api]
listen    = "0.0.0.0:8765"
demo_mode = $([ "$KVM_AVAILABLE" = "1" ] && echo "false" || echo "true")

[network]
mode      = "$NET_MODE"
uplink    = "$IFACE"
bridge    = "caiman0"
subnet    = "10.100.0.0/24"
gateway   = "10.100.0.1"

[storage]
data_dir  = "/var/lib/caiman"

[ui]
listen    = "0.0.0.0:80"
EOF

[ "$CLUSTER_MODE" = "2" ] && cat >> /mnt/caiman/etc/caiman/config.toml << EOF

[cluster]
api_url    = "$CLUSTER_URL"
join_token = "$JOIN_TOKEN"
EOF

ok "Configuration written"

# 9. OpenRC services
info "Setting up services..."

cat > /mnt/caiman/etc/init.d/caiman-api << EOF
#!/sbin/openrc-run
name="caiman-api"
description="Caiman OS API"
command="/usr/local/bin/caiman-api"
pidfile="/var/run/caiman-api.pid"
command_background=true
output_log="/var/log/caiman-api.log"
environment="CAIMAN_JWT_SECRET=\$(cat /etc/caiman/jwt-secret) CAIMAN_NET_MODE=$NET_MODE"
depend() { need net; }
EOF
chmod +x /mnt/caiman/etc/init.d/caiman-api

# nginx for UI
cat > /mnt/caiman/etc/nginx/conf.d/caiman-ui.conf << 'EOF'
server {
    listen 80 default_server;
    root /var/www/caiman-ui;
    index index.html;
    location / { try_files $uri /index.html; }
    location /api/ {
        proxy_pass http://127.0.0.1:8765;
        proxy_set_header Host $host;
        add_header Access-Control-Allow-Origin "*" always;
        add_header Access-Control-Allow-Headers "Content-Type, Authorization" always;
        if ($request_method = OPTIONS) { return 204; }
    }
}
EOF

# Copy UI from ISO
if [ -d /media/cdrom/ui ]; then
    mkdir -p /mnt/caiman/var/www/caiman-ui
    cp -r /media/cdrom/ui/* /mnt/caiman/var/www/caiman-ui/
fi

chroot /mnt/caiman rc-update add caiman-api default 2>/dev/null || true
chroot /mnt/caiman rc-update add nginx default 2>/dev/null || true
chroot /mnt/caiman rc-update add sshd default 2>/dev/null || true
ok "Services enabled"

# 10. Bootloader
info "Installing GRUB..."
grub-install --target=x86_64-efi \
    --efi-directory=/mnt/caiman/boot/efi \
    --boot-directory=/mnt/caiman/boot \
    --removable >/dev/null 2>&1 || true

cp /media/cdrom/boot/vmlinuz /mnt/caiman/boot/ 2>/dev/null || true
cp /media/cdrom/boot/initramfs.img /mnt/caiman/boot/ 2>/dev/null || true

mkdir -p /mnt/caiman/boot/grub
cat > /mnt/caiman/boot/grub/grub.cfg << EOF
set default=0
set timeout=3

menuentry "Caiman OS $VERSION" {
    linux  /boot/vmlinuz quiet root=LABEL=caiman-root
    initrd /boot/initramfs.img
}
EOF
ok "GRUB installed"

# 11. Root password
info "Setting password..."
echo "root:$PASSWORD" | chroot /mnt/caiman chpasswd 2>/dev/null || true
ok "Password set"

# 12. Generate first admin token
info "Generating admin token..."
ADMIN_TOKEN="caim_$(cat /dev/urandom | tr -dc 'a-zA-Z0-9' | head -c 48)"
echo "$ADMIN_TOKEN" > /mnt/caiman/etc/caiman/admin-token
chmod 600 /mnt/caiman/etc/caiman/admin-token
ok "Admin token generated"

# 13. Unmount
info "Finalizing..."
sync
umount /mnt/caiman/boot/efi 2>/dev/null || true
umount /mnt/caiman 2>/dev/null || true
ok "Done"

# ── Complete ──────────────────────────────────────────────────────────────
IP_DISPLAY=$([ "$IP_MODE" = "2" ] && echo "${STATIC_IP%%/*}" || echo "<dhcp-ip>")

echo ""
echo -e "${G}  ==========================================================${NC}"
echo -e "${G}  🐊 Caiman OS ${VERSION} installed successfully!${NC}"
echo -e "${G}  ==========================================================${NC}"
echo ""
echo -e "  ${D}Remove the USB drive and reboot.${NC}"
echo ""
echo -e "  ${C}Dashboard:${NC}   http://${IP_DISPLAY}"
echo -e "  ${C}API:${NC}         http://${IP_DISPLAY}:8765"
echo ""
echo -e "  ${A}Admin token (save this!):${NC}"
echo -e "  ${W}${ADMIN_TOKEN}${NC}"
echo ""
echo -e "  ${D}Use this token to connect from caiman-ui.${NC}"
echo -e "${G}  ==========================================================${NC}"
echo ""
ask "Reboot now? [Y/n]:"
read -r REBOOT
[ "$REBOOT" = "n" ] || reboot
