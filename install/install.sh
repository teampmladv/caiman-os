#!/bin/bash
set -e

# ============================================================
#  Caiman OS -- Install Script
#  https://caimanos.com/install.sh | sudo bash
#  Supports: Ubuntu 20.04+, Debian 11+, CentOS 8+, RHEL 8+
# ============================================================

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
DIM='\033[2m'
BOLD='\033[1m'
NC='\033[0m'

CAIMAN_VERSION="1.0.3"
GITHUB_REPO="teampmladv/caiman-os"
INSTALL_DIR="/opt/caiman"
BIN_DIR="/usr/local/bin"
DATA_DIR="/var/lib/caiman"
RUN_DIR="/var/run/caiman"

print_logo() {
  echo -e "${GREEN}"
  cat << 'EOF'
   ____      _                    ___  ____
  / ___|__ _(_)_ __ ___   __ _ _ |  _ \/ ___|
 | |   / _` | | '_ ` _ \ / _` | '_ \| |
 | |__| (_| | | | | | | | (_| | | | | |___
  \____\__,_|_|_| |_| |_|\__,_|_| |_|\____|

  KVM Hypervisor Without QEMU  --  v1.0.3
  Named after the Cuban crocodile. Built for the cloud.
EOF
  echo -e "${NC}"
}

log()  { echo -e "${GREEN}  ✓${NC} $1"; }
info() { echo -e "${DIM}  →${NC} $1"; }
warn() { echo -e "${YELLOW}  !${NC} $1"; }
fail() { echo -e "${RED}  ✗ ERROR:${NC} $1"; exit 1; }

check_root() {
  [ "$EUID" -eq 0 ] || fail "Please run as root: curl -fsSL https://caimanos.com/install.sh | sudo bash"
}

check_arch() {
  ARCH=$(uname -m)
  [ "$ARCH" = "x86_64" ] || fail "Caiman OS requires x86_64 (got: $ARCH)"
  log "Architecture: $ARCH"
}

check_kvm() {
  if [ -e /dev/kvm ]; then
    log "KVM: /dev/kvm available"
  else
    warn "KVM not available -- running in demo mode (no VMs will actually start)"
    warn "Enable VT-x/AMD-V in BIOS for full functionality"
  fi
}

detect_distro() {
  if [ -f /etc/os-release ]; then
    . /etc/os-release
    DISTRO=$ID
    DISTRO_VERSION=$VERSION_ID
  else
    fail "Cannot detect OS distribution"
  fi
  log "OS: $PRETTY_NAME"
}

install_docker() {
  if command -v docker &>/dev/null; then
    log "Docker: $(docker --version | cut -d' ' -f3 | tr -d ',')"
    return
  fi

  info "Installing Docker..."
  case $DISTRO in
    ubuntu|debian)
      apt-get update -qq
      apt-get install -y -qq curl ca-certificates
      curl -fsSL https://get.docker.com | sh
      ;;
    centos|rhel|rocky|almalinux)
      dnf install -y -q yum-utils
      yum-config-manager --add-repo https://download.docker.com/linux/centos/docker-ce.repo
      dnf install -y -q docker-ce docker-ce-cli containerd.io docker-compose-plugin
      ;;
    fedora)
      dnf install -y -q docker docker-compose
      ;;
    *)
      fail "Unsupported distro: $DISTRO. Install Docker manually then re-run."
      ;;
  esac

  systemctl enable --now docker
  log "Docker installed and started"
}

install_caiman_vmm() {
  info "Downloading caiman-vmm v${CAIMAN_VERSION}..."

  VMM_URL="https://github.com/${GITHUB_REPO}/releases/download/v${CAIMAN_VERSION}/caiman-vmm"

  if curl -fsSL --head "$VMM_URL" &>/dev/null; then
    curl -fsSL "$VMM_URL" -o "${BIN_DIR}/caiman-vmm"
    chmod +x "${BIN_DIR}/caiman-vmm"
    log "caiman-vmm installed: $(caiman-vmm --version 2>&1 | head -1)"
  else
    # Fallback: extract from Docker image
    info "Extracting from Docker image..."
    docker pull "ghcr.io/${GITHUB_REPO%-*}/caiman-vmm:${CAIMAN_VERSION}" -q
    docker create --name _caiman_tmp "ghcr.io/${GITHUB_REPO%-*}/caiman-vmm:${CAIMAN_VERSION}"
    docker cp _caiman_tmp:/usr/local/bin/caiman-vmm "${BIN_DIR}/caiman-vmm"
    docker rm _caiman_tmp
    chmod +x "${BIN_DIR}/caiman-vmm"
    log "caiman-vmm installed: $(caiman-vmm --version 2>&1 | head -1)"
  fi
}

setup_directories() {
  mkdir -p "$INSTALL_DIR" "$DATA_DIR/disks" "$DATA_DIR/kernels" "$RUN_DIR"
  log "Directories created"
}

create_compose() {
  cat > "${INSTALL_DIR}/docker-compose.yml" << EOF
services:

  caimanapi:
    image: ghcr.io/teampmladv/caiman-api:${CAIMAN_VERSION}
    container_name: caimanapi
    restart: unless-stopped
    network_mode: host
    pid: host
    privileged: true
    environment:
      RUST_LOG: caiman_api=info
      DEMO_MODE: "false"
    volumes:
      - /var/run/caiman:/var/run/caiman
      - /var/lib/caiman:/var/lib/caiman
      - /dev/kvm:/dev/kvm
      - /usr/local/bin/caiman-vmm:/usr/local/bin/caiman-vmm:ro

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
  log "docker-compose.yml created"
}

pull_and_start() {
  info "Pulling images (this may take a minute)..."
  cd "$INSTALL_DIR"
  docker compose pull -q
  docker compose up -d
  log "Caiman OS started"
}

get_ip() {
  hostname -I 2>/dev/null | awk '{print $1}' || ip route get 1 | awk '{print $7}' || echo "localhost"
}

print_success() {
  IP=$(get_ip)
  echo ""
  echo -e "${GREEN}${BOLD}  ============================================${NC}"
  echo -e "${GREEN}${BOLD}   Caiman OS v${CAIMAN_VERSION} installed successfully!${NC}"
  echo -e "${GREEN}${BOLD}  ============================================${NC}"
  echo ""
  echo -e "  ${BOLD}API:${NC}       http://${IP}:8765"
  echo -e "  ${BOLD}Grafana:${NC}   http://${IP}:3001  (admin / caiman)"
  echo ""
  echo -e "  ${DIM}Create a VM:${NC}"
  echo -e "  ${GREEN}curl -X POST http://localhost:8765/api/vms \\"
  echo -e "    -H 'Content-Type: application/json' \\"
  echo -e "    -d '{\"name\":\"vm-01\",\"cpus\":1,\"memMib\":512}'${NC}"
  echo ""
  echo -e "  ${DIM}Docs:${NC} https://github.com/${GITHUB_REPO}#readme"
  echo -e "  ${DIM}Issues:${NC} https://github.com/${GITHUB_REPO}/issues"
  echo ""
}

# ── Main ────────────────────────────────────────────────────

print_logo
check_root
check_arch
detect_distro
check_kvm
install_docker
setup_directories
install_caiman_vmm
create_compose
pull_and_start
print_success
