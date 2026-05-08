#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Mongodb Installer
#  NoSQL document database
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/db/mongodb.sh)
#         bash <(curl -fsSL ...) --new-vm --name mongodb-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Mongodb"
APP_PORT="27017"
APP_CPUS=2
APP_MEM=2048
APP_DESC="NoSQL document database"

show_help() {
  echo "Usage: bash mongodb.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: mongodb-01)"
  echo "  --cpus N          vCPUs (default: 2)"
  echo "  --mem MiB         RAM (default: 2048)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_mongodb() {
  detect_os
  pkg_update
  curl -fsSL https://www.mongodb.org/static/pgp/server-7.0.asc | gpg -o /usr/share/keyrings/mongodb-server-7.0.gpg --dearmor
  echo "deb [ arch=amd64 signed-by=/usr/share/keyrings/mongodb-server-7.0.gpg ] https://repo.mongodb.org/apt/ubuntu jammy/mongodb-org/7.0 multiverse" | tee /etc/apt/sources.list.d/mongodb-org-7.0.list
  pkg_update && pkg_install mongodb-org
  service_enable mongod
  msg "MongoDB installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-mongodb-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/db/mongodb.sh) --existing"
  else
    check_root
    install_mongodb
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
