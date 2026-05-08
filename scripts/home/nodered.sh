#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Nodered Installer
#  Low-code event-driven automation
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/home/nodered.sh)
#         bash <(curl -fsSL ...) --new-vm --name nodered-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Nodered"
APP_PORT="1880"
APP_CPUS=1
APP_MEM=512
APP_DESC="Low-code event-driven automation"

show_help() {
  echo "Usage: bash nodered.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: nodered-01)"
  echo "  --cpus N          vCPUs (default: 1)"
  echo "  --mem MiB         RAM (default: 512)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_nodered() {
  detect_os
  pkg_update
  curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
  pkg_install nodejs
  npm install -g --unsafe-perm node-red
  cat > /etc/systemd/system/nodered.service << 'SVC'
[Unit]
Description=Node-RED
After=network.target
[Service]
ExecStart=/usr/local/bin/node-red
[Install]
WantedBy=multi-user.target
SVC
  systemctl daemon-reload && service_enable nodered
  msg "Node-RED installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-nodered-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/home/nodered.sh) --existing"
  else
    check_root
    install_nodered
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
