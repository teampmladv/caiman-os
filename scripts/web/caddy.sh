#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Caddy Installer
#  Automatic HTTPS web server
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/web/caddy.sh)
#         bash <(curl -fsSL ...) --new-vm --name caddy-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Caddy"
APP_PORT="80"
APP_CPUS=1
APP_MEM=512
APP_DESC="Automatic HTTPS web server"

show_help() {
  echo "Usage: bash caddy.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: caddy-01)"
  echo "  --cpus N          vCPUs (default: 1)"
  echo "  --mem MiB         RAM (default: 512)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_caddy() {
  detect_os
  pkg_update
  curl -fsSL https://dl.cloudsmith.io/public/caddy/stable/gpg.key | gpg --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg
  echo "deb [signed-by=/usr/share/keyrings/caddy-stable-archive-keyring.gpg] https://dl.cloudsmith.io/public/caddy/stable/deb/debian any-version main" | tee /etc/apt/sources.list.d/caddy-stable.list
  pkg_update && pkg_install caddy
  service_enable caddy
  msg "Caddy installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-caddy-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/web/caddy.sh) --existing"
  else
    check_root
    install_caddy
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
