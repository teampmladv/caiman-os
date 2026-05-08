#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Postgresql Installer
#  Production SQL database
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/db/postgresql.sh)
#         bash <(curl -fsSL ...) --new-vm --name postgresql-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Postgresql"
APP_PORT="5432"
APP_CPUS=2
APP_MEM=1024
APP_DESC="Production SQL database"

show_help() {
  echo "Usage: bash postgresql.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: postgresql-01)"
  echo "  --cpus N          vCPUs (default: 2)"
  echo "  --mem MiB         RAM (default: 1024)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_postgresql() {
  detect_os
  pkg_update
  pkg_install postgresql postgresql-contrib
  service_enable postgresql
  msg "PostgreSQL installed"
  info "Create DB: sudo -u postgres createdb mydb"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-postgresql-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/db/postgresql.sh) --existing"
  else
    check_root
    install_postgresql
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
