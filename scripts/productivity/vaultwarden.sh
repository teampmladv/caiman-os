#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Vaultwarden Installer
#  Bitwarden-compatible password manager
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/productivity/vaultwarden.sh)
#         bash <(curl -fsSL ...) --new-vm --name vaultwarden-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Vaultwarden"
APP_PORT="8080"
APP_CPUS=1
APP_MEM=512
APP_DESC="Bitwarden-compatible password manager"

show_help() {
  echo "Usage: bash vaultwarden.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: vaultwarden-01)"
  echo "  --cpus N          vCPUs (default: 1)"
  echo "  --mem MiB         RAM (default: 512)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_vaultwarden() {
  detect_os
  pkg_update
  pkg_install docker.io 2>/dev/null || pkg_install docker
  service_enable docker
  mkdir -p /opt/vaultwarden/data
  docker run -d --name vaultwarden -v /opt/vaultwarden/data:/data -p 8080:80 --restart unless-stopped vaultwarden/server:latest
  msg "Vaultwarden running"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-vaultwarden-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/productivity/vaultwarden.sh) --existing"
  else
    check_root
    install_vaultwarden
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
