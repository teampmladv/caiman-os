#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  Caimán OS — ISO Builder
#  Builds a bootable ISO based on Alpine Linux
#
#  Usage: sudo ./iso/scripts/build-iso.sh [version]
#  Output: caiman-os-<version>-x86_64.iso
#
#  Requirements (Ubuntu/Debian):
#    apt-get install -y xorriso grub-efi-amd64-bin grub-pc-bin \
#                       mtools squashfs-tools curl
#
#  Requirements (CentOS/RHEL):
#    dnf install -y xorriso grub2-efi-x64 grub2-tools squashfs-tools curl
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

VERSION="${1:-1.1.0}"
ARCH="x86_64"
ISO_NAME="caiman-os-${VERSION}-${ARCH}.iso"
WORK_DIR="/tmp/caiman-iso-build"
ISO_ROOT="$WORK_DIR/iso"
ALPINE_VERSION="3.19"
ALPINE_MINI="https://dl-cdn.alpinelinux.org/alpine/v${ALPINE_VERSION}/releases/x86_64/alpine-minirootfs-${ALPINE_VERSION}.5-x86_64.tar.gz"
ALPINE_KERNEL="https://dl-cdn.alpinelinux.org/alpine/v${ALPINE_VERSION}/releases/x86_64/alpine-virt-${ALPINE_VERSION}.5-x86_64.iso"

BRT='\033[38;2;22;163;74m'; GRN='\033[38;2;76;175;80m'
DIM='\033[38;2;71;85;105m'; RED='\033[38;2;220;38;38m'; NC='\033[0m'

step() { echo -e "\n${BRT}━━ $1${NC}"; }
ok()   { echo -e "  ${GRN}✓${NC} $1"; }
die()  { echo -e "  ${RED}✗${NC} $1"; exit 1; }

echo -e "\n${BRT}🐊 Building Caimán OS ${VERSION} ISO${NC}\n"

[[ $EUID -eq 0 ]] || die "Run as root: sudo $0"

# ── Dependencies ──────────────────────────────────────────────────────────
step "Checking dependencies"

# Install missing deps automatically
if command -v dnf &>/dev/null || command -v yum &>/dev/null; then
    PKG="dnf"
    command -v dnf &>/dev/null || PKG="yum"
    $PKG install -y -q xorriso grub2-efi-x64-modules grub2-tools-extra         squashfs-tools curl mtools 2>/dev/null || true
    # grub-mkstandalone is in grub2-tools-extra on CentOS
    # symlink if needed
    if ! command -v grub-mkstandalone &>/dev/null; then
        ln -sf /usr/bin/grub2-mkstandalone /usr/local/bin/grub-mkstandalone 2>/dev/null || true
    fi
elif command -v apt-get &>/dev/null; then
    apt-get install -y -qq xorriso grub-efi-amd64-bin grub-pc-bin         squashfs-tools mtools curl isolinux 2>/dev/null || true
fi

for cmd in xorriso mksquashfs curl; do
    command -v "$cmd" &>/dev/null || die "$cmd not found"
done

# grub-mkstandalone or grub2-mkstandalone
GRUB_MKSTANDALONE=$(command -v grub-mkstandalone || command -v grub2-mkstandalone || echo "")
[ -n "$GRUB_MKSTANDALONE" ] || die "grub-mkstandalone not found — dnf install grub2-tools-extra"
ok "All dependencies present"

# ── Prepare directories ───────────────────────────────────────────────────
step "Preparing build directory"

rm -rf "$WORK_DIR"
mkdir -p "$ISO_ROOT"/{boot,bin,lib}
mkdir -p "$ISO_ROOT/boot/grub"
mkdir -p "$WORK_DIR/rootfs"
ok "Build directory: $WORK_DIR"

# ── Download Alpine kernel + initramfs ────────────────────────────────────
step "Downloading Alpine Linux kernel"

ALPINE_ISO_CACHE="/tmp/alpine-virt.iso"
if [[ ! -f "$ALPINE_ISO_CACHE" ]]; then
    echo -e "  ${DIM}Downloading Alpine virt ISO...${NC}"
    curl -fsSL -o "$ALPINE_ISO_CACHE" "$ALPINE_KERNEL"
fi

# Mount Alpine ISO to extract kernel + initramfs
ALPINE_MOUNT="/tmp/alpine-mount"
mkdir -p "$ALPINE_MOUNT"
mount -o loop "$ALPINE_ISO_CACHE" "$ALPINE_MOUNT"

cp "$ALPINE_MOUNT/boot/vmlinuz-virt"       "$ISO_ROOT/boot/vmlinuz"
cp "$ALPINE_MOUNT/boot/initramfs-virt"     "$ISO_ROOT/boot/initramfs-alpine.img"
umount "$ALPINE_MOUNT"
ok "Kernel: $(du -sh $ISO_ROOT/boot/vmlinuz | cut -f1)"

# ── Download Alpine minirootfs ────────────────────────────────────────────
step "Building rootfs"

echo -e "  ${DIM}Downloading Alpine minirootfs...${NC}"
curl -fsSL -o "$WORK_DIR/alpine-rootfs.tar.gz" "$ALPINE_MINI"
tar -xzf "$WORK_DIR/alpine-rootfs.tar.gz" -C "$WORK_DIR/rootfs/"
ok "Alpine base extracted"

# Install packages into rootfs
cat > "$WORK_DIR/rootfs/etc/resolv.conf" << 'EOF'
nameserver 1.1.1.1
nameserver 8.8.8.8
EOF

chroot "$WORK_DIR/rootfs" /bin/sh << 'CHROOT'
apk update --quiet
apk add --quiet --no-cache \
    busybox-initscripts \
    openrc \
    bash \
    parted \
    e2fsprogs \
    dosfstools \
    grub \
    grub-efi \
    efibootmgr \
    dhcpcd \
    openssh \
    curl \
    jq \
    iproute2 \
    iptables \
    docker \
    docker-compose \
    nginx \
    ca-certificates \
    dialog

# Enable services
rc-update add docker default 2>/dev/null || true
rc-update add nginx default 2>/dev/null || true
rc-update add sshd default 2>/dev/null || true
rc-update add dhcpcd default 2>/dev/null || true
CHROOT

ok "Alpine packages installed"

# ── Copy Caimán OS binaries ───────────────────────────────────────────────
step "Adding Caimán OS binaries"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Build binaries if not present
if [[ ! -f "$REPO_ROOT/target/release/caiman-vmm" ]]; then
    echo -e "  ${DIM}Building Rust binaries...${NC}"
    cd "$REPO_ROOT"
    cargo build --release --workspace
fi

for bin in caiman-vmm caiman-api caiman-drs caiman-bts caiman-mcp \
           caiman-storage caiman-gpu caiman-livemig caiman; do
    src="$REPO_ROOT/target/release/$bin"
    if [[ -f "$src" ]]; then
        cp "$src" "$WORK_DIR/rootfs/usr/local/bin/$bin"
        chmod +x "$WORK_DIR/rootfs/usr/local/bin/$bin"
        cp "$src" "$ISO_ROOT/bin/$bin"
        echo -e "  ${GRN}✓${NC} $bin  ($(du -sh $src | cut -f1))"
    fi
done

# ── Copy installer ────────────────────────────────────────────────────────
step "Adding installer"

cp "$SCRIPT_DIR/../installer/caiman-install.sh" \
   "$WORK_DIR/rootfs/usr/local/bin/caiman-install"
chmod +x "$WORK_DIR/rootfs/usr/local/bin/caiman-install"

# Auto-start installer on boot
cat > "$WORK_DIR/rootfs/etc/profile.d/caiman-installer.sh" << 'EOF'
#!/bin/sh
# Auto-start installer if caiman.mode=install in cmdline
if grep -q "caiman.mode=install" /proc/cmdline 2>/dev/null; then
    /usr/local/bin/caiman-install
fi
EOF

# ── OpenRC service for Caimán ─────────────────────────────────────────────
cat > "$WORK_DIR/rootfs/etc/init.d/caiman-api" << 'EOF'
#!/sbin/openrc-run
name="caiman-api"
description="Caimán OS API server"
command="/usr/local/bin/caiman-api"
pidfile="/var/run/caiman-api.pid"
command_background=true
output_log="/var/log/caiman-api.log"

depend() {
    need net
    after docker
}
EOF
chmod +x "$WORK_DIR/rootfs/etc/init.d/caiman-api"
ok "Installer and services added"

# ── Build initramfs ───────────────────────────────────────────────────────
step "Building initramfs"

# Create custom initramfs that includes our rootfs
cd "$WORK_DIR/rootfs"
find . | cpio -H newc -o | gzip -9 > "$ISO_ROOT/boot/initramfs.img"
ok "Initramfs: $(du -sh $ISO_ROOT/boot/initramfs.img | cut -f1)"

# ── GRUB config ───────────────────────────────────────────────────────────
step "Configuring bootloader"

cp "$SCRIPT_DIR/../grub/grub.cfg" "$ISO_ROOT/boot/grub/grub.cfg"
ok "GRUB config installed"

# ── Build GRUB EFI image ──────────────────────────────────────────────────
$GRUB_MKSTANDALONE \
    --format=x86_64-efi \
    --output="$ISO_ROOT/boot/grub/efiboot.img" \
    --modules="part_gpt part_msdos fat iso9660 all_video" \
    "boot/grub/grub.cfg=$ISO_ROOT/boot/grub/grub.cfg"
ok "GRUB EFI image built"

# ── Pack ISO ──────────────────────────────────────────────────────────────
step "Creating ISO"

# ── Copy syslinux from Alpine for BIOS boot ────────────────────────────
ALPINE_ISO_CACHED="/tmp/alpine-virt.iso"
SYSLINUX_SRC="/tmp/alpine-syslinux"
mkdir -p "$SYSLINUX_SRC"

if [[ -f "$ALPINE_ISO_CACHED" ]]; then
    ALPINE_MNT="/tmp/alpine-mnt-$$"
    mkdir -p "$ALPINE_MNT"
    mount -o loop "$ALPINE_ISO_CACHED" "$ALPINE_MNT" 2>/dev/null || true
    if [[ -d "$ALPINE_MNT/boot/syslinux" ]]; then
        cp "$ALPINE_MNT"/boot/syslinux/*.bin            "$ALPINE_MNT"/boot/syslinux/*.c32            "$SYSLINUX_SRC/" 2>/dev/null || true
    fi
    umount "$ALPINE_MNT" 2>/dev/null || true
    rmdir "$ALPINE_MNT"
fi

# Create syslinux config for BIOS boot
mkdir -p "$ISO_ROOT/boot/syslinux"
if [[ -d "$SYSLINUX_SRC" ]] && ls "$SYSLINUX_SRC"/*.bin &>/dev/null; then
    cp "$SYSLINUX_SRC"/*.bin "$SYSLINUX_SRC"/*.c32        "$ISO_ROOT/boot/syslinux/" 2>/dev/null || true
    ISOHDPFX="$ISO_ROOT/boot/syslinux/isohdpfx.bin"
    ISOLINUX="boot/syslinux/isolinux.bin"
else
    # Fallback: use system syslinux if available
    SYSLINUX_DATA=$(find /usr -name "isolinux.bin" 2>/dev/null | head -1)
    if [[ -n "$SYSLINUX_DATA" ]]; then
        cp "$(dirname $SYSLINUX_DATA)"/*.bin            "$(dirname $SYSLINUX_DATA)"/*.c32            "$ISO_ROOT/boot/syslinux/" 2>/dev/null || true
    fi
    ISOHDPFX="$ISO_ROOT/boot/syslinux/isohdpfx.bin"
    ISOLINUX="boot/syslinux/isolinux.bin"
fi

cat > "$ISO_ROOT/boot/syslinux/isolinux.cfg" << SYSEOF
DEFAULT caiman
LABEL caiman
  MENU LABEL ^Install Caiman OS
  LINUX /boot/vmlinuz
  APPEND quiet console=ttyS0,115200 caiman.mode=install
  INITRD /boot/initramfs.img
LABEL live
  MENU LABEL ^Live (no install)
  LINUX /boot/vmlinuz
  APPEND quiet console=ttyS0,115200 caiman.mode=live
  INITRD /boot/initramfs.img
SEOF

# ── Build hybrid BIOS+UEFI ISO ─────────────────────────────────────────
if [[ -f "$ISOHDPFX" ]]; then
    xorriso -as mkisofs \
        -iso-level 3 \
        -full-iso9660-filenames \
        -volid "CAIMAN${VERSION//./}" \
        -isohybrid-mbr "$ISOHDPFX" \
        -b boot/syslinux/isolinux.bin \
        -c boot/syslinux/boot.cat \
        -no-emul-boot \
        -boot-load-size 4 \
        -boot-info-table \
        -eltorito-alt-boot \
        -e boot/grub/efiboot.img \
        -no-emul-boot \
        -isohybrid-gpt-basdat \
        -append_partition 2 0xef "$ISO_ROOT/boot/grub/efiboot.img" \
        -output "$ISO_NAME" \
        "$ISO_ROOT"
else
    # EFI only fallback
    xorriso -as mkisofs \
        -iso-level 3 \
        -full-iso9660-filenames \
        -volid "CAIMAN${VERSION//./}" \
        -eltorito-alt-boot \
        -e boot/grub/efiboot.img \
        -no-emul-boot \
        -isohybrid-gpt-basdat \
        -append_partition 2 0xef "$ISO_ROOT/boot/grub/efiboot.img" \
        -output "$ISO_NAME" \
        "$ISO_ROOT"
fi

ISO_SIZE=$(du -sh "$ISO_NAME" | cut -f1)
ISO_SHA=$(sha256sum "$ISO_NAME" | cut -d' ' -f1)

ok "ISO created: $ISO_NAME ($ISO_SIZE)"
ok "SHA256: $ISO_SHA"

echo "$ISO_SHA  $ISO_NAME" > "${ISO_NAME}.sha256"
ok "Checksum saved: ${ISO_NAME}.sha256"

# ── Done ─────────────────────────────────────────────────────────────────
echo ""
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BRT}  🐊 Caimán OS ${VERSION} ISO ready!${NC}"
echo ""
echo -e "  ${GRN}File:${NC}    $(pwd)/$ISO_NAME"
echo -e "  ${GRN}Size:${NC}    $ISO_SIZE"
echo -e "  ${GRN}SHA256:${NC}  $ISO_SHA"
echo ""
echo -e "  ${DIM}Flash to USB:${NC}"
echo -e "  dd if=$ISO_NAME of=/dev/sdX bs=4M status=progress"
echo ""
echo -e "  ${DIM}Or test with QEMU:${NC}"
echo -e "  qemu-system-x86_64 -cdrom $ISO_NAME -m 4G -enable-kvm"
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
