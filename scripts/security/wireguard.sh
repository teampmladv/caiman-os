#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Wireguard Installer
#  Fast modern VPN
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/security/wireguard.sh)
#         bash <(curl -fsSL ...) --new-vm --name wireguard-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Wireguard"
APP_PORT="51820"
APP_CPUS=1
APP_MEM=256
APP_DESC="Fast modern VPN"

show_help() {
  echo "Usage: bash wireguard.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: wireguard-01)"
  echo "  --cpus N          vCPUs (default: 1)"
  echo "  --mem MiB         RAM (default: 256)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_wireguard() {
  detect_os
  pkg_update
  pkg_install wireguard
  wg genkey | tee /etc/wireguard/private.key | wg pubkey > /etc/wireguard/public.key
  chmod 600 /etc/wireguard/private.key
  echo "net.ipv4.ip_forward=1" >> /etc/sysctl.conf && sysctl -p
  service_enable "wg-quick@wg0" || true
  msg "WireGuard installed — public key: $(cat /etc/wireguard/public.key)"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-wireguard-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/security/wireguard.sh) --existing"
  else
    check_root
    install_wireguard
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
