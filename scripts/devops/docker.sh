#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Docker Installer
#  Docker container runtime
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/devops/docker.sh)
#         bash <(curl -fsSL ...) --new-vm --name docker-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Docker"
APP_PORT=""
APP_CPUS=2
APP_MEM=1024
APP_DESC="Docker container runtime"

show_help() {
  echo "Usage: bash docker.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: docker-01)"
  echo "  --cpus N          vCPUs (default: 2)"
  echo "  --mem MiB         RAM (default: 1024)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_docker() {
  detect_os
  pkg_update
  curl -fsSL https://get.docker.com | sh
  service_enable docker
  msg "Docker installed: $(docker --version)"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-docker-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/devops/docker.sh) --existing"
  else
    check_root
    install_docker
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
