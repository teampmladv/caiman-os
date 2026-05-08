#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Jellyfin Installer
#  Free media server and streaming
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/media/jellyfin.sh)
#         bash <(curl -fsSL ...) --new-vm --name jellyfin-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Jellyfin"
APP_PORT="8096"
APP_CPUS=4
APP_MEM=4096
APP_DESC="Free media server and streaming"

show_help() {
  echo "Usage: bash jellyfin.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: jellyfin-01)"
  echo "  --cpus N          vCPUs (default: 4)"
  echo "  --mem MiB         RAM (default: 4096)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_jellyfin() {
  detect_os
  pkg_update
  curl -fsSL https://repo.jellyfin.org/install-debuntu.sh | bash
  service_enable jellyfin
  msg "Jellyfin installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-jellyfin-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/media/jellyfin.sh) --existing"
  else
    check_root
    install_jellyfin
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
