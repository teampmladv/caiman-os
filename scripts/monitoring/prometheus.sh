#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Prometheus Installer
#  Metrics collection and alerting
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/monitoring/prometheus.sh)
#         bash <(curl -fsSL ...) --new-vm --name prometheus-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Prometheus"
APP_PORT="9090"
APP_CPUS=2
APP_MEM=1024
APP_DESC="Metrics collection and alerting"

show_help() {
  echo "Usage: bash prometheus.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: prometheus-01)"
  echo "  --cpus N          vCPUs (default: 2)"
  echo "  --mem MiB         RAM (default: 1024)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_prometheus() {
  detect_os
  pkg_update
  local VER="2.52.0"
  useradd --no-create-home --shell /bin/false prometheus 2>/dev/null || true
  mkdir -p /etc/prometheus /var/lib/prometheus
  curl -fsSL "https://github.com/prometheus/prometheus/releases/download/v${VER}/prometheus-${VER}.linux-amd64.tar.gz" | tar xz -C /tmp
  cp "/tmp/prometheus-${VER}.linux-amd64/prometheus" /usr/local/bin/
  chown -R prometheus:prometheus /etc/prometheus /var/lib/prometheus
  cat > /etc/systemd/system/prometheus.service << 'SVC'
[Unit]
Description=Prometheus
After=network.target
[Service]
User=prometheus
ExecStart=/usr/local/bin/prometheus --config.file=/etc/prometheus/prometheus.yml --storage.tsdb.path=/var/lib/prometheus/
[Install]
WantedBy=multi-user.target
SVC
  systemctl daemon-reload && service_enable prometheus
  msg "Prometheus installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-prometheus-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/monitoring/prometheus.sh) --existing"
  else
    check_root
    install_prometheus
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
