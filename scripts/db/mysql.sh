#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Mysql Installer
#  MySQL relational database
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/db/mysql.sh)
#         bash <(curl -fsSL ...) --new-vm --name mysql-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Mysql"
APP_PORT="3306"
APP_CPUS=2
APP_MEM=1024
APP_DESC="MySQL relational database"

show_help() {
  echo "Usage: bash mysql.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: mysql-01)"
  echo "  --cpus N          vCPUs (default: 2)"
  echo "  --mem MiB         RAM (default: 1024)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_mysql() {
  detect_os
  pkg_update
  pkg_install mysql-server
  service_enable mysql
  msg "MySQL installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-mysql-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/db/mysql.sh) --existing"
  else
    check_root
    install_mysql
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
