#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Nginx Installer
#  High-performance web server
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/web/nginx.sh)
#         bash <(curl -fsSL ...) --new-vm --name nginx-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Nginx"
APP_PORT="80"
APP_CPUS=1
APP_MEM=512
APP_DESC="High-performance web server"

show_help() {
  echo "Usage: bash nginx.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: nginx-01)"
  echo "  --cpus N          vCPUs (default: 1)"
  echo "  --mem MiB         RAM (default: 512)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_nginx() {
  detect_os
  pkg_update
  pkg_install nginx
  service_enable nginx
  msg "Nginx installed"
  info "Config: /etc/nginx/sites-available/"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-nginx-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/web/nginx.sh) --existing"
  else
    check_root
    install_nginx
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
