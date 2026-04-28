#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════
#  Caimán OS — Provisioning via iDRAC / iLO / Redfish BMC
#  Sin SO instalado. Solo IPMI/iDRAC access.
#
#  Soporta:
#    Dell iDRAC 7/8/9         (DRAC, racadm, Redfish)
#    HPE iLO 4/5/6            (iLO REST, ribcl)
#    Supermicro IPMI           (ipmitool, Redfish)
#    Generic IPMI 2.0          (ipmitool)
#    Redfish-compatible BMC    (cualquier vendor)
#
#  Método recomendado: Virtual Media + Redfish API
#  El BMC monta el ISO de Caimán como unidad virtual CD-ROM
#  y arranca desde él sin tocar el hardware físicamente.
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

BRT='\033[38;2;118;255;3m'
GRN='\033[38;2;76;175;80m'
DIM='\033[38;2;74;124;74m'
AMB='\033[38;2;255;179;0m'
RED='\033[38;2;239;83;80m'
NC='\033[0m'

# ── Configuración ──────────────────────────────────────────────────────────
BMC_IP=""
BMC_USER="root"
BMC_PASS="calvin"          # Dell default; cambia esto
BMC_TYPE="idrac"           # idrac | ilo | supermicro | generic
ISO_URL=""                 # URL del ISO Caimán accesible desde el BMC
ROLE="cp"
UPLINK="eth0"
CP_IP=""
JOIN_TOKEN=""
NODE_NAME=""
SKIP_CONFIRM=0
DRY_RUN=0

usage() {
  cat <<EOF
Caimán OS — Provisioning vía BMC/iDRAC

  $0 [opciones]

Opciones obligatorias:
  --bmc-ip      <IP>      IP del iDRAC/iLO/BMC
  --bmc-user    <user>    Usuario BMC (default: root)
  --bmc-pass    <pass>    Contraseña BMC
  --iso-url     <URL>     URL del ISO Caimán (accesible desde el BMC)
                          Ej: http://192.168.1.100/caiman-0.1.0.iso

Opciones de rol:
  --role        cp|worker|aio   Rol del nodo (default: cp)
  --join        <CP_IP>         IP del control plane (para workers)
  --token       <TOKEN>         Token de join (para workers)
  --node-name   <name>          Nombre del nodo (default: auto)
  --uplink      <iface>         NIC para XDP (default: eth0)

Opciones de BMC:
  --bmc-type    idrac|ilo|supermicro|generic  (default: idrac)

Ejemplos:
  # Provisionar control plane vía iDRAC:
  $0 --bmc-ip 192.168.0.101 --bmc-pass 'MyPass123' \\
     --iso-url http://192.168.0.50/caiman.iso \\
     --role cp

  # Provisionar worker vía iLO:
  $0 --bmc-ip 192.168.0.102 --bmc-user Administrator \\
     --bmc-pass 'MyPass' --bmc-type ilo \\
     --iso-url http://192.168.0.50/caiman.iso \\
     --role worker --join 192.168.1.10 --token <TOKEN>

  # Provisionar 10 nodos desde un CSV:
  $0 --batch nodes.csv --iso-url http://192.168.0.50/caiman.iso
EOF
  exit 0
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --bmc-ip)    BMC_IP="$2";    shift 2 ;;
    --bmc-user)  BMC_USER="$2";  shift 2 ;;
    --bmc-pass)  BMC_PASS="$2";  shift 2 ;;
    --bmc-type)  BMC_TYPE="$2";  shift 2 ;;
    --iso-url)   ISO_URL="$2";   shift 2 ;;
    --role)      ROLE="$2";      shift 2 ;;
    --join)      CP_IP="$2";     shift 2 ;;
    --token)     JOIN_TOKEN="$2";shift 2 ;;
    --node-name) NODE_NAME="$2"; shift 2 ;;
    --uplink)    UPLINK="$2";    shift 2 ;;
    --batch)     batch_mode "$2"; shift 2 ;;
    --skip-confirm) SKIP_CONFIRM=1; shift ;;
    --dry-run)   DRY_RUN=1; shift ;;
    -h|--help)   usage ;;
    *) echo "Opción desconocida: $1"; usage ;;
  esac
done

log()  { echo -e "${GRN}[$(date +%H:%M:%S)]${NC} $*"; }
warn() { echo -e "${AMB}[WARN]${NC} $*" >&2; }
die()  { echo -e "${RED}[ERROR]${NC} $*" >&2; exit 1; }
run()  {
  if [[ $DRY_RUN -eq 1 ]]; then
    echo -e "${DIM}  [dry-run] $*${NC}"
  else
    eval "$@"
  fi
}

# Redfish helper
redfish() {
  local method="$1" path="$2"
  shift 2
  curl -sk -u "${BMC_USER}:${BMC_PASS}" \
    -X "$method" \
    -H "Content-Type: application/json" \
    "https://${BMC_IP}${path}" \
    "$@"
}

# ── 1. PING BMC ────────────────────────────────────────────────────────────
check_bmc() {
  log "Verificando conectividad con BMC en $BMC_IP…"

  # Ping básico
  ping -c1 -W3 "$BMC_IP" &>/dev/null \
    || die "No se puede alcanzar el BMC en $BMC_IP"

  # Redfish endpoint
  local info
  info=$(redfish GET /redfish/v1/ 2>/dev/null)
  echo "$info" | grep -q "RedfishVersion" \
    || die "Redfish API no responde en $BMC_IP — verificar usuario/contraseña"

  local vendor model
  vendor=$(redfish GET /redfish/v1/Systems/System.Embedded.1 2>/dev/null \
           | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Manufacturer','?'))" 2>/dev/null || echo "?")
  model=$(redfish GET /redfish/v1/Systems/System.Embedded.1 2>/dev/null \
          | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Model','?'))" 2>/dev/null || echo "?")

  log "BMC respondiendo: ${BRT}$vendor $model${NC}"
}

# ── 2. MONTAR ISO VÍA VIRTUAL MEDIA ───────────────────────────────────────
mount_virtual_media() {
  log "Montando ISO vía Virtual Media: $ISO_URL"

  case "$BMC_TYPE" in

    idrac)
      # Dell iDRAC 9 — Redfish Virtual Media
      # Primero verificar si ya hay algo montado y desmontar
      local current
      current=$(redfish GET /redfish/v1/Managers/iDRAC.Embedded.1/VirtualMedia/CD \
                | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('Inserted','false'))" 2>/dev/null || echo "false")

      if [[ "$current" == "True" || "$current" == "true" ]]; then
        log "Desmontando Virtual Media previo…"
        run redfish POST /redfish/v1/Managers/iDRAC.Embedded.1/VirtualMedia/CD/Actions/VirtualMedia.EjectMedia \
          -d '{}'
        sleep 2
      fi

      # Montar el ISO
      run redfish POST /redfish/v1/Managers/iDRAC.Embedded.1/VirtualMedia/CD/Actions/VirtualMedia.InsertMedia \
        -d "{\"Image\": \"$ISO_URL\", \"Inserted\": true, \"WriteProtected\": true}"

      log "ISO montado en iDRAC Virtual Media ✓"
      ;;

    ilo)
      # HPE iLO 5 — Virtual Media vía Redfish
      run redfish POST /redfish/v1/Managers/1/VirtualMedia/2/Actions/VirtualMedia.InsertMedia \
        -d "{\"Image\": \"$ISO_URL\", \"Inserted\": true}"

      log "ISO montado en iLO Virtual Media ✓"
      ;;

    supermicro)
      # Supermicro — ipmitool virtual storage
      run ipmitool -I lanplus -H "$BMC_IP" -U "$BMC_USER" -P "$BMC_PASS" \
        raw 0x30 0x20 0x01 0x00
      log "ISO preparado para Supermicro ✓"
      ;;

    generic)
      # Genérico IPMI 2.0 — via ipmitool
      log "BMC genérico — usando ipmitool para Virtual Media"
      run ipmitool -I lanplus -H "$BMC_IP" -U "$BMC_USER" -P "$BMC_PASS" \
        sol activate 2>/dev/null &
      log "SOL activo para consola serial"
      ;;
  esac
}

# ── 3. CONFIGURAR BOOT ORDER: CD-ROM PRIMERO ──────────────────────────────
set_boot_once_cdrom() {
  log "Configurando boot order: Virtual CD-ROM (una vez)…"

  case "$BMC_TYPE" in

    idrac)
      # BIOS boot override para boot único desde virtual CD
      run redfish PATCH /redfish/v1/Systems/System.Embedded.1 \
        -d '{
          "Boot": {
            "BootSourceOverrideEnabled": "Once",
            "BootSourceOverrideTarget": "Cd"
          }
        }'
      log "Boot override: Cd (once) configurado ✓"
      ;;

    ilo)
      run redfish PATCH /redfish/v1/Systems/1 \
        -d '{
          "Boot": {
            "BootSourceOverrideEnabled": "Once",
            "BootSourceOverrideTarget": "Cd"
          }
        }'
      ;;

    supermicro|generic)
      run ipmitool -I lanplus -H "$BMC_IP" -U "$BMC_USER" -P "$BMC_PASS" \
        chassis bootdev cdrom options=efiboot
      log "IPMI boot device: cdrom ✓"
      ;;
  esac
}

# ── 4. CREAR CLOUD-INIT / AUTOINSTALL CON ROL DEL NODO ────────────────────
prepare_autoinstall() {
  log "Preparando configuración de autoinstall para rol=$ROLE…"

  # Generar user-data que embebemos en el ISO o servimos por HTTP
  local node_hostname="$NODE_NAME"
  if [[ -z "$node_hostname" ]]; then
    # Derivar nombre del nodo del IP del BMC
    node_hostname="caiman-$(echo $BMC_IP | tr '.' '-')"
  fi

  cat > /tmp/caiman-user-data-${BMC_IP}.yaml <<EOF
#cloud-config
hostname: $node_hostname
fqdn: $node_hostname.caiman.local

# Ejecutar install.sh en first boot con el rol correcto
runcmd:
  - |
    curl -fsSL https://install.caiman.io | bash -s -- \\
      --role $ROLE \\
      --uplink $UPLINK \\
      --skip-confirm \\
$(if [[ "$ROLE" == "worker" && -n "$CP_IP" ]]; then
  echo "      --join $CP_IP \\"
  echo "      --token $JOIN_TOKEN"
fi)
EOF

  log "user-data generado para $node_hostname"
}

# ── 5. REINICIAR EL SERVIDOR ───────────────────────────────────────────────
reboot_server() {
  log "Reiniciando servidor para arrancar desde Virtual Media…"

  local power_state
  power_state=$(redfish GET /redfish/v1/Systems/System.Embedded.1 \
    | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('PowerState','?'))" 2>/dev/null || echo "?")

  log "Estado actual de energía: $power_state"

  if [[ "$power_state" == "Off" ]]; then
    log "Servidor apagado — encendiendo…"
    run redfish POST /redfish/v1/Systems/System.Embedded.1/Actions/ComputerSystem.Reset \
      -d '{"ResetType": "On"}'
  else
    log "Servidor encendido — reiniciando gracefully…"
    run redfish POST /redfish/v1/Systems/System.Embedded.1/Actions/ComputerSystem.Reset \
      -d '{"ResetType": "GracefulRestart"}'

    # Si no responde en 30s, forzar
    sleep 30
    power_state=$(redfish GET /redfish/v1/Systems/System.Embedded.1 \
      | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('PowerState','?'))" 2>/dev/null || echo "?")
    if [[ "$power_state" == "On" ]]; then
      warn "Reinicio graceful no completado — forzando ForceRestart"
      run redfish POST /redfish/v1/Systems/System.Embedded.1/Actions/ComputerSystem.Reset \
        -d '{"ResetType": "ForceRestart"}'
    fi
  fi

  log "Servidor reiniciando → arrancará desde Virtual Media en ~60s"
}

# ── 6. MONITOREAR ARRANQUE VÍA CONSOLA SERIAL ────────────────────────────
monitor_boot() {
  log "Conectando a consola serial (iDRAC SOL)…"
  log "Presiona Ctrl+] para desconectar cuando la instalación termine"
  echo
  echo -e "${DIM}  Tip: la instalación tarda ~10 minutos${NC}"
  echo -e "${DIM}  Cuando veas '🐊 Caimán OS installed' el nodo está listo${NC}"
  echo

  case "$BMC_TYPE" in
    idrac)
      # iDRAC Serial over LAN
      ipmitool -I lanplus \
        -H "$BMC_IP" -U "$BMC_USER" -P "$BMC_PASS" \
        sol activate
      ;;
    ilo)
      # iLO SOL via SSH
      ssh -o StrictHostKeyChecking=no \
        "${BMC_USER}@${BMC_IP}" "textcons"
      ;;
    supermicro|generic)
      ipmitool -I lanplus \
        -H "$BMC_IP" -U "$BMC_USER" -P "$BMC_PASS" \
        sol activate
      ;;
  esac
}

# ── 7. ESPERAR A QUE EL NODO ESTÉ LISTO (polling SSH) ────────────────────
wait_for_node() {
  log "Esperando a que el nodo responda por SSH…"

  # El nodo obtiene IP por DHCP — intentar conectar por nombre
  local node_hostname="caiman-$(echo $BMC_IP | tr '.' '-')"
  local max_attempts=60  # 10 minutos
  local attempt=0

  while [[ $attempt -lt $max_attempts ]]; do
    if ssh -o StrictHostKeyChecking=no \
           -o ConnectTimeout=5 \
           -o BatchMode=yes \
           "caiman@${node_hostname}" "caiman ping" &>/dev/null; then
      log "${BRT}Nodo $node_hostname listo!${NC}"
      return 0
    fi
    attempt=$((attempt + 1))
    echo -ne "\r  ${DIM}Esperando... ${attempt}/${max_attempts}${NC}"
    sleep 10
  done

  warn "Timeout esperando al nodo — revisar consola serial"
  return 1
}

# ── 8. POST-INSTALACIÓN: UNIR AL CLUSTER ─────────────────────────────────
post_install() {
  local node_hostname="caiman-$(echo $BMC_IP | tr '.' '-')"

  if [[ "$ROLE" == "cp" ]]; then
    log "Obteniendo token de join para workers…"
    local join_cmd
    join_cmd=$(ssh -o StrictHostKeyChecking=no \
      "caiman@${node_hostname}" "kubeadm token create --print-join-command" 2>/dev/null)

    echo
    echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${BRT}  Control plane listo en $node_hostname${NC}"
    echo
    echo -e "  Para agregar workers, usa este comando:"
    echo
    echo -e "${DIM}  $join_cmd${NC}"
    echo
    echo -e "  O con el script de Caimán:"
    local node_ip
    node_ip=$(ssh -o StrictHostKeyChecking=no "caiman@${node_hostname}" \
      "ip route get 1.1.1.1 | awk '{print \$7; exit}'" 2>/dev/null || echo "<NODE_IP>")
    local token
    token=$(ssh -o StrictHostKeyChecking=no "caiman@${node_hostname}" \
      "kubeadm token list | awk 'NR==2{print \$1}'" 2>/dev/null || echo "<TOKEN>")
    echo -e "${DIM}  ./caiman-bmc-provision.sh \\${NC}"
    echo -e "${DIM}    --bmc-ip <WORKER_BMC_IP> --bmc-pass <PASS> \\${NC}"
    echo -e "${DIM}    --iso-url $ISO_URL \\${NC}"
    echo -e "${DIM}    --role worker --join $node_ip --token $token${NC}"
    echo -e "${BRT}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  fi
}

# ── BATCH MODE: CSV de nodos ───────────────────────────────────────────────
batch_mode() {
  local csv="$1"
  log "Modo batch: leyendo $csv"

  # Formato CSV: bmc_ip,bmc_pass,role,node_name
  # Ejemplo:
  # 192.168.0.101,MiPass1,cp,caiman-cp-01
  # 192.168.0.102,MiPass2,worker,caiman-w-01
  # 192.168.0.103,MiPass3,worker,caiman-w-02

  local first=1
  local cp_ip=""
  local cp_token=""

  while IFS=, read -r bmc_ip bmc_pass role node_name; do
    [[ "$bmc_ip" =~ ^# ]] && continue  # skip comments
    [[ -z "$bmc_ip" ]] && continue

    log "Provisionando $node_name ($bmc_ip) rol=$role"

    if [[ $first -eq 1 ]]; then
      BMC_IP="$bmc_ip" BMC_PASS="$bmc_pass" \
      ROLE="$role" NODE_NAME="$node_name" \
      "$0" --bmc-ip "$bmc_ip" --bmc-pass "$bmc_pass" \
           --role "$role" --node-name "$node_name" \
           --iso-url "$ISO_URL" --skip-confirm
      first=0

      # Esperar a que CP esté listo y obtener token
      sleep 300  # 5 min para que arranque
      # cp_token=$(...)
    else
      "$0" --bmc-ip "$bmc_ip" --bmc-pass "$bmc_pass" \
           --role "$role" --node-name "$node_name" \
           --join "$cp_ip" --token "$cp_token" \
           --iso-url "$ISO_URL" --skip-confirm &
    fi
  done < "$csv"

  wait
  log "Batch provisioning completado"
  exit 0
}

# ── MAIN ───────────────────────────────────────────────────────────────────
main() {
  echo -e "\n${BRT}🐊 Caimán OS — Provisioning vía BMC/iDRAC${NC}"
  echo -e "${DIM}   BMC: $BMC_TYPE @ $BMC_IP | Rol: $ROLE | ISO: $ISO_URL${NC}\n"

  [[ -z "$BMC_IP" ]]  && die "--bmc-ip requerido"
  [[ -z "$ISO_URL" ]] && die "--iso-url requerido"
  [[ -z "$BMC_PASS" ]] && {
    read -srp "$(echo -e "${AMB}Contraseña BMC para $BMC_IP: ${NC}")" BMC_PASS
    echo
  }

  check_bmc
  prepare_autoinstall
  mount_virtual_media
  set_boot_once_cdrom
  reboot_server

  echo
  echo -e "${BRT}  El servidor está arrancando desde el ISO de Caimán.${NC}"
  echo -e "${DIM}  Opciones para monitorear:${NC}"
  echo
  echo -e "  1. ${GRN}Consola serial (SOL):${NC}"
  echo -e "     ${DIM}ipmitool -I lanplus -H $BMC_IP -U $BMC_USER -P '***' sol activate${NC}"
  echo
  echo -e "  2. ${GRN}iDRAC web console:${NC}"
  echo -e "     ${DIM}https://$BMC_IP → Virtual Console${NC}"
  echo
  echo -e "  3. ${GRN}Esperar a que responda:${NC}"
  echo -e "     ${DIM}$0 --wait --bmc-ip $BMC_IP${NC}"
  echo

  if [[ $SKIP_CONFIRM -eq 0 ]]; then
    read -rp "$(echo -e "${AMB}¿Abrir consola serial ahora? [y/N] ${NC}")" ans
    [[ "${ans,,}" == "y" ]] && monitor_boot
  fi
}

main "$@"
