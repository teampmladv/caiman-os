#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Grafana Installer
#  Metrics dashboards
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/monitoring/grafana.sh)
#         bash <(curl -fsSL ...) --new-vm --name grafana-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Grafana"
APP_PORT="3000"
APP_CPUS=2
APP_MEM=1024
APP_DESC="Metrics dashboards"

show_help() {
  echo "Usage: bash grafana.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: grafana-01)"
  echo "  --cpus N          vCPUs (default: 2)"
  echo "  --mem MiB         RAM (default: 1024)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_grafana() {
  detect_os
  pkg_update
  pkg_install apt-transport-https software-properties-common
  curl -fsSL https://apt.grafana.com/gpg.key | gpg --dearmor -o /usr/share/keyrings/grafana.gpg
  echo "deb [signed-by=/usr/share/keyrings/grafana.gpg] https://apt.grafana.com stable main" | tee /etc/apt/sources.list.d/grafana.list
  pkg_update && pkg_install grafana
  service_enable grafana-server
  msg "Grafana installed — login: admin/admin"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-grafana-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/monitoring/grafana.sh) --existing"
  else
    check_root
    install_grafana
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
