# Installation Guide

## Requirements

| Requirement | Minimum | Recommended |
|-------------|---------|-------------|
| Architecture | x86_64 | x86_64 |
| CPU | VT-x or AMD-V | Intel Xeon / AMD EPYC |
| RAM | 4 GiB | 32+ GiB |
| Disk | 50 GiB | 1+ TiB NVMe |
| OS | CentOS 8+, Ubuntu 22.04+, Debian 12+ | CentOS Stream 9 |
| Kernel | 5.15+ | 6.8+ |
| Network | 1 GbE | 10/25/100 GbE |

**Verify KVM support:**
```bash
grep -E "vmx|svm" /proc/cpuinfo | head -1
ls /dev/kvm
```

---

## Method 1 — One-command install (recommended)

```bash
curl -fsSL https://caimanos.com/install.sh | sudo bash
```

This script:
1. Verifies CPU virtualization support
2. Installs Docker if missing
3. Creates network bridge `caiman0` (10.100.0.1/24)
4. Enables IP forwarding + NAT
5. Copies `caiman-vmm` binary from container
6. Copies the running kernel to `/var/lib/caiman/vmlinuz`
7. Creates a 4 GiB test disk image
8. Pulls OCI images and starts the stack

---

## Method 2 — Manual install

### 1. Install Docker

```bash
# CentOS / RHEL
curl -fsSL https://get.docker.com | sh
systemctl enable --now docker

# Ubuntu / Debian
curl -fsSL https://get.docker.com | sh
systemctl enable --now docker
```

### 2. Create directories

```bash
mkdir -p /var/run/caiman
mkdir -p /var/lib/caiman/disks
mkdir -p /var/lib/caiman/kernels
```

### 3. Copy kernel

```bash
cp /boot/vmlinuz-$(uname -r) /var/lib/caiman/vmlinuz
```

### 4. Network bridge

```bash
ip link add name caiman0 type bridge
ip link set caiman0 up
ip addr add 10.100.0.1/24 dev caiman0

# Enable forwarding + NAT
echo 1 > /proc/sys/net/ipv4/ip_forward
iptables -t nat -A POSTROUTING -s 10.100.0.0/24 ! -d 10.100.0.0/24 -j MASQUERADE

# Persist
echo "net.ipv4.ip_forward = 1" >> /etc/sysctl.conf
```

### 5. Start the stack

```bash
git clone https://github.com/teampmladv/caiman-os
cd caiman-os
docker compose up -d
```

### 6. Extract caiman-vmm binary

```bash
docker create --name tmp ghcr.io/teampmladv/caiman-vmm:0.7.0
docker cp tmp:/usr/local/bin/caiman-vmm /usr/local/bin/caiman-vmm
docker rm tmp
chmod +x /usr/local/bin/caiman-vmm
```

---

## Method 3 — PXE network boot

For bare metal provisioning via iPXE:

```bash
# boot.ipxe
#!ipxe
kernel https://caimanos.com/boot/vmlinuz console=ttyS0 caiman.url=https://caimanos.com
initrd https://caimanos.com/boot/initrd.img
boot
```

See [`install/pxe/boot.ipxe`](../../install/pxe/boot.ipxe) for the full configuration.

---

## Method 4 — iDRAC / BMC provisioning

For bulk provisioning via Redfish API (Dell iDRAC, HP iLO, Supermicro BMC):

```bash
sudo ./install/scripts/caiman-bmc-provision.sh \
  --bmc-ip    192.168.1.100 \
  --bmc-user  root \
  --bmc-pass  calvin \
  --iso-url   https://caimanos.com/caiman.iso
```

This mounts the Caimán OS ISO via Virtual Media and boots the server remotely.

---

## Post-install verification

```bash
# Check all services are running
docker compose ps

# Verify the API responds
curl http://localhost:8765/health

# Create a test VM
curl -X POST http://localhost:8765/api/vms \
  -H 'Content-Type: application/json' \
  -d '{
    "name":   "test-vm",
    "cpus":   1,
    "memMib": 256,
    "kernel": "/var/lib/caiman/vmlinuz"
  }'

# Open the dashboard
open http://localhost:3000
```

---

## Firewall configuration

Ports used by Caimán OS:

| Port | Service | Protocol | Description |
|------|---------|----------|-------------|
| 3000 | caiman-ui | TCP | Dashboard |
| 8765 | caiman-api | TCP | REST API + WebSocket |
| 8766 | caiman-drs | TCP | DRS scheduler |
| 8767 | caiman-mcp | TCP | MCP server |
| 8768 | caiman-bts | TCP | BTS server |
| 7777 | caiman-livemig | TCP | Live migration |
| 9091 | prometheus | TCP | Metrics |
| 3001 | grafana | TCP | Dashboards |

```bash
# CentOS / RHEL (firewalld)
firewall-cmd --permanent --add-port=8765/tcp
firewall-cmd --permanent --add-port=3000/tcp
firewall-cmd --reload

# Ubuntu / Debian (ufw)
ufw allow 8765/tcp
ufw allow 3000/tcp
```

---

## Upgrading

```bash
# Pull new images
docker compose pull

# Restart with new images
docker compose up -d

# Verify
curl http://localhost:8765/health
```

---

## Uninstall

```bash
# Stop all services
docker compose down

# Remove images
docker rmi $(docker images 'ghcr.io/teampmladv/caiman-*' -q)

# Remove data (WARNING: destroys all VMs)
rm -rf /var/run/caiman /var/lib/caiman

# Remove network bridge
ip link del caiman0
```

---

## Troubleshooting

### `/dev/kvm` not found

```bash
# Check CPU support
grep -E "vmx|svm" /proc/cpuinfo

# Load KVM modules
modprobe kvm kvm_intel   # Intel
modprobe kvm kvm_amd     # AMD

# On VPS: contact provider to enable nested virtualization
```

### API not responding

```bash
docker logs caimanapi | tail -20
curl http://127.0.0.1:8765/health

# Check firewall
iptables -L -n | grep 8765
# If blocked: csf -a 127.0.0.1 8765
```

### VM stuck in BOOTING

```bash
# Check serial console
curl http://localhost:8765/api/vms/{id}/console

# Check if caiman-vmm is running
ps aux | grep caiman-vmm

# Check logs
tail -f /var/run/caiman/{id}.log
```

### Out of disk space for VM images

```bash
# Check usage
df -h /var/lib/caiman

# Move to larger filesystem
mv /var/lib/caiman /mnt/large-disk/caiman
ln -sfn /mnt/large-disk/caiman /var/lib/caiman
```
