#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Redis Installer
#  In-memory data store and cache
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/db/redis.sh)
#         bash <(curl -fsSL ...) --new-vm --name redis-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Redis"
APP_PORT="6379"
APP_CPUS=1
APP_MEM=512
APP_DESC="In-memory data store and cache"

show_help() {
  echo "Usage: bash redis.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: redis-01)"
  echo "  --cpus N          vCPUs (default: 1)"
  echo "  --mem MiB         RAM (default: 512)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_redis() {
  detect_os
  pkg_update
  pkg_install redis-server
  sed -i "s/bind 127.0.0.1/bind 0.0.0.0/" /etc/redis/redis.conf 2>/dev/null || true
  service_enable redis-server
  msg "Redis installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-redis-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/db/redis.sh) --existing"
  else
    check_root
    install_redis
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
