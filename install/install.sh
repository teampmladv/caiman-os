#!/bin/bash
set -e

# ==========================================================================
#  Caiman OS -- Universal Installer v1.2.0
#  curl -fsSL https://caimanos.com/install.sh | sudo bash
#
#  Supports:
#    x86_64: Ubuntu 20.04+, Debian 11+, CentOS 8+, RHEL 8+, Alpine 3.18+
#    aarch64: Raspberry Pi 4/5, Mac Mini M1/M2, AWS Graviton
#
#  One command. Any PC. Any architecture.
# ==========================================================================

CAIMAN_VERSION="1.2.0"
GITHUB_REPO="teampmladv/caiman-os"
INSTALL_DIR="/opt/caiman"
DATA_DIR="/var/lib/caiman"
RUN_DIR="/var/run/caiman"
CONFIG_DIR="/etc/caiman"

G='\033[38;2;22;163;74m'
C='\033[38;2;0;212;255m'
W='\033[38;2;248;250;252m'
D='\033[2m'
R='\033[38;2;220;38;38m'
A='\033[38;2;217;119;6m'
NC='\033[0m'

# ── Logo ───────────────────────────────────────────────────────────────────
clear
echo ""
echo -e "${G}     ______      _                    ____  ____${NC}"
echo -e "${G}    / ____/___ _(_)___ ___  ____ ___ / __ \/ ___/${NC}"
echo -e "${G}   / /   / \`_ \`/ / __ \`__ \/ __ \`__ / / / /\__ \ ${NC}"
echo -e "${G}  / /___/ /_/ / / / / / / / / / / / / /_/ /___/ /${NC}"
echo -e "${G}  \____/\__,_/_/_/ /_/ /_/_/ /_/ /_/\____//____/ ${NC}"
echo ""
echo -e "  ${C}v${CAIMAN_VERSION}${NC}  ${D}KVM hypervisor without QEMU${NC}"
echo -e "  ${D}Named after the Cuban crocodile. Works on any PC.${NC}"
echo -e "${G}  ==========================================================${NC}"
echo ""

# ── Helpers ────────────────────────────────────────────────────────────────
ok()   { echo -e "  ${G}[OK]${NC} $1"; }
info() { echo -e "  ${D}[..]${NC} $1"; }
warn() { echo -e "  ${A}[!!]${NC} $1"; }
fail() { echo -e "  ${R}[XX] ERROR:${NC} $1"; exit 1; }
step() { echo -e "\n${G}  == $1${NC}"; }

# ── Root check ─────────────────────────────────────────────────────────────
[ "$EUID" -eq 0 ] || fail "Run as root: curl -fsSL https://caimanos.com/install.sh | sudo bash"

# ── Architecture ───────────────────────────────────────────────────────────
step "Detecting hardware"
ARCH=$(uname -m)
case $ARCH in
    x86_64)  IMAGE_ARCH="amd64"; ok "Architecture: x86_64" ;;
    aarch64) IMAGE_ARCH="arm64"; ok "Architecture: aarch64 (ARM)" ;;
    armv7l)  IMAGE_ARCH="arm/v7"; ok "Architecture: ARMv7" ;;
    *)       fail "Unsupported architecture: $ARCH" ;;
esac

# CPU
CPU_MODEL=$(grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || echo "Unknown")
CPU_CORES=$(nproc 2>/dev/null || echo 1)
ok "CPU: $CPU_MODEL ($CPU_CORES cores)"

# RAM
RAM_GIB=$(awk '/MemTotal/{printf "%.0f", $2/1024/1024}' /proc/meminfo 2>/dev/null || echo 0)
if [ "$RAM_GIB" -lt 2 ]; then
    warn "Low RAM: ${RAM_GIB}GiB -- minimum 2GiB recommended"
else
    ok "RAM: ${RAM_GIB}GiB"
fi

# KVM
if [ -e /dev/kvm ] || grep -qE "vmx|svm" /proc/cpuinfo 2>/dev/null; then
    ok "KVM: virtualization supported"
    KVM_AVAILABLE=1
    DEMO_MODE="false"
else
    warn "KVM not detected -- will run in demo mode"
    warn "Enable VT-x/AMD-V in BIOS for real VM support"
    KVM_AVAILABLE=0
    DEMO_MODE="true"
fi

# Disk
DISK_FREE=$(df -BG / | tail -1 | awk '{print $4}' | tr -d 'G')
if [ "${DISK_FREE:-0}" -lt 10 ]; then
    warn "Low disk space: ${DISK_FREE}GB free -- minimum 10GB recommended"
else
    ok "Disk: ${DISK_FREE}GB free"
fi

# ── Network detection ──────────────────────────────────────────────────────
step "Detecting network"

# Find primary interface
UPLINK=$(ip route show default 2>/dev/null | awk '/dev/{print $5}' | head -1)
if [ -z "$UPLINK" ]; then
    for iface in eth0 ens3 enp3s0 wlan0 wlp2s0; do
        [ -d "/sys/class/net/$iface" ] && UPLINK=$iface && break
    done
fi
UPLINK=${UPLINK:-eth0}

# Get host IP
HOST_IP=$(ip route get 1 2>/dev/null | awk '{print $7}' | head -1 || hostname -I 2>/dev/null | awk '{print $1}' || echo "localhost")

ok "Primary interface: $UPLINK"
ok "Host IP: $HOST_IP"

# ── OS detection ───────────────────────────────────────────────────────────
step "Detecting OS"

if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO=$ID
    ok "OS: ${PRETTY_NAME:-$ID}"
elif [ -f /etc/alpine-release ]; then
    DISTRO="alpine"
    ok "OS: Alpine $(cat /etc/alpine-release)"
else
    DISTRO="unknown"
    warn "Unknown OS -- will attempt generic install"
fi

# ── Install Docker ─────────────────────────────────────────────────────────
step "Installing Docker"

if command -v docker &>/dev/null; then
    DOCKER_VER=$(docker --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+' | head -1)
    ok "Docker already installed: v${DOCKER_VER}"
else
    info "Installing Docker for $DISTRO..."
    case $DISTRO in
        ubuntu|debian|raspbian)
            apt-get update -qq 2>/dev/null
            apt-get install -y -qq curl ca-certificates 2>/dev/null
            curl -fsSL https://get.docker.com | sh >/dev/null 2>&1
            ;;
        centos|rhel|rocky|almalinux|fedora)
            if command -v dnf &>/dev/null; then
                dnf install -y -q yum-utils 2>/dev/null
                yum-config-manager --add-repo https://download.docker.com/linux/centos/docker-ce.repo 2>/dev/null
                dnf install -y -q docker-ce docker-ce-cli containerd.io docker-compose-plugin 2>/dev/null
            else
                yum install -y -q docker 2>/dev/null
            fi
            ;;
        alpine)
            apk add --quiet docker docker-compose 2>/dev/null
            rc-update add docker default 2>/dev/null || true
            service docker start 2>/dev/null || true
            ;;
        *)
            curl -fsSL https://get.docker.com | sh >/dev/null 2>&1 || \
                fail "Could not install Docker. Install manually: https://docs.docker.com/engine/install/"
            ;;
    esac

    systemctl enable --now docker 2>/dev/null || true
    ok "Docker installed"
fi

# ── Create directories ─────────────────────────────────────────────────────
step "Setting up directories"

mkdir -p \
    "$INSTALL_DIR" \
    "$DATA_DIR/disks" \
    "$DATA_DIR/kernels" \
    "$DATA_DIR/ipam" \
    "$RUN_DIR" \
    "$CONFIG_DIR"

ok "Directories created"

# ── Generate credentials ───────────────────────────────────────────────────
step "Generating security credentials"

# Generate JWT secret
if [ -f "$CONFIG_DIR/jwt-secret" ]; then
    JWT_SECRET=$(cat "$CONFIG_DIR/jwt-secret")
    ok "JWT secret: existing ($(echo $JWT_SECRET | cut -c1-8)...)"
else
    JWT_SECRET=$(cat /dev/urandom | tr -dc 'a-f0-9' | head -c 64 2>/dev/null || \
                 openssl rand -hex 32 2>/dev/null || \
                 date +%s | sha256sum | cut -c1-64)
    echo "$JWT_SECRET" > "$CONFIG_DIR/jwt-secret"
    chmod 600 "$CONFIG_DIR/jwt-secret"
    ok "JWT secret generated"
fi

# ── Write config ───────────────────────────────────────────────────────────
cat > "$CONFIG_DIR/config.toml" << EOF
[node]
hostname  = "$(hostname)"
arch      = "$ARCH"
version   = "$CAIMAN_VERSION"
kvm       = $KVM_AVAILABLE

[api]
listen    = "0.0.0.0:8765"
demo_mode = $DEMO_MODE

[network]
mode      = "nat"
uplink    = "$UPLINK"
bridge    = "caiman0"
subnet    = "10.100.0.0/24"
gateway   = "10.100.0.1"

[storage]
data_dir  = "$DATA_DIR"
EOF
ok "Config written"

# ── Create docker-compose.yml ──────────────────────────────────────────────
step "Configuring services"

# Detect compose command
if docker compose version &>/dev/null 2>&1; then
    COMPOSE="docker compose"
elif command -v docker-compose &>/dev/null; then
    COMPOSE="docker-compose"
else
    info "Installing docker-compose..."
    curl -fsSL "https://github.com/docker/compose/releases/latest/download/docker-compose-$(uname -s)-$(uname -m)" \
        -o /usr/local/bin/docker-compose 2>/dev/null
    chmod +x /usr/local/bin/docker-compose
    COMPOSE="docker-compose"
fi

KVM_DEVICE=""
[ "$KVM_AVAILABLE" = "1" ] && KVM_DEVICE="      - /dev/kvm:/dev/kvm"

cat > "$INSTALL_DIR/docker-compose.yml" << EOF
services:
  caimanapi:
    image: ghcr.io/teampmladv/caiman-api:${CAIMAN_VERSION}
    container_name: caimanapi
    restart: unless-stopped
    network_mode: host
    pid: host
    cap_add:
      - NET_ADMIN
      - SYS_ADMIN
    environment:
      RUST_LOG: caiman_api=info
      DEMO_MODE: "${DEMO_MODE}"
      CAIMAN_JWT_SECRET: "${JWT_SECRET}"
      CAIMAN_NET_MODE: "nat"
      CAIMAN_UPLINK: "${UPLINK}"
    volumes:
      - /var/run/caiman:/var/run/caiman
      - /var/lib/caiman:/var/lib/caiman
      - /etc/caiman:/etc/caiman:ro
${KVM_DEVICE}

  caimangrafana:
    image: grafana/grafana:10.4.0
    container_name: caimangrafana
    restart: unless-stopped
    ports:
      - "3001:3000"
    environment:
      GF_SECURITY_ADMIN_PASSWORD: caiman
      GF_USERS_ALLOW_SIGN_UP: "false"
    volumes:
      - grafana_data:/var/lib/grafana

volumes:
  grafana_data:
EOF
ok "docker-compose.yml created"

# ── Pull images ────────────────────────────────────────────────────────────
step "Pulling Caiman OS images"
info "Downloading caiman-api:${CAIMAN_VERSION}..."
cd "$INSTALL_DIR"
$COMPOSE pull -q 2>/dev/null || $COMPOSE pull
ok "Images downloaded"

# ── Start services ─────────────────────────────────────────────────────────
step "Starting Caiman OS"
$COMPOSE up -d
sleep 3

# Check API health
HEALTH=$(curl -sf "http://localhost:8765/health" 2>/dev/null || echo "")
if echo "$HEALTH" | grep -q "ok"; then
    API_VERSION=$(echo "$HEALTH" | grep -o '"version":"[^"]*"' | cut -d'"' -f4)
    API_DEMO=$(echo "$HEALTH" | grep -q '"demo":true' && echo "demo" || echo "production")
    ok "API running: v${API_VERSION} (${API_DEMO})"
else
    warn "API not responding yet -- may take a few seconds"
fi

# ── Generate first admin token ─────────────────────────────────────────────
step "Generating admin token"

sleep 2
# Enable bootstrap temporarily
$COMPOSE down -q 2>/dev/null
$COMPOSE up -d --no-deps caimanapi \
    -e CAIMAN_BOOTSTRAP_ALLOWED=1 \
    -e CAIMAN_JWT_SECRET="$JWT_SECRET" 2>/dev/null || true

# Try to get bootstrap token
BOOTSTRAP_RESPONSE=""
for i in 1 2 3 4 5; do
    sleep 2
    BOOTSTRAP_RESPONSE=$(curl -sf -X POST "http://localhost:8765/auth/bootstrap" \
        -H "Content-Type: application/json" \
        -d "{\"name\":\"admin\",\"role\":\"admin\",\"expires\":\"1y\",\"cluster\":\"$(hostname)\"}" 2>/dev/null || echo "")
    [ -n "$BOOTSTRAP_RESPONSE" ] && break
done

ADMIN_TOKEN=""
if echo "$BOOTSTRAP_RESPONSE" | grep -q "caim_"; then
    ADMIN_TOKEN=$(echo "$BOOTSTRAP_RESPONSE" | grep -o '"token":"[^"]*"' | cut -d'"' -f4)
    echo "$ADMIN_TOKEN" > "$CONFIG_DIR/admin-token"
    chmod 600 "$CONFIG_DIR/admin-token"
    ok "Admin token generated"
fi

# Restart without bootstrap
$COMPOSE down -q 2>/dev/null
$COMPOSE up -d
ok "Services restarted (bootstrap disabled)"

# ── Done ───────────────────────────────────────────────────────────────────
echo ""
echo -e "${G}  ==========================================================${NC}"
echo -e "${G}  🐊 Caiman OS v${CAIMAN_VERSION} installed successfully!${NC}"
echo -e "${G}  ==========================================================${NC}"
echo ""
echo -e "  ${D}Mode:${NC}      $([ "$KVM_AVAILABLE" = "1" ] && echo "${G}Production (KVM enabled)${NC}" || echo "${A}Demo (no KVM)${NC}")"
echo -e "  ${D}Host:${NC}      $HOST_IP"
echo -e "  ${D}API:${NC}       ${C}http://${HOST_IP}:8765${NC}"
echo -e "  ${D}Grafana:${NC}   ${C}http://${HOST_IP}:3001${NC}  ${D}(admin / caiman)${NC}"
echo ""

if [ -n "$ADMIN_TOKEN" ]; then
echo -e "  ${A}Admin token (save this -- shown only once!):${NC}"
echo -e "  ${W}${ADMIN_TOKEN}${NC}"
echo ""
echo -e "  ${D}Connect from caiman-ui:${NC}"
echo -e "  ${D}  URL:${NC}   ${C}http://${HOST_IP}:8765${NC}"
echo -e "  ${D}  Token:${NC} ${C}${ADMIN_TOKEN:0:32}...${NC}"
else
echo -e "  ${D}Generate admin token:${NC}"
echo -e "  ${C}  curl -X POST http://localhost:8765/auth/bootstrap \\"
echo -e "    -H 'Content-Type: application/json' \\"
echo -e "    -d '{\"name\":\"admin\",\"role\":\"admin\",\"expires\":\"1y\"}' \\"
echo -e "    # (requires CAIMAN_BOOTSTRAP_ALLOWED=1)${NC}"
fi

echo ""
echo -e "  ${D}Quick test:${NC}"
echo -e "  ${C}  curl http://localhost:8765/health${NC}"
echo ""
echo -e "  ${D}Create your first VM:${NC}"
echo -e "  ${C}  curl -X POST http://localhost:8765/api/vms \\"
echo -e "    -H 'Authorization: Bearer \$TOKEN' \\"
echo -e "    -H 'Content-Type: application/json' \\"
echo -e "    -d '{\"name\":\"vm-01\",\"cpus\":1,\"memMib\":512}'${NC}"
echo ""
echo -e "  ${D}Docs:${NC} ${C}https://github.com/${GITHUB_REPO}${NC}"
echo -e "${G}  ==========================================================${NC}"
echo ""
