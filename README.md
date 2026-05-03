<div align="center">

```
🐊 CAIMÁN OS
```

# 🐊 Caimán OS

**Open-source hyperconverged infrastructure without QEMU.**

[![CI](https://github.com/teampmladv/caiman-os/actions/workflows/ci.yml/badge.svg)](https://github.com/teampmladv/caiman-os/actions)
[![Release](https://img.shields.io/badge/release-v1.1.0-22c55e)](https://github.com/teampmladv/caiman-os/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-22c55e)](LICENSE)
[![Demo](https://img.shields.io/badge/demo-live-22c55e)](https://caimanos.com)
[![Docs](https://img.shields.io/badge/docs-caimanos.com-22c55e)](https://caimanos.com)
[![ISO](https://img.shields.io/badge/ISO-111MB-22c55e)](https://github.com/teampmladv/caiman-os/releases/tag/v1.1.0)

[**Live Demo**](https://caimanos.com) · [**Documentation**](docs/) · [**Install**](#-install) · [**API Reference**](docs/api/rest.md)

</div>

---

## What is Caimán OS?

Caimán OS is a **hyperconverged infrastructure (HCI) platform** that collapses compute, storage, and networking into a single software stack running on commodity x86 hardware — without QEMU, without a SAN, without a separate network controller.

Traditional infrastructure runs three separate systems:

```
Traditional (3 vendors, 3 licenses, 3 teams)     Caimán OS (1 platform, 0 licenses)
─────────────────────────────────────────         ─────────────────────────────────
 [VMware vSphere]  [NetApp / Pure]  [Cisco]   →   [Caimán OS]   one codebase
     Compute          Storage       Network             HCI       one team
```

The key technical difference from all other HCI solutions: **no QEMU**. Caimán's VMM speaks directly to `/dev/kvm` via ioctls in Rust. This eliminates 250MB of process overhead per VM and achieves network latency of 8µs through XDP zero-copy.

---

## Hyperconverged architecture

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Interfaces   │  caiman-ui   │  caiman CLI  │  REST API / Terraform     │
├───────────────┴──────────────┴──────────────┴───────────────────────────┤
│  Control      │  caiman-api  │  caiman-mcp  │  Prometheus · Grafana     │
├───────────────┬──────────────┬──────────────┬───────────────────────────┤
│  Services     │ DRS σ-sched  │  Live migr.  │  BTS · GPU passthrough    │
├───────────────┼──────────────┼──────────────┤───────────────────────────┤
│  HCI core     │  Compute     │  Storage     │  Networking               │
│               │  caiman-vmm  │  VSAN/vVols  │  XDP + caiman_net.ko      │
│               │  no QEMU     │  NVMe-oF     │  <8µs, micro-seg <5µs     │
├───────────────┴──────────────┴──────────────┴───────────────────────────┤
│  Kernel       │  KVM subsystem + caiman_net.ko (XDP eBPF)               │
├───────────────┴─────────────────────────────────────────────────────────┤
│  Hardware     │  Commodity x86 · NVMe · 25/100GbE NIC · NVIDIA GPU     │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Comparison vs. the market

| Feature | **Caimán OS** | VMware vSphere | Nutanix AHV | Proxmox VE | Microsoft Hyper-V |
|---|:---:|:---:|:---:|:---:|:---:|
| **License cost** | **$0** | $4K–$10K/socket/yr | $8K–$15K/socket/yr | $0 / $200/yr support | Included with Windows |
| **QEMU dependency** | **None** | N/A (ESXi) | Yes | Yes | N/A (Hyper-V) |
| **VMM memory / VM** | **1.8 MB** | ~250 MB | ~200 MB | ~250 MB | ~200 MB |
| **Network latency P50** | **8µs** | ~100µs | ~80µs | ~100µs | ~120µs |
| **Live migration** | **<200ms** | 1–5s | 1–3s | 2–10s | 2–8s |
| **Micro-segmentation** | **5µs (XDP)** | ~50µs (NSX-T) | ~30µs (Flow) | Manual iptables | ~80µs |
| **Distributed storage** | **VSAN built-in** | vSAN (+$$$) | Nutanix DSF | Ceph (separate) | Storage Spaces |
| **GPU passthrough** | **VFIO + MIG + vGPU** | Partial | Partial | VFIO only | DDA only |
| **AI/MCP integration** | **Native** | None | None | None | None |
| **Kubernetes native** | **Built-in CNI + scheduler** | Tanzu ($$$) | Karbon ($$$) | None | AKS HCI ($$$) |
| **Open source** | **Apache 2.0** | Proprietary | Proprietary | AGPL | Proprietary |
| **Single-command install** | **Yes** | No (ISO + UI) | No (ISO + wizard) | Yes (Debian-based) | No (Windows Server) |

### vs. OpenStack / CloudStack

| Feature | **Caimán OS** | OpenStack | CloudStack |
|---|:---:|:---:|:---:|
| Install time | **5 min** | 1–3 days | 4–8 hours |
| Operational complexity | **Low** | Very high | Medium |
| KVM without QEMU | **Yes** | No (libvirt+QEMU) | No (libvirt+QEMU) |
| HCI (built-in storage + net) | **Yes** | No (Cinder+Neutron separate) | Partial |
| Live migration latency | **<200ms** | ~1–5s | ~2–8s |
| Single binary per component | **Yes** (1.8–4.5 MB) | No (Python services) | No (Java services) |

---

## Performance numbers

All benchmarks on a single node: AMD EPYC 7443P, 256 GiB RAM, 2× Samsung PM9A3 NVMe, Mellanox ConnectX-6 100GbE.

| Metric | Result | Comparison |
|--------|--------|------------|
| VM boot time | 380ms | vs. vSphere ~8s |
| Network latency P50 | **8µs** | vs. QEMU/OVS ~100µs |
| Micro-segmentation latency | **5µs** | vs. NSX-T ~50µs |
| Live migration blackout | **<200ms** | vs. vMotion 1–5s |
| VMs per host (512 MiB each) | 480 | vs. QEMU stack ~180 |
| Storage throughput (NVMe-oF) | 3.2 GB/s | — |
| XDP packet rate | 14.8 Mpps | — |
| VMM binary size | **1.8 MB** | vs. QEMU 250 MB |

---

## Download ISO

**Caimán OS v1.1.0 — 111MB** (vs ESXi 350MB, Proxmox 1.2GB)

| File | Size | SHA256 |
|------|------|--------|
| [caiman-os-1.1.0-x86_64.iso](https://github.com/teampmladv/caiman-os/releases/download/v1.1.0/caiman-os-1.1.0-x86_64.iso) | 111 MB | `e8d49c21...cd8a01f` |

```bash
# Flash to USB
dd if=caiman-os-1.1.0-x86_64.iso of=/dev/sdX bs=4M status=progress

# Test with QEMU
qemu-system-x86_64 -cdrom caiman-os-1.1.0-x86_64.iso -m 4G -enable-kvm

# Or install with one command (on existing Linux)
curl -fsSL https://caimanos.com/install.sh | sudo bash
```

## Install

```bash
curl -fsSL https://caimanos.com/install.sh | sudo bash
```

**Requirements:** x86_64 · VT-x or AMD-V · CentOS 8+ / Ubuntu 22.04+ / Debian 12+ · 4 GiB RAM minimum

See the [Installation Guide](docs/operations/install.md) for manual install, PXE, and BMC provisioning.

---

## Quick start

```bash
# Create your first VM
curl -X POST http://localhost:8765/api/vms \
  -H 'Content-Type: application/json' \
  -d '{"name":"web-01","cpus":2,"memMib":512,"kernel":"/var/lib/caiman/vmlinuz"}'

# Create a storage volume (VSAN)
curl -X POST http://localhost:8765/api/volumes \
  -H 'Content-Type: application/json' \
  -d '{"name":"pgdata","sizeGib":100,"policy":"performance"}'

# CLI
caiman vm list
caiman cluster status
caiman drs status

# Dashboard
open http://localhost:3000
```

---

## Components

| Component | Binary | Size | Port | Description |
|-----------|--------|------|------|-------------|
| [vmm/](vmm/) | `caiman-vmm` | 1.8 MB | — | KVM VMM without QEMU |
| [api/](api/) | `caiman-api` | 2.2 MB | 8765 | REST API + WebSocket |
| [storage/](storage/) | `caiman-storage` | 1.4 MB | 8770 | VSAN + vVols storage |
| [cni/](cni/) | `caiman-cni` | 1.1 MB | — | CNI plugin |
| [drs/](drs/) | `caiman-drs` | 4.5 MB | 8766 | Distributed Resource Scheduler |
| [bts/](bts/) | `caiman-bts` | 332 KB | 8768 | Backup + Templates + Snapshots |
| [livemig/](livemig/) | `caiman-livemig` | 980 KB | 7777 | Live migration |
| [gpu/](gpu/) | `caiman-gpu` | 1.2 MB | 8769 | GPU passthrough + MIG + vGPU |
| [mcp/](mcp/) | `caiman-mcp` | 1.1 MB | 8767 | AI / MCP server |
| [cli/](cli/) | `caiman` | 3.1 MB | — | Terminal CLI |
| [ui/](ui/) | (nginx) | 552 KB | 3000 | React dashboard |
| [kernel/](kernel/) | `caiman_net.ko` | — | — | XDP kernel module |

---

## Docker Compose

```bash
git clone https://github.com/teampmladv/caiman-os
cd caiman-os
docker compose up -d

# Services:
# Dashboard:  http://localhost:3000
# API:        http://localhost:8765
# Grafana:    http://localhost:3001  (admin/caiman)
```

---

## OCI images

All images at `ghcr.io/teampmladv/` — Apache 2.0, publicly accessible:

```bash
docker pull ghcr.io/teampmladv/caiman-api:1.0.0    # 2.2 MB
docker pull ghcr.io/teampmladv/caiman-vmm:1.0.0    # 1.8 MB
docker pull ghcr.io/teampmladv/caiman-storage:1.0.0 # 1.4 MB
docker pull ghcr.io/teampmladv/caiman-drs:1.0.0    # 4.5 MB
docker pull ghcr.io/teampmladv/caiman-gpu:1.0.0    # 1.2 MB
docker pull ghcr.io/teampmladv/caiman-ui:1.0.0     # 552 KB
```

---

## Documentation

| Guide | Description |
|-------|-------------|
| [Architecture overview](docs/architecture/overview.md) | HCI design, component diagram, data flows |
| [Hyperconvergence](docs/architecture/hyperconvergence.md) | What HCI means in Caimán OS, comparison with vSAN/Nutanix |
| [VMM internals](docs/architecture/vmm.md) | How caiman-vmm works without QEMU |
| [XDP networking](docs/architecture/networking.md) | caiman_net.ko, XDP < 8µs, micro-segmentation |
| [Live migration](docs/architecture/livemig.md) | Pre-copy algorithm, BPF map transfer, <200ms |
| [ISO Installation](docs/operations/iso.md) | Bootable ISO — flash to USB, TUI installer |
| [Installation guide](docs/operations/install.md) | Bare metal, VPS, PXE, iDRAC/BMC |
| [API reference](docs/api/rest.md) | All REST endpoints with examples |
| [Development setup](docs/development/setup.md) | Build from source, run tests |
| [Contributing](CONTRIBUTING.md) | How to contribute |
| [Security](SECURITY.md) | Vulnerability reporting |
| [Changelog](CHANGELOG.md) | Version history |

---

## Roadmap

- [x] v0.1–v0.6 · KVM VMM, serial, virtio-net, virtio-blk, REST API
- [x] v0.7.0 · Live migration pre-copy < 200ms
- [x] v0.8.0 · GPU passthrough + NVIDIA MIG + vGPU
- [x] v0.9.0 · VSAN distributed storage + vVols
- [x] **v1.0.0 · Production GA**
- [x] **v1.1.0 · Bootable ISO** ← current
- [ ] v1.2.0 · Multi-cluster federation
- [ ] v1.2.0 · Terraform provider + Ansible collection
- [ ] v1.3.0 · eBPF service mesh (no Envoy/Istio)
- [ ] v2.0.0 · Caimán Cloud (SaaS managed HCI)

---

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). All contributions welcome — from bug fixes to new storage backends and hardware support.

```bash
git clone https://github.com/teampmladv/caiman-os
cargo build --workspace
cd ui && npm install && npm run dev
```

---

## License

Apache 2.0 — see [LICENSE](LICENSE)

Enterprise features (multi-tenant, SSO, billing, SLA support) available under a commercial license. Contact us at [team@caimanos.com](mailto:team@caimanos.com).

---

<div align="center">

**Named after the Cuban crocodile 🐊 · Built for the cloud ☁️**

[caimanos.com](https://caimanos.com) · [GitHub](https://github.com/teampmladv/caiman-os) · [Releases](https://github.com/teampmladv/caiman-os/releases)

</div>
