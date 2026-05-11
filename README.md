# 🐊 Caimán OS

**Open-source hyperconverged infrastructure without QEMU.**

[![CI](https://github.com/teampmladv/caiman-os/actions/workflows/ci.yml/badge.svg)](https://github.com/teampmladv/caiman-os/actions)
![Status](https://img.shields.io/badge/status-alpha-orange)
[![License](https://img.shields.io/badge/license-Apache%202.0-22c55e)](LICENSE)
[![Demo](https://img.shields.io/badge/demo-live-22c55e)](https://caimanos.com)

[**Live Demo**](https://caimanos.com) · [**Documentation**](./docs) · [**Install**](#install)

---

> **Project status: Alpha (v0.x).**
> The VMM, REST API, and UI are functional and run real VMs without QEMU.
> Higher-level subsystems (distributed storage, live migration, DRS, GPU,
> micro-segmentation, backup) are in active development — most exist today
> as designed scaffolding (types, REST routes, algorithm sketches) and need
> further implementation before reaching the performance targets below.
> Pre-1.0 = breaking changes expected.

---

## What is Caimán OS?

Caimán OS is a **hyperconverged infrastructure (HCI) platform under
active development** that aims to collapse compute, storage, and networking
into a single Rust codebase running on commodity x86 hardware — without
QEMU, without a SAN, without a separate network controller.

The key technical bet is **no QEMU**: Caimán's VMM speaks directly to
`/dev/kvm` via ioctls in Rust. The current VMM is ~5K lines of Rust and
boots Linux 6.6 on AMD bare metal in ~1.3 seconds. This is the part that
exists and works today.

---

## What works today (v0.x alpha)

| Component | State | Notes |
|---|---|---|
| **caiman-vmm** (KVM without QEMU) | works | Linux 6.6 boots in ~1.3s; virtio-blk + virtio-net + serial console |
| **caiman-api** (REST + JWT + WebSocket) | works | Single-node lifecycle: create / start / stop / delete / console |
| **caiman-ui** (React dashboard) | works | VM management, interactive xterm console, import wizard |
| **caiman-cni** (NAT + bridge) | works | Per-VM TAP, basic NAT to host |
| **Import wizard** | partial | UI supports 9 sources; Proxmox / vSphere / oVirt / Nutanix / Oracle backends call real APIs to discover. Disk conversion is stubbed. |
| **caiman-drs** | scaffolding | sigma-balancer algorithm + Kubernetes extender protocol in place; needs multi-node testing |
| **caiman-storage** | scaffolding | VSAN / vVols types + REST API defined; replication and data path not implemented |
| **caiman-livemig** | scaffolding | Pre-copy protocol designed; dirty-page tracking and full transfer not yet implemented |
| **caiman-gpu** | partial | VFIO passthrough flow present; NVIDIA MIG / vGPU code is skeleton |
| **caiman-microseg** | partial | Policy compiler (labels -> BPF map entries); BPF program not yet attached in production |
| **caiman-bts** | scaffolding | Snapshot / backup / template REST routes defined; storage backend not implemented |
| **caiman_net.ko** (XDP) | skeleton | Module structure in place; XDP program not finalized |
| **Live migration < 200ms** | target | Goal — not measured yet |
| **8us network latency** | target | Goal — not measured yet |
| **Bootable ISO** | in progress | Earlier ISOs published; current rebuild for v0.x in progress |

works = functional today · partial = partially working · scaffolding = designed but not implemented · skeleton = stub · target = stated goal

---

## Why no QEMU?

QEMU is a brilliant emulator covering 30+ architectures, dozens of devices,
and decades of compatibility. Caimán does not need most of that. We support:

- One architecture (x86_64)
- Modern KVM-only acceleration (no software emulation fallback)
- A small fixed set of devices (16550 UART, virtio-blk, virtio-net)

The result is a VMM in roughly 5,000 lines of Rust instead of hundreds of
thousands of lines of C. The trade-off is intentional: less compatibility,
more auditability and a smaller attack surface.

---

## Performance targets (not yet measured at scale)

Stated targets the project is built toward. Real benchmarks will be
published as features land and stabilize.

| Metric | Target | Status |
|---|---|---|
| VM boot time (Alpine) | < 500ms | ~1.3s measured today |
| VMM binary size | < 5 MB | 1.8 MB in earlier builds; current binary larger |
| Network latency (XDP) | < 10us | Goal — XDP module not yet finalized |
| Live migration blackout | < 1s | Goal — pre-copy not yet implemented end-to-end |
| Micro-segmentation overhead | < 10us | Goal — BPF program not yet attached |

Please do not quote these as benchmarks — they are stated design goals.

---

## Comparison vs. the market

Honest version. Caimán is early-stage; established platforms are mature,
battle-tested, and have ecosystems we do not have yet.

|  | **Caimán OS (alpha)** | Proxmox VE | vSphere | Nutanix AHV | Harvester |
|---|---|---|---|---|---|
| Stability | alpha | GA, ~15 years | GA, ~20 years | GA, ~10 years | GA, ~3 years |
| Distributed storage | planned | external Ceph | vSAN | DSF | Longhorn |
| Live migration | planned | yes | yes (vMotion) | yes | yes |
| HA / auto-failover | planned | yes | yes | yes | yes |
| Backup / DR | planned | PBS | SRM | Mine | external |
| GPU MIG / vGPU | partial | passthrough | yes | yes | passthrough |
| Multi-tenant / SSO | planned | LDAP / SAML | AD / SAML | AD | OIDC |
| Open source | Apache 2.0 | GNU AGPL | proprietary | proprietary | Apache 2.0 |
| No QEMU | **yes** | no | n/a | no | no |
| Single Rust codebase | **yes** | mixed | proprietary | proprietary | Go + k8s |

The "no QEMU, single Rust codebase" column is what makes Caimán worth
building. Everything else needs to be earned through implementation.

---

## Install

> Alpha software — not for production. Run on a test host.

```bash
curl -fsSL https://caimanos.com/install.sh | sudo bash
```

**Requirements:** x86_64 · VT-x / AMD-V · CentOS 8+ / Ubuntu 22.04+ /
Debian 12+ · 4 GiB RAM minimum.

See [docs/operations/install.md](./docs/operations/install.md) for manual
install.

---

## Quick start

```bash
# Get an auth token (default admin/admin123 -- change this!)
TOKEN=$(curl -s -X POST http://localhost:8765/auth/token \
  -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}' | jq -r .token)

# Create a VM
curl -X POST http://localhost:8765/api/vms \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"name":"web-01","cpus":1,"memMib":256}'

# Open the dashboard
open http://localhost:3000
```

---

## Components

| Component | State | Description |
|---|---|---|
| [vmm/](./vmm) | works | KVM VMM without QEMU |
| [api/](./api) | works | REST API + WebSocket + JWT |
| [ui/](./ui) | works | React dashboard, xterm console |
| [cni/](./cni) | works | NAT + bridge networking |
| [drs/](./drs) | scaffolding | DRS scheduler (k8s extender) |
| [storage/](./storage) | scaffolding | VSAN + vVols (designed, not implemented) |
| [livemig/](./livemig) | scaffolding | Pre-copy live migration (designed) |
| [gpu/](./gpu) | partial | VFIO passthrough; MIG / vGPU stubs |
| [microseg/](./microseg) | partial | Policy compiler; BPF program stub |
| [bts/](./bts) | scaffolding | Backup + Templates + Snapshots |
| [mcp/](./mcp) | stub | AI / MCP server |
| [cli/](./cli) | partial | Terminal CLI |
| [kernel/](./kernel) | skeleton | XDP module (caiman_net.ko) |

---

## Roadmap

- [x] **v0.1–v0.6** · KVM VMM, virtio-net / blk, REST API, console
- [x] **v0.7** · Web UI with interactive xterm console
- [x] **v0.8** · Import wizard (Proxmox / vSphere / oVirt / Nutanix / Oracle / OpenStack / Harvester / libvirt / AWS)
- [x] **v0.9 (current)** · Authentication, VM lifecycle, single-node production-ready
- [ ] **v1.0** · LVM thin-pool local storage with snapshots; PAM auth; ISO rebuild
- [ ] **v1.1** · Cluster federation (3+ nodes, gossip + Raft consensus)
- [ ] **v1.2** · Caimán Storage (vSAN-style distributed block storage)
- [ ] **v1.3** · Live migration end-to-end (pre-copy, < 1s blackout)
- [ ] **v1.4** · HA / auto-failover · DRS active across nodes
- [ ] **v1.5** · XDP networking finalized · micro-segmentation enforcing
- [ ] **v2.0** · First stable release · production claims with measured benchmarks

---

## Contributing

This is an ambitious, early-stage project and contributions are very
welcome — especially in storage, networking, and live migration.

See [CONTRIBUTING.md](./CONTRIBUTING.md). Start by reading the relevant
component README to understand current state vs. designed state.

```bash
git clone https://github.com/teampmladv/caiman-os
cargo build --workspace
cd ui && npm install && npm run dev
```

---

## License

Apache 2.0 — see [LICENSE](./LICENSE)

---

**Named after the Cuban crocodile 🐊**

[caimanos.com](https://caimanos.com) · [GitHub](https://github.com/teampmladv/caiman-os)
