#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  Caimán OS — Production installer
#  Named after the Cuban crocodile. Built for the cloud.
#
#  Usage (single command on a fresh server):
#    curl -fsSL https://raw.githubusercontent.com/teampmladv/caiman-os/main/install/scripts/install.sh | sudo bash
#
#  Or locally:
#    sudo ./install/scripts/install.sh
#
#  Supports: CentOS/RHEL 8+, Ubuntu 22.04+, Debian 12+
#  Requires: x86_64, KVM capable CPU, root access
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

BRT='\033[38;2;118;255;3m'; GRN='\033[38;2;76;175;80m'
DIM='\033[38;2;74;124;74m'; AMB='\033[38;2;255;179;0m'
RED='\033[38;2;239;83;80m'; NC='\033[0m'

CAIMAN_VERSION="0.7.0"
REGISTRY="ghcr.io/teampmladv"
INSTALL_DIR="/opt/caiman"
DATA_DIR="/var/lib/caiman"
RUN_DIR="/var/run/caiman"
LOG_DIR="/var/log/caiman"

step() { echo -e "\n${BRT}━━ $1${NC}"; }
ok()   { echo -e "  ${GRN}✓${NC} $1"; }
warn() { echo -e "  ${AMB}⚠${NC} $1"; }
die()  { echo -e "  ${RED}✗${NC} $1"; exit 1; }

echo -e "\n${BRT}🐊 Caimán OS v${CAIMAN_VERSION} — Production Installer${NC}"
echo -e "${DIM}   Named after the Cuban crocodile. Built for the cloud.${NC}\n"

# ── 0. Root check ─────────────────────────────────────────────────────────
[[ $EUID -eq 0 ]] || die "Run as root: sudo $0"

# ── 1. Detect OS + package manager ────────────────────────────────────────
step "Detecting system"

if   command -v dnf  &>/dev/null; then PKG="dnf"
elif command -v yum  &>/dev/null; then PKG="yum"
elif command -v apt-get &>/dev/null; then PKG="apt"
else die "Unsupported package manager (need dnf/yum/apt)"; fi

ARCH=$(uname -m)
[[ "$ARCH" == "x86_64" ]] || die "Only x86_64 is supported (got $ARCH)"

# Check KVM
if [[ ! -e /dev/kvm ]]; then
    warn "/dev/kvm not found — checking CPU virtualization support"
    if grep -qE "(vmx|svm)" /proc/cpuinfo; then
        modprobe kvm kvm_intel kvm_amd 2>/dev/null || true
        [[ -e /dev/kvm ]] || die "KVM not available — enable VT-x/AMD-V in BIOS"
    else
        die "CPU doesn't support hardware virtualization (no vmx/svm in /proc/cpuinfo)"
    fi
fi
ok "KVM available"

HOSTNAME=$(hostname -f 2>/dev/null || hostname)
MEMORY_GIB=$(awk '/MemTotal/{printf "%.0f", $2/1024/1024}' /proc/meminfo)
CPUS=$(nproc)
ok "Node: $HOSTNAME | ${CPUS} CPUs | ${MEMORY_GIB} GiB RAM"

# ── 2. System dependencies ────────────────────────────────────────────────
step "Installing system dependencies"

case "$PKG" in
  dnf|yum)
    $PKG install -y -q \
        iproute2 \
        kernel-devel \
        gcc \
        make \
        clang \
        llvm \
        elfutils-libelf-devel \
        curl \
        jq \
        bridge-utils \
        2>/dev/null || true
    ;;
  apt)
    apt-get update -qq
    apt-get install -y -qq \
        iproute2 \
        linux-headers-$(uname -r) \
        gcc \
        make \
        clang \
        llvm \
        libelf-dev \
        curl \
        jq \
        bridge-utils \
        2>/dev/null || true
    ;;
esac
ok "System dependencies installed"

# ── 3. Docker ─────────────────────────────────────────────────────────────
step "Checking Docker"

if ! command -v docker &>/dev/null; then
    echo -e "  ${DIM}Installing Docker…${NC}"
    curl -fsSL https://get.docker.com | sh
    systemctl enable --now docker
fi

if ! command -v docker &>/dev/null; then
    die "Docker installation failed"
fi
ok "Docker $(docker --version | cut -d' ' -f3 | tr -d ,)"

# ── 4. Directories ────────────────────────────────────────────────────────
step "Creating directories"

mkdir -p \
    "$INSTALL_DIR" \
    "$DATA_DIR/disks" \
    "$DATA_DIR/snapshots" \
    "$DATA_DIR/templates" \
    "$RUN_DIR" \
    "$LOG_DIR"

chmod 755 "$RUN_DIR"
ok "Directories created under /var/lib/caiman and /var/run/caiman"

# ── 5. Kernel module (caiman_net.ko) ──────────────────────────────────────
step "Building caiman_net kernel module"

KMOD_DIR="$INSTALL_DIR/kmod"
mkdir -p "$KMOD_DIR"

# Try to download pre-built kmod first
KMOD_URL="https://github.com/teampmladv/caiman-os/releases/download/v${CAIMAN_VERSION}/caiman_net-$(uname -r).ko"
if curl -fsSL -o "$KMOD_DIR/caiman_net.ko" "$KMOD_URL" 2>/dev/null; then
    ok "Downloaded pre-built caiman_net.ko for kernel $(uname -r)"
else
    warn "Pre-built kmod not found for $(uname -r) — will build from source"
    # Build is done separately via: make build-kmod
    # For now, skip and continue without XDP acceleration
    warn "XDP acceleration disabled — run 'make build-kmod' separately"
fi

# Load module if built
if [[ -f "$KMOD_DIR/caiman_net.ko" ]]; then
    if insmod "$KMOD_DIR/caiman_net.ko" 2>/dev/null; then
        ok "caiman_net.ko loaded (XDP acceleration active)"
    else
        warn "caiman_net.ko failed to load — continuing without XDP"
    fi
fi

# ── 6. Network bridge setup ───────────────────────────────────────────────
step "Setting up network bridge (caiman0)"

if ! ip link show caiman0 &>/dev/null; then
    ip link add name caiman0 type bridge
    ip link set caiman0 up
    ip addr add 10.100.0.1/24 dev caiman0
    ok "Bridge caiman0 created (10.100.0.1/24)"
else
    ok "Bridge caiman0 already exists"
fi

# Enable IP forwarding
echo 1 > /proc/sys/net/ipv4/ip_forward
# Persist
grep -q "net.ipv4.ip_forward" /etc/sysctl.conf || \
    echo "net.ipv4.ip_forward = 1" >> /etc/sysctl.conf

# NAT for VMs
iptables -t nat -A POSTROUTING -s 10.100.0.0/24 ! -d 10.100.0.0/24 -j MASQUERADE 2>/dev/null || true
ok "IP forwarding + NAT enabled"

# ── 7. Download caiman-vmm binary ─────────────────────────────────────────
step "Installing caiman-vmm binary"

VMM_URL="https://github.com/teampmladv/caiman-os/releases/download/v${CAIMAN_VERSION}/caiman-vmm-linux-amd64"
if curl -fsSL -o /usr/local/bin/caiman-vmm "$VMM_URL" 2>/dev/null; then
    chmod +x /usr/local/bin/caiman-vmm
    ok "caiman-vmm installed"
else
    # Try pulling from container
    warn "Extracting caiman-vmm from container image"
    docker create --name caiman-extract "${REGISTRY}/caiman-vmm:${CAIMAN_VERSION}" 2>/dev/null && \
    docker cp caiman-extract:/usr/local/bin/caiman-vmm /usr/local/bin/caiman-vmm && \
    docker rm caiman-extract 2>/dev/null && \
    chmod +x /usr/local/bin/caiman-vmm && \
    ok "caiman-vmm extracted from container" || \
    warn "caiman-vmm not installed — run VMs via docker"
fi

# ── 8. Pull OCI images ────────────────────────────────────────────────────
step "Pulling Caimán OS images"

IMAGES=(caiman-api caiman-ui caiman-drs caiman-bts caiman-mcp)
for img in "${IMAGES[@]}"; do
    echo -ne "  ${DIM}Pulling ${img}…${NC} "
    if docker pull -q "${REGISTRY}/${img}:${CAIMAN_VERSION}" &>/dev/null; then
        echo -e "${GRN}✓${NC}"
    else
        echo -e "${AMB}⚠ failed${NC}"
    fi
done

# ── 9. Install docker-compose ─────────────────────────────────────────────
step "Setting up docker-compose"

if ! command -v docker-compose &>/dev/null && ! docker compose version &>/dev/null 2>&1; then
    curl -fsSL \
        "https://github.com/docker/compose/releases/download/v2.27.0/docker-compose-linux-x86_64" \
        -o /usr/local/bin/docker-compose
    chmod +x /usr/local/bin/docker-compose
fi

# Copy compose file
if [[ -f "$(dirname "$0")/../../docker-compose.yml" ]]; then
    cp "$(dirname "$0")/../../docker-compose.yml" "$INSTALL_DIR/"
    ok "docker-compose.yml installed to $INSTALL_DIR"
fi

# ── 10. Download first VM kernel ──────────────────────────────────────────
step "Setting up test VM kernel"

VM_KERNEL="/var/lib/caiman/vmlinuz"
if [[ ! -f "$VM_KERNEL" ]] && [[ -f /boot/vmlinuz-$(uname -r) ]]; then
    cp /boot/vmlinuz-$(uname -r) "$VM_KERNEL"
    ok "Kernel copied: $(uname -r)"
elif [[ ! -f "$VM_KERNEL" ]]; then
    warn "No kernel found at /boot/vmlinuz-$(uname -r)"
    warn "Copy a bzImage manually: cp /boot/vmlinuz-* /var/lib/caiman/vmlinuz"
fi

# Create a test disk image
TEST_DISK="/var/lib/caiman/disks/test.img"
if [[ ! -f "$TEST_DISK" ]]; then
    dd if=/dev/zero of="$TEST_DISK" bs=1M count=4096 status=none 2>/dev/null
    ok "Test disk created: $TEST_DISK (4 GiB)"
fi

# ── 11. Start the stack ───────────────────────────────────────────────────
step "Starting Caimán OS stack"

cd "$INSTALL_DIR"
if [[ -f "docker-compose.yml" ]]; then
    docker compose up -d 2>/dev/null || docker-compose up -d
    sleep 3

    # Health check
    if curl -sf http://localhost:8765/health &>/dev/null; then
        ok "caiman-api responding"
    else
        warn "caiman-api not responding yet — may need a few seconds"
    fi
else
    warn "docker-compose.yml not found at $INSTALL_DIR"
    warn "Run manually: cd /opt/caiman && docker compose up -d"
fi

# ── Done ─────────────────────────────────────────────────────────────────
echo
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${BRT}  🐊 Caimán OS v${CAIMAN_VERSION} installed!${NC}"
echo
echo -e "  ${GRN}Dashboard:${NC}    http://$(hostname -I | awk '{print $1}'):3000"
echo -e "  ${GRN}API:${NC}          http://$(hostname -I | awk '{print $1}'):8765"
echo -e "  ${GRN}Grafana:${NC}      http://$(hostname -I | awk '{print $1}'):3001  (admin/caiman)"
echo
echo -e "  ${DIM}Create your first VM:${NC}"
echo -e "  curl -X POST http://localhost:8765/api/vms \\"
echo -e "    -H 'Content-Type: application/json' \\"
echo -e "    -d '{\"name\":\"vm-01\",\"cpus\":2,\"memMib\":512,\"kernel\":\"/var/lib/caiman/vmlinuz\"}'"
echo
echo -e "  ${DIM}Kernel module (XDP acceleration):${NC}"
echo -e "  make build-kmod   # builds caiman_net.ko for $(uname -r)"
echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
