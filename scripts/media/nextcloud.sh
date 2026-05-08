#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Nextcloud Installer
#  Self-hosted file collaboration
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/media/nextcloud.sh)
#         bash <(curl -fsSL ...) --new-vm --name nextcloud-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Nextcloud"
APP_PORT="80"
APP_CPUS=4
APP_MEM=4096
APP_DESC="Self-hosted file collaboration"

show_help() {
  echo "Usage: bash nextcloud.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: nextcloud-01)"
  echo "  --cpus N          vCPUs (default: 4)"
  echo "  --mem MiB         RAM (default: 4096)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_nextcloud() {
  detect_os
  pkg_update
  pkg_install apache2 libapache2-mod-php php php-gd php-mysql php-curl php-mbstring php-xml php-zip mariadb-server
  curl -fsSL https://download.nextcloud.com/server/installer/setup-nextcloud.php -o /var/www/html/setup-nextcloud.php
  chown www-data:www-data /var/www/html/setup-nextcloud.php
  service_enable apache2 && service_enable mariadb
  msg "Nextcloud installer ready — visit http://$(get_ip)/setup-nextcloud.php"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-nextcloud-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/media/nextcloud.sh) --existing"
  else
    check_root
    install_nextcloud
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
