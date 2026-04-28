# ═══════════════════════════════════════════════════════════════════════════
#  Caimán OS — Terraform para Hetzner Robot (servidores dedicados)
#  Hetzner Robot API = equivalente al iDRAC en Hetzner
#  Permite: reinstalar SO, configurar boot, montar ISO, reboot
# ═══════════════════════════════════════════════════════════════════════════

terraform {
  required_providers {
    hcloud = {
      source  = "hetznercloud/hcloud"
      version = "~> 1.45"
    }
    # Para servidores dedicados (Robot API)
    hetznerdns = {
      source  = "timohirt/hetznerdns"
      version = "~> 2.2"
    }
  }
}

# ── Variables ──────────────────────────────────────────────────────────────
variable "hcloud_token"       { sensitive = true }
variable "robot_user"         { description = "Hetzner Robot usuario" }
variable "robot_pass"         { sensitive = true }
variable "caiman_iso_url"     { description = "URL del ISO Caimán" }
variable "ssh_public_key"     { description = "Tu SSH public key" }
variable "node_count"         { default = 3 }

# ── SSH Key ────────────────────────────────────────────────────────────────
resource "hcloud_ssh_key" "caiman" {
  name       = "caiman-key"
  public_key = var.ssh_public_key
}

# ── Red privada para comunicación inter-nodo ───────────────────────────────
resource "hcloud_network" "caiman" {
  name     = "caiman-cluster"
  ip_range = "10.100.0.0/16"
}

resource "hcloud_network_subnet" "caiman" {
  type         = "cloud"
  network_id   = hcloud_network.caiman.id
  network_zone = "eu-central"
  ip_range     = "10.100.1.0/24"
}

# ──────────────────────────────────────────────────────────────────────────
# OPCIÓN A: Hetzner Cloud (VMs) — rápido para testing
# Limitación: KVM anidado, XDP genérico, no NVMe real
# ──────────────────────────────────────────────────────────────────────────

resource "hcloud_server" "caiman_cloud" {
  count       = 0  # cambiar a 3 para cloud VMs
  name        = "caiman-node-${format("%02d", count.index + 1)}"
  server_type = "cx52"          # 16 vCPU, 32 GB RAM, €0.063/h
  image       = "ubuntu-22.04"
  location    = "nbg1"
  ssh_keys    = [hcloud_ssh_key.caiman.id]

  network {
    network_id = hcloud_network.caiman.id
    ip         = "10.100.1.${10 + count.index}"
  }

  # cloud-init: ejecutar install.sh en first boot
  user_data = count.index == 0 ? templatefile("${path.module}/cloud-init-cp.yaml", {
    iso_url = var.caiman_iso_url
    uplink  = "eth0"
  }) : templatefile("${path.module}/cloud-init-worker.yaml", {
    cp_ip  = hcloud_server.caiman_cloud[0].ipv4_address
    uplink = "eth0"
  })

  labels = {
    "caiman-role" = count.index == 0 ? "cp" : "worker"
    "cluster"     = "havana"
  }
}

# ──────────────────────────────────────────────────────────────────────────
# OPCIÓN B: Hetzner Dedicated (bare metal) — producción real
# Robot API para reinstalar + configurar boot
# ──────────────────────────────────────────────────────────────────────────

# Nota: Hetzner Dedicated se provisiona vía Robot API (REST)
# Los servidores dedicados son alquilados separadamente en:
#   https://robot.hetzner.com/server
# Aquí automatizamos el proceso de instalación vía API

locals {
  # IPs de los servidores dedicados que ya tienes en Hetzner Robot
  # Completa con las IPs de tus servidores
  dedicated_servers = [
    { ip = "5.9.xxx.xxx",   bmc_ip = "5.9.xxx.yyy",   role = "cp"     },
    { ip = "5.9.xxx.xxx2",  bmc_ip = "5.9.xxx.yyy2",  role = "worker" },
    { ip = "5.9.xxx.xxx3",  bmc_ip = "5.9.xxx.yyy3",  role = "worker" },
  ]
}

# Provisionar bare metal vía Robot API usando null_resource + local-exec
# Hetzner Robot no tiene provider Terraform oficial para bare metal,
# pero expone una REST API que usamos directamente

resource "null_resource" "caiman_dedicated" {
  count = 0  # cambiar a length(local.dedicated_servers) para activar

  triggers = {
    server_ip = local.dedicated_servers[count.index].ip
    role      = local.dedicated_servers[count.index].role
  }

  provisioner "local-exec" {
    command = <<-CMD
      # Usar Robot API para activar el rescue system
      # y luego instalar Caimán desde ahí

      SERVER_IP="${local.dedicated_servers[count.index].ip}"
      ROLE="${local.dedicated_servers[count.index].role}"

      # 1. Activar Hetzner Rescue System (Linux 64bit)
      curl -su '${var.robot_user}:${var.robot_pass}' \
        -X POST "https://robot-ws.your-server.de/boot/$SERVER_IP/rescue" \
        -d "os=linux&arch=64&authorized_key=${hcloud_ssh_key.caiman.fingerprint}" \
        | jq '.rescue.password'

      # 2. Reiniciar el servidor en rescue
      curl -su '${var.robot_user}:${var.robot_pass}' \
        -X POST "https://robot-ws.your-server.de/reset/$SERVER_IP" \
        -d "type=hw"

      # 3. Esperar ~60s y conectar al rescue system
      sleep 90

      # 4. Desde el rescue system, instalar Caimán con installimage
      # O usar nuestro install.sh directamente
      ssh -o StrictHostKeyChecking=no root@$SERVER_IP \
        "curl -fsSL ${var.caiman_iso_url}/install.sh | bash -s -- \
          --role $ROLE \
          --uplink eth0 \
          --skip-confirm"
    CMD
  }
}

# ── Outputs ────────────────────────────────────────────────────────────────
output "cloud_node_ips" {
  value = [for s in hcloud_server.caiman_cloud : s.ipv4_address]
}

output "install_commands" {
  value = {
    cp = "ssh ubuntu@${try(hcloud_server.caiman_cloud[0].ipv4_address, "N/A")} 'caiman cluster status'"
    dashboard = "http://${try(hcloud_server.caiman_cloud[0].ipv4_address, "N/A")}:3000"
    grafana   = "http://${try(hcloud_server.caiman_cloud[0].ipv4_address, "N/A")}:3001"
  }
}
