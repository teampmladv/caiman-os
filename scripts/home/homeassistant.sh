#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Homeassistant Installer
#  Open source home automation
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/home/homeassistant.sh)
#         bash <(curl -fsSL ...) --new-vm --name homeassistant-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Homeassistant"
APP_PORT="8123"
APP_CPUS=2
APP_MEM=2048
APP_DESC="Open source home automation"

show_help() {
  echo "Usage: bash homeassistant.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: homeassistant-01)"
  echo "  --cpus N          vCPUs (default: 2)"
  echo "  --mem MiB         RAM (default: 2048)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_homeassistant() {
  detect_os
  pkg_update
  pkg_install python3 python3-pip python3-venv
  useradd -rm homeassistant 2>/dev/null || true
  mkdir -p /srv/homeassistant && chown homeassistant:homeassistant /srv/homeassistant
  sudo -u homeassistant python3 -m venv /srv/homeassistant
  sudo -u homeassistant /srv/homeassistant/bin/pip install homeassistant
  cat > /etc/systemd/system/homeassistant.service << 'SVC'
[Unit]
Description=Home Assistant
After=network-online.target
[Service]
User=homeassistant
ExecStart=/srv/homeassistant/bin/hass -c /home/homeassistant/.homeassistant
[Install]
WantedBy=multi-user.target
SVC
  systemctl daemon-reload && service_enable homeassistant
  msg "Home Assistant installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-homeassistant-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/home/homeassistant.sh) --existing"
  else
    check_root
    install_homeassistant
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
