#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Uptime-Kuma Installer
#  Self-hosted uptime monitoring
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/monitoring/uptime-kuma.sh)
#         bash <(curl -fsSL ...) --new-vm --name uptime-kuma-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Uptime-Kuma"
APP_PORT="3001"
APP_CPUS=1
APP_MEM=512
APP_DESC="Self-hosted uptime monitoring"

show_help() {
  echo "Usage: bash uptime-kuma.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: uptime-kuma-01)"
  echo "  --cpus N          vCPUs (default: 1)"
  echo "  --mem MiB         RAM (default: 512)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_uptime_kuma() {
  detect_os
  pkg_update
  curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
  pkg_install nodejs
  npm install -g pm2
  git clone https://github.com/louislam/uptime-kuma.git /opt/uptime-kuma
  cd /opt/uptime-kuma && npm run setup
  pm2 start server/server.js --name uptime-kuma && pm2 save
  msg "Uptime Kuma installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-uptime-kuma-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/monitoring/uptime-kuma.sh) --existing"
  else
    check_root
    install_uptime_kuma
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
