# ISO Installation Guide

## Download

| Version | Size | Download | SHA256 |
|---------|------|----------|--------|
| v1.1.0 | 111 MB | [caiman-os-1.1.0-x86_64.iso](https://github.com/teampmladv/caiman-os/releases/download/v1.1.0/caiman-os-1.1.0-x86_64.iso) | `f6b513b069da67e7ffb84602f9aac753023df94e0d2b2ffeb29d4abae2484509` |

**vs. competition:**

| Product | ISO Size |
|---------|----------|
| **Caimán OS** | **111 MB** |
| VMware ESXi | 350 MB |
| Proxmox VE | 1.2 GB |
| Nutanix CE | 8.5 GB |

---

## Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| CPU | x86_64 + VT-x/AMD-V | Intel Xeon / AMD EPYC |
| RAM | 4 GiB | 32+ GiB |
| Storage | 50 GiB | 1+ TiB NVMe |
| Network | 1 GbE | 25/100 GbE |
| Boot | UEFI | UEFI |

---

## Flash to USB

```bash
# Linux / macOS
dd if=caiman-os-1.1.0-x86_64.iso of=/dev/sdX bs=4M status=progress

# Or use Balena Etcher (GUI) — https://etcher.balena.io
```

---

## Boot from USB

1. Insert the USB drive
2. Boot your server from USB (F11 or F12 for boot menu)
3. Select **"🐊 Install Caimán OS"**

---

## Installer walkthrough

The TUI installer guides you through:

### 1. Hardware check
```
━━ Checking hardware
  ✓ CPU virtualization supported (AMD EPYC 7443P)
  ✓ 256 GiB RAM
  ✓ Architecture: x86_64
```

### 2. Disk selection
```
━━ Select installation disk

  [1] /dev/nvme0n1  2000 GiB  Samsung PM9A3
  [2] /dev/nvme1n1  2000 GiB  Samsung PM9A3
  [3] /dev/sda       500 GiB  HGST HUS726500

  ? Enter disk number [1]: 1

  ⚠ ALL DATA ON /dev/nvme0n1 WILL BE ERASED
  ? Type 'yes' to confirm: yes
```

### 3. Network
```
  ? Primary interface [eth0]: eth0
  ? IP configuration — (1) DHCP  (2) Static [1]: 2
  ? IP address (e.g. 192.168.1.100/24): 10.0.1.10/24
  ? Gateway: 10.0.1.1
  ? DNS [1.1.1.1]: 1.1.1.1
```

### 4. System
```
  ? Hostname [caiman-node-01]: prod-node-01
  ? Admin password: ********
  ? Confirm password: ********
```

### 5. Cluster mode
```
  ? Cluster mode — (1) Standalone  (2) Join existing cluster [1]:
```

**Standalone** — first node, creates a new cluster.

**Join cluster** — additional node. Needs the API URL and join token from the first node:
```
  ? Cluster API URL: https://10.0.1.10:8765
  ? Cluster join token: caiman-join-xxxxxxxxxxxx
```

### 6. Installation summary
```
━━ Installation summary

  Disk:      /dev/nvme0n1
  Hostname:  prod-node-01
  Network:   eth0 10.0.1.10/24
  Mode:      Standalone
  Version:   Caimán OS 1.1.0

  ? Start installation? [y/N]: y
```

### 7. Installation progress
```
━━ Installing Caimán OS

  Partitioning /dev/nvme0n1... ✓
  Formatting partitions... ✓
  Installing Alpine base... ✓
  Installing Caimán OS binaries... ✓
  Configuring system... ✓
  Installing GRUB2 bootloader... ✓
  Setting password... ✓
  Enabling services... ✓
  Finalizing... ✓

  ─────────────────────────────────────────────────────────
  🐊 Caimán OS 1.1.0 installed successfully!

  Remove the installation media and reboot.
  Dashboard will be available at:
    http://prod-node-01:3000
    http://10.0.1.10:3000

  ? Reboot now? [Y/n]: Y
```

---

## After installation

After rebooting, Caimán OS starts automatically:

```
caiman-api    → http://<ip>:8765   REST API
caiman-ui     → http://<ip>:3000   Dashboard
caiman-drs    → http://<ip>:8766   DRS scheduler
caiman-grafana→ http://<ip>:3001   Metrics (admin/caiman)
```

### First VM

```bash
curl -X POST http://10.0.1.10:8765/api/vms \
  -H 'Content-Type: application/json' \
  -d '{
    "name":   "web-01",
    "cpus":   2,
    "memMib": 1024,
    "kernel": "/var/lib/caiman/vmlinuz"
  }'
```

### Add a second node

On the second server, boot from the same ISO and select **"Join existing cluster"**:
- Cluster API URL: `https://10.0.1.10:8765`
- Join token: get it from the first node:

```bash
curl http://10.0.1.10:8765/api/cluster/join-token
```

---

## Verify ISO integrity

```bash
sha256sum -c caiman-os-1.1.0-x86_64.iso.sha256
# caiman-os-1.1.0-x86_64.iso: OK
```

---

## Test with QEMU (no hardware needed)

```bash
# Install QEMU
apt-get install -y qemu-system-x86

# Boot the ISO
qemu-system-x86_64 \
  -cdrom caiman-os-1.1.0-x86_64.iso \
  -m 4G \
  -enable-kvm \
  -cpu host \
  -smp 2 \
  -boot d \
  -nographic
```

---

## Build from source

```bash
git clone https://github.com/teampmladv/caiman-os
cd caiman-os
sudo ./iso/scripts/build-iso.sh 1.1.0
# → caiman-os-1.1.0-x86_64.iso (111 MB)
```

Build requirements:
```bash
# Ubuntu/Debian
apt-get install -y xorriso grub-efi-amd64-bin squashfs-tools curl

# CentOS/RHEL
dnf install -y xorriso grub2-tools-extra squashfs-tools curl
```
