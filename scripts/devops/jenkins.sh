#!/usr/bin/env bash
# ============================================================
#  Caimán OS — Jenkins Installer
#  CI/CD automation server
#  Usage: bash <(curl -fsSL https://scripts.caimanos.com/devops/jenkins.sh)
#         bash <(curl -fsSL ...) --new-vm --name jenkins-01
# ============================================================
set -euo pipefail

source <(curl -fsSL https://scripts.caimanos.com/lib/caiman.func 2>/dev/null) || {
  echo "Cannot load caiman.func from https://scripts.caimanos.com/lib/caiman.func" >&2
  exit 1
}

APP_NAME="Jenkins"
APP_PORT="8080"
APP_CPUS=4
APP_MEM=2048
APP_DESC="CI/CD automation server"

show_help() {
  echo "Usage: bash jenkins.sh [OPTIONS]"
  echo ""
  echo "  --new-vm          Create Caiman VM + install inside"
  echo "  --existing        Install in current system (default)"
  echo "  --name NAME       VM name (default: jenkins-01)"
  echo "  --cpus N          vCPUs (default: 4)"
  echo "  --mem MiB         RAM (default: 2048)"
  echo "  --api URL         Caiman API URL"
  echo "  -h, --help        Show help"
}

install_jenkins() {
  detect_os
  pkg_update
  curl -fsSL https://pkg.jenkins.io/debian-stable/jenkins.io-2023.key | gpg --dearmor -o /usr/share/keyrings/jenkins-keyring.gpg
  echo "deb [signed-by=/usr/share/keyrings/jenkins-keyring.gpg] https://pkg.jenkins.io/debian-stable binary/" | tee /etc/apt/sources.list.d/jenkins.list
  pkg_update && pkg_install jenkins
  service_enable jenkins
  msg "Jenkins installed — admin password in /var/lib/jenkins/secrets/initialAdminPassword"
}

main() {
  parse_mode "$@"
  VM_NAME="${VM_NAME:-jenkins-01}"
  header "$APP_NAME — $APP_DESC"

  if [[ "$MODE" == "new-vm" ]]; then
    check_api
    local vm_id
    vm_id=$(create_vm "$VM_NAME" "$APP_CPUS" "$APP_MEM")
    wait_vm_running "$vm_id"
    info "VM ready: $vm_id"
    info "Connect via console, then run:"
    info "  bash <(curl -fsSL https://scripts.caimanos.com/devops/jenkins.sh) --existing"
  else
    check_root
    install_jenkins
    [[ -n "$APP_PORT" ]] && open_port "$APP_PORT"
    success "$APP_NAME" "$APP_PORT"
  fi
}

main "$@"
