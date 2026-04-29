<div align="center">

# 🐊 Caimán OS

**KVM hypervisor without QEMU. Named after the Cuban crocodile. Built for the cloud.**

[![CI](https://github.com/teampmladv/caiman-os/actions/workflows/ci.yml/badge.svg)](https://github.com/teampmladv/caiman-os/actions)
[![Release](https://img.shields.io/github/v/release/teampmladv/caiman-os)](https://github.com/teampmladv/caiman-os/releases)
[![License](https://img.shields.io/badge/license-Apache%202.0-green)](LICENSE)
[![Demo](https://img.shields.io/badge/demo-live-brightgreen)](https://caimanos.com)

[Live Demo](https://caimanos.com) · [Documentation](docs/) · [Install](#install) · [API Reference](docs/api/)

</div>

---

## What is Caimán OS?

Caimán OS is a production-grade KVM hypervisor stack that replaces VMware vSphere — **without QEMU**. Instead of the traditional KVM → QEMU → VM stack, Caimán speaks directly to `/dev/kvm` via ioctls in Rust.

```
Traditional stack:         Caimán OS:
┌─────────────┐           ┌─────────────┐
│  Guest VM   │           │  Guest VM   │
├─────────────┤           ├─────────────┤
│    QEMU     │  250MB    │ caiman-vmm  │  1.8MB
├─────────────┤           ├─────────────┤
│  KVM (kernel)│          │ KVM (kernel)│
└─────────────┘           └─────────────┘
```

### Performance vs vSphere

| Metric | Caimán OS | VMware vSphere | Improvement |
|--------|-----------|----------------|-------------|
| Network latency P50 | **8µs** | ~100µs | 12× faster |
| Micro-segmentation | **5µs** | ~50µs (NSX-T) | 10× faster |
| VMM memory per VM | **1.8 MB** | ~250 MB | 140× smaller |
| Live migration blackout | **<200ms** | 1–5s | 5–25× faster |
| License cost | **$0/year** | $4K–$10K/socket/year | ∞ cheaper |

---

## Install

```bash
curl -fsSL https://caimanos.com/install.sh | sudo bash
```

**Requirements:** x86_64, VT-x or AMD-V, CentOS 8+ / Ubuntu 22.04+ / Debian 12+

See [Installation Guide](docs/operations/install.md) for manual install, PXE boot, and iDRAC provisioning.

---

## Quick Start

```bash
# Create your first VM
curl -X POST http://localhost:8765/api/vms \
  -H 'Content-Type: application/json' \
  -d '{
    "name":    "web-01",
    "cpus":    2,
    "memMib":  512,
    "kernel":  "/var/lib/caiman/vmlinuz"
  }'

# List VMs
curl http://localhost:8765/api/vms | jq .

# Open the dashboard
open http://localhost:3000
```

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                         Caimán OS                                │
│                                                                  │
│  ┌─────────────┐  ┌─────────────┐  ┌──────────────────────────┐ │
│  │ caiman-ui   │  │ caiman-cli  │  │ caiman-mcp (AI/MCP)      │ │
│  │ React dash  │  │ Terminal    │  │ Claude integration        │ │
│  └──────┬──────┘  └──────┬──────┘  └────────────┬─────────────┘ │
│         └────────────────┼─────────────────────  │               │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │              caiman-api  (REST + WebSocket)               │    │
│  │  POST /api/vms  GET /api/cluster  WS /ws/metrics         │    │
│  └──────────────────────┬───────────────────────────────────┘    │
│         ┌───────────────┼──────────────────────┐                 │
│  ┌──────▼──────┐ ┌──────▼──────┐ ┌─────────────▼──────────────┐ │
│  │ caiman-vmm  │ │ caiman-drs  │ │ caiman-bts                  │ │
│  │ KVM direct  │ │ σ-balancer  │ │ Backup + Templates          │ │
│  │ no QEMU     │ │ K8s sched   │ │ Snapshots (COW)             │ │
│  └──────┬──────┘ └─────────────┘ └────────────────────────────┘ │
│         │                                                         │
│  ┌──────▼──────────────────────────────────────────────────┐     │
│  │        kernel/caiman_net.ko + XDP eBPF                  │     │
│  │   Network: <8µs  ·  Micro-seg: <5µs  ·  Zero-copy      │     │
│  └─────────────────────────────────────────────────────────┘     │
└──────────────────────────────────────────────────────────────────┘
```

### Components

| Component | Language | Description |
|-----------|----------|-------------|
| [`vmm/`](vmm/) | Rust | KVM VMM — direct ioctls, no QEMU |
| [`api/`](api/) | Rust | REST API + WebSocket + lifecycle management |
| [`cni/`](cni/) | Rust | CNI plugin — Calico/Cilium/Flannel compatible |
| [`drs/`](drs/) | Rust | Distributed Resource Scheduler (σ-balancer) |
| [`bts/`](bts/) | Rust | Backup, Templates & Snapshots |
| [`livemig/`](livemig/) | Rust | Live migration — pre-copy, <200ms blackout |
| [`mcp/`](mcp/) | Rust | Model Context Protocol server (AI integration) |
| [`kernel/`](kernel/) | C + eBPF | XDP kernel module — <8µs network latency |
| [`ui/`](ui/) | React + TS | Dashboard — better than Rancher, better than vSphere |
| [`cli/`](cli/) | Rust | Terminal CLI — `caiman vm list`, `caiman drs status` |

---

## Documentation

| Guide | Description |
|-------|-------------|
| [Architecture](docs/architecture/overview.md) | System design, data flows, KVM internals |
| [Installation](docs/operations/install.md) | Bare metal, VPS, PXE, iDRAC/BMC |
| [API Reference](docs/api/rest.md) | All REST endpoints with examples |
| [VMM Internals](docs/architecture/vmm.md) | How caiman-vmm works without QEMU |
| [XDP Networking](docs/architecture/networking.md) | caiman_net.ko, micro-segmentation |
| [Live Migration](docs/architecture/livemig.md) | Pre-copy algorithm, BPF map transfer |
| [DRS](docs/architecture/drs.md) | σ-balancer, K8s scheduler extender |
| [Development](docs/development/setup.md) | Build from source, run tests |
| [Contributing](CONTRIBUTING.md) | How to contribute |

---

## OCI Images

All images available at `ghcr.io/teampmladv/`:

```bash
docker pull ghcr.io/teampmladv/caiman-api:0.7.0    # 2.2MB
docker pull ghcr.io/teampmladv/caiman-vmm:0.7.0    # 1.8MB
docker pull ghcr.io/teampmladv/caiman-cni:0.7.0    # 1.1MB
docker pull ghcr.io/teampmladv/caiman-drs:0.7.0    # 4.5MB
docker pull ghcr.io/teampmladv/caiman-bts:0.7.0    # 332KB
docker pull ghcr.io/teampmladv/caiman-mcp:0.7.0    # 1.1MB
docker pull ghcr.io/teampmladv/caiman-ui:0.7.0     # 552KB
```

---

## Stack (docker-compose)

```bash
git clone https://github.com/teampmladv/caiman-os
cd caiman-os
docker compose up -d
```

Services:
- **Dashboard:** http://localhost:3000
- **API:** http://localhost:8765
- **Grafana:** http://localhost:3001 (admin/caiman)

---

## Roadmap

- [x] v0.1.0 — KVM open, VM created
- [x] v0.2.0 — bzImage loader, guest memory, vCPU KVM_RUN loop
- [x] v0.3.0 — Serial console ttyS0 (boot log visible)
- [x] v0.4.0 — virtio-net + TAP dataplane
- [x] v0.5.0 — Full REST API + VM lifecycle
- [x] v0.6.0 — virtio-blk (disk)
- [x] v0.7.0 — Live migration pre-copy <200ms
- [ ] v0.8.0 — GPU passthrough + NVIDIA MIG
- [ ] v0.9.0 — Multi-node cluster + VSAN
- [ ] v1.0.0 — Production GA

---

## License

Apache 2.0 — see [LICENSE](LICENSE)

---

<div align="center">

**Named after the Cuban crocodile 🐊 · Built for the cloud ☁️**

[caimanos.com](https://caimanos.com) · [GitHub](https://github.com/teampmladv/caiman-os)

</div>
