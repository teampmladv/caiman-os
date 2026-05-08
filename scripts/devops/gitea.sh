#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Gitea Installer
#  Lightweight self-hosted Git service
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/devops/gitea.sh)
#         bash <(curl -fsSL ...) --new-vm --name gitea-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Gitea"
APP_PORT="3000"
APP_CPUS=2
APP_MEM=1024
APP_DESC="Lightweight self-hosted Git service"

show_help() {
  echo "Usage: bash gitea.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: gitea-01)"
  echo "  --cpus N          vCPUs (default: 2)"
  echo "  --mem MiB         RAM (default: 1024)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_gitea() {
  detect_os
  pkg_update
  local VER="1.21.0"
  adduser --system --shell /bin/bash --group --disabled-password --home /home/git git 2>/dev/null || true
  mkdir -p /var/lib/gitea/{custom,data,log} /etc/gitea
  curl -fsSL "https://dl.gitea.io/gitea/${VER}/gitea-${VER}-linux-amd64" -o /usr/local/bin/gitea && chmod +x /usr/local/bin/gitea
  cat > /etc/systemd/system/gitea.service << 'SVC'
[Unit]
Description=Gitea
After=network.target
[Service]
User=git
WorkingDirectory=/var/lib/gitea
ExecStart=/usr/local/bin/gitea web --config /etc/gitea/app.ini
[Install]
WantedBy=multi-user.target
SVC
  systemctl daemon-reload && service_enable gitea
  msg "Gitea installed"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-gitea-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/devops/gitea.sh) --existing"
  else
    check_root
    install_gitea
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
