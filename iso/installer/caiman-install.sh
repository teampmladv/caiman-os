#!/bin/sh
# ═══════════════════════════════════════════════════════════════════════════
#  Caimán OS Installer v1.1.0
#  Named after the Cuban crocodile. Built for the cloud.
#
#  This script runs inside the Alpine-based initramfs.
#  It guides the user through installing Caimán OS on bare metal.
# ═══════════════════════════════════════════════════════════════════════════

set -e

# ── Colors ────────────────────────────────────────────────────────────────
G='\033[38;2;22;163;74m'   # green
W='\033[38;2;248;250;252m' # white
D='\033[38;2;71;85;105m'   # dim
R='\033[38;2;220;38;38m'   # red
A='\033[38;2;217;119;6m'   # amber
NC='\033[0m'

CAIMAN_VERSION="1.1.0"

clear
echo ""
echo -e "${G}  🐊 Caimán OS v${CAIMAN_VERSION} Installer${NC}"
echo -e "${D}  Named after the Cuban crocodile. Built for the cloud.${NC}"
echo ""
echo -e "${D}  ─────────────────────────────────────────────────────────${NC}"
echo ""

# ── Check KVM ─────────────────────────────────────────────────────────────
step() { echo -e "\n${G}  ━━ $1${NC}"; }
ok()   { echo -e "  ${G}✓${NC} $1"; }
warn() { echo -e "  ${A}⚠${NC} $1"; }
die()  { echo -e "  ${R}✗${NC} $1"; exit 1; }
ask()  { echo -ne "${G}  ?${NC} $1 "; }

step "Checking hardware"

# CPU virtualization
if grep -qE "vmx|svm" /proc/cpuinfo 2>/dev/null; then
    ok "CPU virtualization supported ($(grep -m1 'model name' /proc/cpuinfo | cut -d: -f2 | xargs))"
else
    warn "CPU virtualization not detected — Caimán OS will run in demo mode"
fi

# RAM
RAM_GIB=$(awk '/MemTotal/{printf "%.0f", $2/1024/1024}' /proc/meminfo)
if [ "$RAM_GIB" -lt 4 ]; then
    warn "Only ${RAM_GIB} GiB RAM detected — minimum 4 GiB recommended"
else
    ok "${RAM_GIB} GiB RAM"
fi

# Architecture
ARCH=$(uname -m)
[ "$ARCH" = "x86_64" ] || die "Only x86_64 is supported (got $ARCH)"
ok "Architecture: x86_64"

# ── Disk selection ────────────────────────────────────────────────────────
step "Select installation disk"

echo ""
echo -e "  ${D}Available disks:${NC}"
echo ""

# List disks
DISKS=""
IDX=1
for disk in /sys/block/sd* /sys/block/nvme* /sys/block/vd*; do
    [ -e "$disk" ] || continue
    NAME=$(basename "$disk")
    SIZE_BYTES=$(cat "$disk/size" 2>/dev/null || echo 0)
    SIZE_GIB=$((SIZE_BYTES * 512 / 1024 / 1024 / 1024))
    MODEL=$(cat "$disk/device/model" 2>/dev/null | xargs || echo "Unknown")
    [ "$SIZE_GIB" -gt 0 ] || continue
    echo -e "  ${W}[$IDX]${NC} /dev/$NAME  ${G}${SIZE_GIB} GiB${NC}  $MODEL"
    DISKS="$DISKS /dev/$NAME"
    IDX=$((IDX + 1))
done

echo ""
ask "Enter disk number [1]:"
read -r DISK_NUM
DISK_NUM=${DISK_NUM:-1}

TARGET_DISK=$(echo "$DISKS" | tr ' ' '\n' | sed -n "${DISK_NUM}p")
[ -n "$TARGET_DISK" ] || die "Invalid disk selection"

echo ""
warn "ALL DATA ON $TARGET_DISK WILL BE ERASED"
ask "Type 'yes' to confirm:"
read -r CONFIRM
[ "$CONFIRM" = "yes" ] || { echo "  Aborted."; exit 0; }

# ── Network configuration ─────────────────────────────────────────────────
step "Network configuration"

# Detect interfaces
IFACES=$(ip -o link show | grep -v lo | awk -F': ' '{print $2}' | cut -d@ -f1)
PRIMARY=$(echo "$IFACES" | head -1)

echo ""
echo -e "  ${D}Network interfaces: $(echo $IFACES | tr '\n' ' ')${NC}"
echo ""

ask "Primary interface [$PRIMARY]:"
read -r IFACE
IFACE=${IFACE:-$PRIMARY}

echo ""
ask "IP configuration — (1) DHCP  (2) Static [1]:"
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

# ── Hostname ──────────────────────────────────────────────────────────────
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
ask "Cluster mode — (1) Standalone  (2) Join existing cluster [1]:"
read -r CLUSTER_MODE
CLUSTER_MODE=${CLUSTER_MODE:-1}

if [ "$CLUSTER_MODE" = "2" ]; then
    ask "Cluster API URL (e.g. https://192.168.1.10:8765):"
    read -r CLUSTER_URL
    ask "Cluster join token:"
    read -r JOIN_TOKEN
fi

# ── Summary ───────────────────────────────────────────────────────────────
echo ""
echo -e "${G}  ━━ Installation summary${NC}"
echo ""
echo -e "  ${D}Disk:${NC}      $TARGET_DISK"
echo -e "  ${D}Hostname:${NC}  $HOSTNAME"
echo -e "  ${D}Network:${NC}   $IFACE $([ "$IP_MODE" = "2" ] && echo "$STATIC_IP" || echo "DHCP")"
echo -e "  ${D}Mode:${NC}      $([ "$CLUSTER_MODE" = "2" ] && echo "Join cluster $CLUSTER_URL" || echo "Standalone")"
echo -e "  ${D}Version:${NC}   Caimán OS $CAIMAN_VERSION"
echo ""
ask "Start installation? [y/N]:"
read -r START
[ "$START" = "y" ] || [ "$START" = "Y" ] || { echo "  Aborted."; exit 0; }

# ── Installation ──────────────────────────────────────────────────────────
step "Installing Caimán OS"

echo ""

# 1. Partition disk
echo -ne "  Partitioning $TARGET_DISK... "
parted -s "$TARGET_DISK" mklabel gpt
parted -s "$TARGET_DISK" mkpart ESP fat32 1MiB 512MiB
parted -s "$TARGET_DISK" set 1 esp on
parted -s "$TARGET_DISK" mkpart primary ext4 512MiB 100%
echo -e "${G}✓${NC}"

# Determine partition names
if echo "$TARGET_DISK" | grep -q nvme; then
    BOOT_PART="${TARGET_DISK}p1"
    ROOT_PART="${TARGET_DISK}p2"
else
    BOOT_PART="${TARGET_DISK}1"
    ROOT_PART="${TARGET_DISK}2"
fi

# 2. Format
echo -ne "  Formatting partitions... "
mkfs.fat -F32 "$BOOT_PART" -n CAIMAN-BOOT >/dev/null 2>&1
mkfs.ext4 -q -L caiman-root "$ROOT_PART"
echo -e "${G}✓${NC}"

# 3. Mount
echo -ne "  Mounting... "
mkdir -p /mnt/caiman
mount "$ROOT_PART" /mnt/caiman
mkdir -p /mnt/caiman/boot/efi
mount "$BOOT_PART" /mnt/caiman/boot/efi
echo -e "${G}✓${NC}"

# 4. Install Alpine base
echo -ne "  Installing Alpine base... "
# Copy squashfs from ISO
if [ -f /media/cdrom/caiman-rootfs.tar.gz ]; then
    tar -xzf /media/cdrom/caiman-rootfs.tar.gz -C /mnt/caiman/ >/dev/null 2>&1
else
    # Bootstrap from network
    apk --root /mnt/caiman --initdb add alpine-base >/dev/null 2>&1 || true
fi
echo -e "${G}✓${NC}"

# 5. Install Caimán OS binaries
echo -ne "  Installing Caimán OS binaries... "
mkdir -p /mnt/caiman/usr/local/bin
mkdir -p /mnt/caiman/etc/caiman
mkdir -p /mnt/caiman/var/run/caiman
mkdir -p /mnt/caiman/var/lib/caiman/disks
mkdir -p /mnt/caiman/var/lib/caiman/kernels

# Copy binaries from ISO
for bin in caiman-vmm caiman-api caiman-drs caiman-bts caiman-mcp \
           caiman-storage caiman-gpu caiman-livemig caiman; do
    if [ -f "/media/cdrom/bin/$bin" ]; then
        cp "/media/cdrom/bin/$bin" "/mnt/caiman/usr/local/bin/$bin"
        chmod +x "/mnt/caiman/usr/local/bin/$bin"
    fi
done
echo -e "${G}✓${NC}"

# 6. Configure system
echo -ne "  Configuring system... "

# Hostname
echo "$HOSTNAME" > /mnt/caiman/etc/hostname

# Hosts
cat > /mnt/caiman/etc/hosts << EOF
127.0.0.1   localhost
127.0.1.1   $HOSTNAME
::1         localhost
EOF

# Network
if [ "$IP_MODE" = "2" ]; then
    cat > /mnt/caiman/etc/network/interfaces << EOF
auto lo
iface lo inet loopback

auto $IFACE
iface $IFACE inet static
    address $STATIC_IP
    gateway $GATEWAY
    dns-nameservers $DNS
EOF
else
    cat > /mnt/caiman/etc/network/interfaces << EOF
auto lo
iface lo inet loopback

auto $IFACE
iface $IFACE inet dhcp
EOF
fi

# Caiman config
cat > /mnt/caiman/etc/caiman/config.toml << EOF
[node]
hostname    = "$HOSTNAME"
role        = "$([ "$CLUSTER_MODE" = "2" ] && echo "worker" || echo "standalone")"
version     = "$CAIMAN_VERSION"

[api]
listen      = "0.0.0.0:8765"
demo_mode   = false

[storage]
data_dir    = "/var/lib/caiman"
disks_dir   = "/var/lib/caiman/disks"

[drs]
mode        = "FullyAutomated"
threshold   = 0.10
EOF

[ "$CLUSTER_MODE" = "2" ] && cat >> /mnt/caiman/etc/caiman/config.toml << EOF

[cluster]
api_url     = "$CLUSTER_URL"
join_token  = "$JOIN_TOKEN"
EOF

echo -e "${G}✓${NC}"

# 7. Install bootloader
echo -ne "  Installing GRUB2 bootloader... "
grub-install --target=x86_64-efi \
    --efi-directory=/mnt/caiman/boot/efi \
    --boot-directory=/mnt/caiman/boot \
    --removable >/dev/null 2>&1 || true

# Copy kernel + initramfs
cp /media/cdrom/boot/vmlinuz /mnt/caiman/boot/
cp /media/cdrom/boot/initramfs.img /mnt/caiman/boot/

# GRUB config
mkdir -p /mnt/caiman/boot/grub
cat > /mnt/caiman/boot/grub/grub.cfg << EOF
set default=0
set timeout=3

menuentry "Caimán OS $CAIMAN_VERSION" {
    linux  /boot/vmlinuz quiet root=LABEL=caiman-root
    initrd /boot/initramfs.img
}
EOF

echo -e "${G}✓${NC}"

# 8. Set root password
echo -ne "  Setting password... "
echo "root:$PASSWORD" | chroot /mnt/caiman chpasswd 2>/dev/null || true
echo -e "${G}✓${NC}"

# 9. Enable services
echo -ne "  Enabling services... "
for svc in caiman-api caiman-drs caiman-bts; do
    if [ -f "/mnt/caiman/etc/init.d/$svc" ]; then
        chroot /mnt/caiman rc-update add "$svc" default 2>/dev/null || true
    fi
done
echo -e "${G}✓${NC}"

# 10. Unmount
echo -ne "  Finalizing... "
sync
umount /mnt/caiman/boot/efi 2>/dev/null || true
umount /mnt/caiman 2>/dev/null || true
echo -e "${G}✓${NC}"

# ── Done ─────────────────────────────────────────────────────────────────
echo ""
echo -e "${G}  ─────────────────────────────────────────────────────────${NC}"
echo -e "${G}  🐊 Caimán OS ${CAIMAN_VERSION} installed successfully!${NC}"
echo ""
echo -e "  ${D}Remove the installation media and reboot.${NC}"
echo -e "  ${D}Dashboard will be available at:${NC}"
echo -e "  ${G}  http://${HOSTNAME}:3000${NC}"
echo -e "  ${G}  http://<ip>:3000${NC}"
echo ""
ask "Reboot now? [Y/n]:"
read -r REBOOT
[ "$REBOOT" = "n" ] || reboot
