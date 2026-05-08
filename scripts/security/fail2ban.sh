#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Fail2Ban Installer
#  Intrusion prevention system
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/security/fail2ban.sh)
#         bash <(curl -fsSL ...) --new-vm --name fail2ban-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Fail2Ban"
APP_PORT=""
APP_CPUS=1
APP_MEM=256
APP_DESC="Intrusion prevention system"

show_help() {
  echo "Usage: bash fail2ban.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: fail2ban-01)"
  echo "  --cpus N          vCPUs (default: 1)"
  echo "  --mem MiB         RAM (default: 256)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_fail2ban() {
  detect_os
  pkg_update
  pkg_install fail2ban
  cp /etc/fail2ban/jail.conf /etc/fail2ban/jail.local
  service_enable fail2ban
  msg "Fail2ban installed — SSH protection active"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-fail2ban-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/security/fail2ban.sh) --existing"
  else
    check_root
    install_fail2ban
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
