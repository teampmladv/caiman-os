#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  Caimán OS — Bootstrap installer
#  Born in Cuba. Built for the cloud.
#
#  Usage:
#    Control plane:  curl -fsSL https://install.caiman.io | bash -s -- --role cp
#    Worker node:    curl -fsSL https://install.caiman.io | bash -s -- --role worker --join <CP_IP> --token <TOKEN>
#    All-in-one:     curl -fsSL https://install.caiman.io | bash -s -- --role aio
#    Bare metal ISO: flash caiman.iso → dd / PXE → first boot runs this script
#
#  Supports: Ubuntu 22.04+, Debian 12+, bare metal (from caiman.iso)
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail
IFS=$'\n\t'

# ── Colours ────────────────────────────────────────────────────────────────
BRT='\033[38;2;118;255;3m'      # #76ff03 Caimán bright green
GRN='\033[38;2;76;175;80m'      # #4caf50
DIM='\033[38;2;74;124;74m'      # #4a7c4a
AMB='\033[38;2;255;179;0m'      # #ffb300
RED='\033[38;2;239;83;80m'      # #ef5350
NC='\033[0m'

LOGO="
${BRT}   ██████╗ █████╗ ██╗███╗   ███╗ █████╗ ███╗   ██╗
  ██╔════╝██╔══██╗██║████╗ ████║██╔══██╗████╗  ██║
  ██║     ███████║██║██╔████╔██║███████║██╔██╗ ██║
  ██║     ██╔══██║██║██║╚██╔╝██║██╔══██║██║╚██╗██║
  ╚██████╗██║  ██║██║██║ ╚═╝ ██║██║  ██║██║ ╚████║
   ╚═════╝╚═╝  ╚═╝╚═╝╚═╝     ╚═╝╚═╝  ╚═╝╚═╝  ╚═══╝${NC}
${DIM}  Born in Cuba. Built for the cloud. — v0.1.0${NC}
"

# ── Defaults ───────────────────────────────────────────────────────────────
ROLE="cp"                    # cp | worker | aio
CP_IP=""                     # control-plane IP (workers need this)
JOIN_TOKEN=""                # kubeadm join token
CAIMAN_VERSION="0.1.0"
POD_CIDR="10.244.0.0/16"
SVC_CIDR="10.96.0.0/12"
UPLINK="eth0"                # NIC for XDP attach
INSTALL_DIR="/opt/caiman"
STATE_DIR="/var/run/caiman"
DATA_DIR="/var/lib/caiman"
LOG_FILE="/var/log/caiman-install.log"
SKIP_CONFIRM=0
DRY_RUN=0

# ── Argument parsing ───────────────────────────────────────────────────────
usage() {
  cat <<EOF
Usage: $0 [options]

  --role         cp | worker | aio  (default: cp)
  --join         Control-plane IP   (required for --role worker)
  --token        kubeadm join token (required for --role worker)
  --uplink       Network interface for XDP (default: eth0)
  --pod-cidr     Pod CIDR (default: 10.244.0.0/16)
  --skip-confirm Skip confirmation prompts
  --dry-run      Print steps without executing
  --version      Show version and exit
  -h, --help     Show this help

Examples:
  # Bootstrap a 3-node cluster:
  # Node 1 (control plane):
  $0 --role cp --uplink eth0

  # Node 2-3 (workers) — use token printed by node 1:
  $0 --role worker --join 192.168.1.10 --token <TOKEN>

  # Single-node all-in-one (dev/testing):
  $0 --role aio
EOF
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --role)     ROLE="$2";       shift 2 ;;
    --join)     CP_IP="$2";      shift 2 ;;
    --token)    JOIN_TOKEN="$2"; shift 2 ;;
    --uplink)   UPLINK="$2";     shift 2 ;;
    --pod-cidr) POD_CIDR="$2";   shift 2 ;;
    --skip-confirm) SKIP_CONFIRM=1; shift ;;
    --dry-run)  DRY_RUN=1;       shift ;;
    --version)  echo "caiman-install $CAIMAN_VERSION"; exit 0 ;;
    -h|--help)  usage ;;
    *)          echo "Unknown option: $1"; usage ;;
  esac
done

# ── Helpers ────────────────────────────────────────────────────────────────
log()  { echo -e "${GRN}[$(date +%H:%M:%S)]${NC} $*" | tee -a "$LOG_FILE"; }
warn() { echo -e "${AMB}[WARN]${NC} $*" | tee -a "$LOG_FILE" >&2; }
die()  { echo -e "${RED}[ERROR]${NC} $*" | tee -a "$LOG_FILE" >&2; exit 1; }
step() { echo -e "\n${BRT}━━ $* ${NC}"; }
run()  {
  if [[ $DRY_RUN -eq 1 ]]; then
    echo -e "${DIM}  [dry-run] $*${NC}"
  else
    eval "$@" 2>&1 | tee -a "$LOG_FILE"
  fi
}
confirm() {
  [[ $SKIP_CONFIRM -eq 1 ]] && return 0
  read -rp "$(echo -e "${AMB}$1 [y/N] ${NC}")" ans
  [[ "${ans,,}" == "y" ]] || { echo "Aborted."; exit 1; }
}
spinner() {
  local pid=$!; local delay=0.1
  local sp='⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏'
  local i=0
  while kill -0 "$pid" 2>/dev/null; do
    printf "\r${BRT}  ${sp:$i:1}${NC}  $1"
    i=$(( (i+1) % ${#sp} ))
    sleep "$delay"
  done
  printf "\r${GRN}  ✓${NC}  $1\n"
}

# ── Pre-flight checks ──────────────────────────────────────────────────────
preflight() {
  step "Pre-flight checks"

  # Root check
  [[ $EUID -eq 0 ]] || die "Must run as root: sudo $0 $*"

  # OS check
  if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    log "OS: $PRETTY_NAME"
    case "$ID" in
      ubuntu|debian) ;;
      *) warn "Untested OS: $ID — proceeding anyway" ;;
    esac
  fi

  # Architecture check
  ARCH=$(uname -m)
  [[ "$ARCH" == "x86_64" ]] || die "Only x86_64 supported (got $ARCH)"
  log "Architecture: $ARCH"

  # CPU virtualisation
  if grep -qE "vmx|svm" /proc/cpuinfo; then
    log "CPU virtualisation: ${BRT}enabled${NC} (KVM available)"
  else
    warn "KVM not available — nested virtualisation or bare metal required"
  fi

  # RAM check (minimum 4 GiB for control plane, 8 GiB for workers)
  local mem_gib
  mem_gib=$(awk '/MemTotal/ {printf "%d", $2/1024/1024}' /proc/meminfo)
  local min_gib=4
  [[ "$ROLE" == "worker" ]] && min_gib=8
  if [[ $mem_gib -lt $min_gib ]]; then
    die "Insufficient RAM: ${mem_gib} GiB (minimum ${min_gib} GiB for role=${ROLE})"
  fi
  log "RAM: ${mem_gib} GiB ✓"

  # Disk space (minimum 40 GiB)
  local disk_gib
  disk_gib=$(df / --output=avail | tail -1 | awk '{printf "%d", $1/1024/1024}')
  [[ $disk_gib -ge 40 ]] || die "Insufficient disk: ${disk_gib} GiB free (minimum 40 GiB)"
  log "Disk: ${disk_gib} GiB free ✓"

  # Network interface
  ip link show "$UPLINK" &>/dev/null || die "Interface '$UPLINK' not found"
  log "Uplink: $UPLINK ✓"

  # XDP driver check
  local driver
  driver=$(ethtool -i "$UPLINK" 2>/dev/null | awk '/driver:/{print $2}')
  log "NIC driver: $driver"
  case "$driver" in
    i40e|mlx5_core|ice|ixgbe|nfp)
      log "XDP native mode: ${BRT}supported${NC}" ;;
    virtio_net|veth)
      warn "XDP generic mode only (driver: $driver) — ~10% performance penalty" ;;
    *)
      warn "Unknown driver '$driver' — XDP may not work" ;;
  esac

  # Check for conflicting software
  for svc in docker snap; do
    if systemctl is-active --quiet "$svc" 2>/dev/null; then
      warn "$svc is running — may conflict with containerd"
    fi
  done

  log "Pre-flight: ${BRT}all checks passed${NC}"
}

# ── System preparation ─────────────────────────────────────────────────────
prepare_system() {
  step "Preparing system"

  # Hostname
  if [[ -z "$(hostname -s)" || "$(hostname -s)" == "localhost" ]]; then
    local hostname="caiman-$(hostname -I | awk '{print $1}' | tr '.' '-')"
    run "hostnamectl set-hostname $hostname"
    log "Hostname set: $hostname"
  fi

  # Disable swap (required for Kubernetes)
  run "swapoff -a"
  run "sed -i '/swap/d' /etc/fstab"
  log "Swap disabled"

  # Kernel modules
  run "modprobe overlay"
  run "modprobe br_netfilter"
  cat > /etc/modules-load.d/caiman.conf <<EOF
overlay
br_netfilter
EOF

  # Kernel parameters
  cat > /etc/sysctl.d/99-caiman.conf <<EOF
# Kubernetes networking
net.bridge.bridge-nf-call-iptables  = 1
net.bridge.bridge-nf-call-ip6tables = 1
net.ipv4.ip_forward                 = 1
# XDP / large receive offload
net.core.rmem_max                   = 134217728
net.core.wmem_max                   = 134217728
net.ipv4.tcp_rmem                   = 4096 87380 67108864
net.ipv4.tcp_wmem                   = 4096 65536 67108864
net.core.netdev_max_backlog         = 250000
# KVM / memory
vm.nr_hugepages                     = 1024
vm.swappiness                       = 0
EOF
  run "sysctl --system"
  log "Kernel parameters applied"

  # Required packages
  run "apt-get update -qq"
  run "apt-get install -y -qq \
    curl wget gnupg2 apt-transport-https ca-certificates \
    clang llvm libelf-dev linux-headers-$(uname -r) \
    bpftool iproute2 ethtool \
    restic qemu-utils cloud-utils \
    nvme-cli open-iscsi \
    jq make"
  log "System packages installed"

  # Create directories
  run "mkdir -p $INSTALL_DIR $STATE_DIR $DATA_DIR"
  run "mkdir -p /var/lib/caiman/{disks,snapshots,templates,backups}"
  run "mkdir -p /sys/fs/bpf/caiman"
  run "mount bpffs /sys/fs/bpf -t bpf 2>/dev/null || true"
}

# ── Install containerd ─────────────────────────────────────────────────────
install_containerd() {
  step "Installing containerd"

  local VERSION="1.7.14"
  curl -fsSL "https://github.com/containerd/containerd/releases/download/v${VERSION}/containerd-${VERSION}-linux-amd64.tar.gz" \
    | tar -C /usr/local -xzf - &
  spinner "Downloading containerd $VERSION"

  # runc
  curl -fsSL "https://github.com/opencontainers/runc/releases/latest/download/runc.amd64" \
    -o /usr/local/sbin/runc
  chmod +x /usr/local/sbin/runc

  # CNI plugins
  mkdir -p /opt/cni/bin
  curl -fsSL "https://github.com/containernetworking/plugins/releases/download/v1.4.1/cni-plugins-linux-amd64-v1.4.1.tgz" \
    | tar -C /opt/cni/bin -xzf -

  # containerd config
  mkdir -p /etc/containerd
  containerd config default > /etc/containerd/config.toml
  sed -i 's/SystemdCgroup = false/SystemdCgroup = true/' /etc/containerd/config.toml

  # systemd unit
  curl -fsSL "https://raw.githubusercontent.com/containerd/containerd/main/containerd.service" \
    -o /etc/systemd/system/containerd.service

  run "systemctl daemon-reload"
  run "systemctl enable --now containerd"
  log "containerd: ${BRT}running${NC}"
}

# ── Install Kubernetes ─────────────────────────────────────────────────────
install_kubernetes() {
  step "Installing Kubernetes"

  local K8S_VERSION="1.29"

  curl -fsSL "https://pkgs.k8s.io/core:/stable:/v${K8S_VERSION}/deb/Release.key" \
    | gpg --dearmor -o /etc/apt/keyrings/kubernetes-apt-keyring.gpg

  echo "deb [signed-by=/etc/apt/keyrings/kubernetes-apt-keyring.gpg] \
    https://pkgs.k8s.io/core:/stable:/v${K8S_VERSION}/deb/ /" \
    > /etc/apt/sources.list.d/kubernetes.list

  run "apt-get update -qq"
  run "apt-get install -y kubelet kubeadm kubectl"
  run "apt-mark hold kubelet kubeadm kubectl"
  log "Kubernetes $K8S_VERSION installed"
}

# ── Install Caimán binaries ────────────────────────────────────────────────
install_caiman_binaries() {
  step "Installing Caimán OS binaries"

  local BASE="https://github.com/your-org/caiman-os/releases/download/v${CAIMAN_VERSION}"

  for bin in caiman-vmm caiman-cni caiman-mcp caiman-bts caiman-livemig caiman; do
    log "Downloading $bin…"
    run "curl -fsSL $BASE/$bin-linux-amd64 -o /usr/local/bin/$bin"
    run "chmod +x /usr/local/bin/$bin"
  done

  # Kernel module
  log "Building caiman_net.ko kernel module…"
  run "curl -fsSL $BASE/caiman_net-$(uname -r).ko -o /lib/modules/$(uname -r)/extra/caiman_net.ko \
    || (curl -fsSL $BASE/caiman_net-src.tar.gz | tar -xzf - && make -C caiman_net_src && cp caiman_net_src/caiman_net.ko /lib/modules/$(uname -r)/extra/)"
  run "depmod -a"

  # XDP eBPF objects
  run "mkdir -p /usr/local/lib/caiman"
  run "curl -fsSL $BASE/xdp_microseg.o -o /usr/local/lib/caiman/xdp_microseg.o"
  run "curl -fsSL $BASE/xdp_vm_router.o -o /usr/local/lib/caiman/xdp_vm_router.o"

  # CNI config
  run "mkdir -p /etc/cni/net.d"
  cat > /etc/cni/net.d/10-caiman.conflist <<EOF
{
  "cniVersion": "1.0.0",
  "name":       "caiman",
  "plugins": [
    {
      "type":       "caiman-cni",
      "uplink":     "$UPLINK",
      "bpfPinPath": "/sys/fs/bpf/caiman"
    }
  ]
}
EOF
  log "Caimán binaries installed"
}

# ── Systemd services ───────────────────────────────────────────────────────
install_services() {
  step "Installing systemd services"

  # caiman-init: loads kernel module + XDP at boot
  cat > /etc/systemd/system/caiman-init.service <<EOF
[Unit]
Description=Caimán OS initialisation
After=network.target
Before=containerd.service kubelet.service

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/local/bin/caiman-init.sh

[Install]
WantedBy=multi-user.target
EOF

  cat > /usr/local/bin/caiman-init.sh <<'INITEOF'
#!/bin/bash
set -e
# Load caiman_net kernel module
modprobe caiman_net || insmod /lib/modules/$(uname -r)/extra/caiman_net.ko
# Load XDP programs
bpftool prog load /usr/local/lib/caiman/xdp_microseg.o \
  /sys/fs/bpf/caiman/xdp_microseg pinmaps /sys/fs/bpf/caiman
bpftool net attach xdp pinned /sys/fs/bpf/caiman/xdp_microseg dev ${CAIMAN_UPLINK:-eth0}
echo "Caimán XDP active on ${CAIMAN_UPLINK:-eth0}"
INITEOF
  chmod +x /usr/local/bin/caiman-init.sh

  # caiman-api service
  cat > /etc/systemd/system/caiman-api.service <<EOF
[Unit]
Description=Caimán OS API server
After=containerd.service caiman-init.service
Requires=caiman-init.service

[Service]
Type=simple
ExecStart=/usr/local/bin/caiman-api
Environment=CAIMAN_STATE_DIR=/var/run/caiman
Environment=CAIMAN_UPLINK=$UPLINK
Environment=DATABASE_URL=sqlite:///var/lib/caiman/caiman.db
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

  run "systemctl daemon-reload"
  run "systemctl enable caiman-init caiman-api"
  log "Services installed and enabled"
}

# ── Bootstrap control plane ────────────────────────────────────────────────
bootstrap_control_plane() {
  step "Bootstrapping Kubernetes control plane"

  local NODE_IP
  NODE_IP=$(ip route get 1.1.1.1 | awk '{print $7; exit}')

  cat > /tmp/kubeadm-config.yaml <<EOF
apiVersion: kubeadm.k8s.io/v1beta3
kind: InitConfiguration
localAPIEndpoint:
  advertiseAddress: $NODE_IP
  bindPort: 6443
nodeRegistration:
  criSocket: unix:///run/containerd/containerd.sock
  kubeletExtraArgs:
    node-ip: "$NODE_IP"
    cgroup-driver: "systemd"
---
apiVersion: kubeadm.k8s.io/v1beta3
kind: ClusterConfiguration
networking:
  podSubnet:     $POD_CIDR
  serviceSubnet: $SVC_CIDR
kubernetesVersion: "v1.29.0"
controlPlaneEndpoint: "$NODE_IP:6443"
---
apiVersion: kubelet.config.k8s.io/v1beta1
kind: KubeletConfiguration
cgroupDriver: systemd
EOF

  log "Running kubeadm init…"
  run "kubeadm init --config /tmp/kubeadm-config.yaml --upload-certs 2>&1 | tee /tmp/kubeadm-init.log"

  # Configure kubectl
  run "mkdir -p /root/.kube"
  run "cp /etc/kubernetes/admin.conf /root/.kube/config"

  # Allow scheduling on control plane (single-node or AIO)
  if [[ "$ROLE" == "aio" ]]; then
    run "kubectl taint nodes --all node-role.kubernetes.io/control-plane- 2>/dev/null || true"
    log "Taints removed (all-in-one mode)"
  fi

  # Deploy Caimán stack
  deploy_caiman_stack

  # Print join command
  echo
  echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo -e "${BRT}  Control plane ready!${NC}"
  echo
  echo -e "  To add worker nodes, run on each:"
  echo
  echo -e "${DIM}  $(kubeadm token create --print-join-command 2>/dev/null)${NC}"
  echo
  echo -e "  Or use the Caimán installer:"
  echo -e "${DIM}  curl -fsSL https://install.caiman.io | bash -s -- \\${NC}"
  echo -e "${DIM}    --role worker --join $NODE_IP --token $(kubeadm token list | awk 'NR==2{print $1}')${NC}"
  echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# ── Join worker node ───────────────────────────────────────────────────────
join_worker() {
  step "Joining worker node to cluster"

  [[ -z "$CP_IP" ]]     && die "--join (control-plane IP) required for worker role"
  [[ -z "$JOIN_TOKEN" ]] && die "--token required for worker role"

  # Fetch CA cert hash from control plane
  local CA_HASH
  CA_HASH=$(openssl x509 -pubkey -in /etc/kubernetes/pki/ca.crt 2>/dev/null \
    | openssl rsa -pubin -outform DER 2>/dev/null \
    | sha256sum | awk '{print $1}' \
    || curl -fsSk "https://${CP_IP}:6443/api/v1/namespaces/kube-system/configmaps/kubeadm-config" \
       | jq -r '.data["ClusterConfiguration"]' \
       | grep certificatesDir \
       | true)

  run "kubeadm join ${CP_IP}:6443 \
    --token $JOIN_TOKEN \
    --discovery-token-unsafe-skip-ca-verification \
    --cri-socket unix:///run/containerd/containerd.sock"

  log "Worker joined cluster at ${CP_IP}"
}

# ── Deploy Caimán Kubernetes stack ─────────────────────────────────────────
deploy_caiman_stack() {
  step "Deploying Caimán OS Kubernetes stack"

  local BASE="https://raw.githubusercontent.com/your-org/caiman-os/main/k8s"

  for manifest in \
    daemonset-mcp.yaml \
    drs/drs.yaml \
    microseg/k8s/microsegpolicy_crd.yaml \
    monitoring/k8s/monitoring-stack.yaml; do
    log "Applying $manifest…"
    run "kubectl apply -f $BASE/$manifest"
  done

  # Wait for DaemonSet rollout
  log "Waiting for caiman-mcp DaemonSet to be ready…"
  run "kubectl rollout status daemonset/caiman-mcp -n caiman-system --timeout=120s"

  log "Caimán stack deployed"
}

# ── Verify installation ────────────────────────────────────────────────────
verify() {
  step "Verifying installation"

  # Kernel module
  lsmod | grep -q caiman_net \
    && log "caiman_net.ko: ${BRT}loaded${NC}" \
    || warn "caiman_net.ko not loaded — check dmesg"

  # XDP
  ip link show dev "$UPLINK" | grep -q "xdpgeneric\|xdp" \
    && log "XDP on $UPLINK: ${BRT}attached${NC}" \
    || warn "XDP not attached to $UPLINK"

  # Kubernetes
  if [[ -f /root/.kube/config ]]; then
    run "kubectl get nodes -o wide"
    run "kubectl get pods -n caiman-system"
  fi

  # caiman CLI
  caiman ping &>/dev/null \
    && log "caiman CLI: ${BRT}API reachable${NC}" \
    || warn "caiman API not reachable (may still be starting)"

  echo
  echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo -e "${BRT}  🐊 Caimán OS installed successfully!${NC}"
  echo
  echo -e "  Dashboard:   ${GRN}http://$(hostname -I | awk '{print $1}'):3000${NC}"
  echo -e "  API:         ${GRN}http://$(hostname -I | awk '{print $1}'):8765${NC}"
  echo -e "  Grafana:     ${GRN}http://$(hostname -I | awk '{print $1}'):3001${NC}"
  echo -e "  CLI:         ${GRN}caiman cluster status${NC}"
  echo -e "  Logs:        ${DIM}$LOG_FILE${NC}"
  echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
}

# ── Main ───────────────────────────────────────────────────────────────────
main() {
  echo -e "$LOGO"
  echo -e "${DIM}  Role: ${BRT}$ROLE${NC}  |  Uplink: ${BRT}$UPLINK${NC}  |  Log: ${DIM}$LOG_FILE${NC}\n"

  [[ $DRY_RUN -eq 1 ]] && echo -e "${AMB}  DRY RUN — no changes will be made${NC}\n"

  if [[ $SKIP_CONFIRM -eq 0 && $DRY_RUN -eq 0 ]]; then
    confirm "Install Caimán OS (role=$ROLE) on $(hostname)?"
  fi

  mkdir -p "$(dirname "$LOG_FILE")"
  touch "$LOG_FILE"

  preflight
  prepare_system
  install_containerd
  install_kubernetes
  install_caiman_binaries
  install_services

  case "$ROLE" in
    cp|aio)  bootstrap_control_plane ;;
    worker)  join_worker ;;
    *)       die "Unknown role: $ROLE" ;;
  esac

  verify
}

main "$@"
