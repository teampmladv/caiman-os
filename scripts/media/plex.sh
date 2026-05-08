#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Plex Installer
#  Plex media server
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/media/plex.sh)
#         bash <(curl -fsSL ...) --new-vm --name plex-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Plex"
APP_PORT="32400"
APP_CPUS=4
APP_MEM=4096
APP_DESC="Plex media server"

show_help() {
  echo "Usage: bash plex.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: plex-01)"
  echo "  --cpus N          vCPUs (default: 4)"
  echo "  --mem MiB         RAM (default: 4096)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_plex() {
  detect_os
  pkg_update
  curl -fsSL https://downloads.plex.tv/plex-keys/PlexSign.key | gpg --dearmor -o /usr/share/keyrings/plex.gpg
  echo "deb [signed-by=/usr/share/keyrings/plex.gpg] https://downloads.plex.tv/repo/deb public main" | tee /etc/apt/sources.list.d/plexmediaserver.list
  pkg_update && pkg_install plexmediaserver
  service_enable plexmediaserver
  msg "Plex installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-plex-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/media/plex.sh) --existing"
  else
    check_root
    install_plex
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
