#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Harbor Installer
#  Enterprise container registry
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/devops/harbor.sh)
#         bash <(curl -fsSL ...) --new-vm --name harbor-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Harbor"
APP_PORT="80"
APP_CPUS=4
APP_MEM=4096
APP_DESC="Enterprise container registry"

show_help() {
  echo "Usage: bash harbor.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: harbor-01)"
  echo "  --cpus N          vCPUs (default: 4)"
  echo "  --mem MiB         RAM (default: 4096)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_harbor() {
  detect_os
  pkg_update
  local VER="v2.10.0"
  curl -fsSL "https://github.com/goharbor/harbor/releases/download/${VER}/harbor-online-installer-${VER}.tgz" | tar xz -C /opt
  cd /opt/harbor && cp harbor.yml.tmpl harbor.yml
  sed -i "s/reg.mydomain.com/$(get_ip)/" harbor.yml
  ./install.sh
  msg "Harbor installed — login: admin/Harbor12345"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-harbor-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/devops/harbor.sh) --existing"
  else
    check_root
    install_harbor
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
