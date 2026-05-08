#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Immich Installer
#  Self-hosted photo and video backup
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/productivity/immich.sh)
#         bash <(curl -fsSL ...) --new-vm --name immich-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Immich"
APP_PORT="2283"
APP_CPUS=4
APP_MEM=4096
APP_DESC="Self-hosted photo and video backup"

show_help() {
  echo "Usage: bash immich.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: immich-01)"
  echo "  --cpus N          vCPUs (default: 4)"
  echo "  --mem MiB         RAM (default: 4096)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_immich() {
  detect_os
  pkg_update
  pkg_install docker.io 2>/dev/null || pkg_install docker
  mkdir -p /opt/immich && cd /opt/immich
  curl -fsSL https://github.com/immich-app/immich/releases/latest/download/docker-compose.yml -o docker-compose.yml
  curl -fsSL https://github.com/immich-app/immich/releases/latest/download/example.env -o .env
  docker compose up -d
  msg "Immich installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-immich-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/productivity/immich.sh) --existing"
  else
    check_root
    install_immich
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
