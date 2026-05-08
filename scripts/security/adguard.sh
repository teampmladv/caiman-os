#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Adguard Installer
#  Network-wide ad and tracker blocker
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/security/adguard.sh)
#         bash <(curl -fsSL ...) --new-vm --name adguard-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Adguard"
APP_PORT="3000"
APP_CPUS=1
APP_MEM=512
APP_DESC="Network-wide ad and tracker blocker"

show_help() {
  echo "Usage: bash adguard.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: adguard-01)"
  echo "  --cpus N          vCPUs (default: 1)"
  echo "  --mem MiB         RAM (default: 512)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_adguard() {
  detect_os
  pkg_update
  curl -fsSL https://static.adguard.com/adguardhome/release/AdGuardHome_linux_amd64.tar.gz | tar xz -C /tmp
  mv /tmp/AdGuardHome/AdGuardHome /usr/local/bin/
  /usr/local/bin/AdGuardHome --service install
  msg "AdGuard Home installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-adguard-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/security/adguard.sh) --existing"
  else
    check_root
    install_adguard
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
