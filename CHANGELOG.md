# Changelog

All notable changes to Caimán OS are documented here.

---

## [1.1.0] — 2026-05-01 — Bootable ISO

### 🐊 First bootable ISO release

Install Caimán OS on bare metal with a single USB drive — like ESXi, but 6× smaller and $0 license.

#### What's new
- `caiman-os-1.1.0-x86_64.iso` — 58 MB bootable ISO (ESXi: 350MB, Proxmox: 1.2GB)
- `iso/installer/caiman-install.sh` — TUI installer with hardware detection
- `iso/grub/grub.cfg` — GRUB2 EFI boot menu (Install / Live / Debug)
- `iso/scripts/build-iso.sh` — Alpine-based ISO builder
- `.github/workflows/build-iso.yml` — Automated ISO build on release

#### ISO contents
- Alpine Linux 3.19 base (musl libc, BusyBox)
- All Caimán OS binaries (caiman-vmm, caiman-api, caiman-drs, ...)
- Docker + docker-compose pre-installed
- nginx pre-installed
- OpenRC init system
- GRUB2 EFI bootloader

#### Installer features
- CPU virtualization detection (VT-x / AMD-V)
- Disk selection with sizes and models
- DHCP or static IP configuration
- Standalone or cluster join mode
- Automatic partitioning (GPT + EFI)
- GRUB2 installation

---

## [1.0.0] — 2026-04-29 — Production GA

### 🎉 First stable release

**Caimán OS is production-ready.** Named after the Cuban crocodile. Built for the cloud.

#### Components
- **caiman-vmm** — KVM VMM without QEMU. Direct ioctls in Rust. 1.8MB binary.
- **caiman-api** — Full REST API + WebSocket. VM lifecycle, real /proc metrics.
- **caiman-cni** — CNI plugin compatible with Calico, Cilium, Flannel.
- **caiman-drs** — σ-balancer + Kubernetes scheduler extender.
- **caiman-bts** — Backup (Restic), Templates (COW), Snapshots.
- **caiman-livemig** — Pre-copy live migration < 200ms blackout.
- **caiman-storage** — VSAN distributed storage + vVols (iSCSI/NVMe-oF/NFS).
- **caiman-gpu** — NVIDIA GPU passthrough, MIG slices, vGPU via mdev.
- **caiman-mcp** — Model Context Protocol server (AI integration).
- **caiman** — Full-featured terminal CLI.
- **caiman-ui** — React dashboard.
- **kernel/caiman_net.ko** — XDP kernel module, < 8µs network latency.

---

## [0.9.0] — 2026-04-29

### Added
- `caiman-storage` — VSAN distributed block storage daemon (port 8770)
- Volume CRUD via REST API
- Local disk discovery via `/sys/block`
- Sparse volume creation (zero-cost until written)
- VSAN cluster status, node info, disk inventory
- `vvols/` module — iSCSI, NVMe-oF, NFS backends (stub)

---

## [0.8.0] — 2026-04-29

### Added
- `caiman-gpu` — GPU management daemon (port 8769)
- VFIO-PCI full passthrough (`gpu/src/passthrough.rs`)
- NVIDIA MIG slice management (`gpu/src/mig.rs`)
  - Enable/disable MIG mode
  - Create/destroy GPU + Compute instances
  - Profile listing (1g.10gb, 3g.40gb, 7g.80gb, etc.)
- NVIDIA vGPU via mdev (`gpu/src/vgpu.rs`)
  - List available profiles from `/sys/bus/pci/.../mdev_supported_types`
  - Create/remove mediated devices
- GPU inventory via `nvidia-smi`
- REST endpoints: `/api/gpu`, `/api/gpu/summary`, `/api/gpu/:pci/mig/profiles`

---

## [0.7.0] — 2026-04-29

### Added
- `caiman-livemig` — Live migration with pre-copy algorithm
- Binary TCP protocol (port 7777): HELLO → READY → PAGE → VCPU_STATE → DONE → RUNNING
- Dirty page tracking via `KVM_MEM_LOG_DIRTY_PAGES`
- BPF map transfer (`bpf_migrate.rs`)
- Network switchover with gratuitous ARP
- `POST /api/vms/:id/migrate` endpoint

---

## [0.6.0] — 2026-04-29

### Added
- `virtio/blk.rs` — Full virtio-blk MMIO device
- BlkState MMIO registers (REG_CONFIG, capacity in sectors)
- `blk_dataplane` thread — `file.read_at` / `file.write_at`
- FLUSH, GET_ID request types
- `--disk path.img` flag in caiman-vmm
- virtio-blk wired into vcpu.rs MMIO dispatch

---

## [0.5.0] — 2026-04-29

### Added
- Full REST API (caiman-api v0.5.0)
- `vm/state.rs` — persistent VM state via `/var/run/caiman/*.json`
- `vm/runner.rs` — spawn/kill caiman-vmm processes
- `node/metrics.rs` — real CPU/RAM/disk/net from `/proc` via sysinfo
- VM lifecycle: POST /api/vms, stop, force-stop, delete, console logs
- Reconcile: auto-mark STOPPED if caiman-vmm process dies
- DEMO_MODE=true — in-memory simulation for Railway/cloud deploys

---

## [0.4.0] — 2026-04-29

### Added
- `virtio/net.rs` — Full virtio-net MMIO + TAP dataplane
- `virtio/queue.rs` — Custom split-ring virtqueue (no vm-memory crate)
- `virtio/tap.rs` — Linux TUN/TAP interface via TUNSETIFF ioctl
- TX queue → tap.send() / tap.recv() → RX queue → irqfd injection
- `--tap` and `--tap-ip` flags in caiman-vmm
- MMIO routing in vcpu.rs for virtio-net at 0xD000_0000

---

## [0.3.0] — 2026-04-29

### Added
- Serial console (16550A ttyS0) via kvm_run mmap
- `KvmRunPtr` — mmaps vcpu fd at offset 0, reads io.port / io.data_offset
- Port 0x3F8 (COM1 TX) → Serial::write_port() → stdout
- `--cmdline "console=ttyS0,115200"` shows boot log in terminal
- ACPI reboot port 0x604 handling

---

## [0.2.0] — 2026-04-29

### Added
- `kvm/loader.rs` — Linux x86 boot protocol (bzImage)
  - Boot header parsing, E820 memory map, zero page (boot_params)
  - GDT setup, cmdline, initrd support
- `kvm/memory.rs` — GuestMemory via mmap + KVM_SET_USER_MEMORY_REGION
- `kvm/vcpu.rs` — vCPU KVM_RUN loop
  - CPUID configuration (hypervisor bit, KVM signature)
  - MSR setup, register and segment configuration
  - VcpuExit handlers: Io, MmioRead, MmioWrite, Hlt, Shutdown
- `kvm/vm.rs` — KVM_CREATE_IRQCHIP + KVM_CREATE_PIT2
- Kernel boot visible in caiman-vmm stdout

---

## [0.1.0] — 2026-04-29

### Added
- Initial monorepo structure
- `caiman-vmm` — opens `/dev/kvm`, creates VM, KVM API version check
- `caiman-api` — minimal Axum server, `/health` endpoint
- `caiman-cni` — CNI plugin stub (ADD/DEL/CHECK/VERSION)
- `caiman-drs` — DRS scheduler with σ-balancer
- `caiman-bts` — Backup, Templates & Snapshots server
- `caiman-mcp` — MCP server (port 8767)
- `caiman-ui` — React dashboard (static build via nginx)
- `kernel/caiman_net/` — XDP kernel module (C + eBPF)
- `install/scripts/install.sh` — one-command installer
- `docker-compose.yml` — full production stack
- GitHub Actions CI + Release workflows
- Apache 2.0 license

---

## Version policy

Caimán OS follows [Semantic Versioning](https://semver.org/):

- **MAJOR** — breaking API changes
- **MINOR** — new features, backward compatible
- **PATCH** — bug fixes, backward compatible

From v1.0.0 onward, the REST API is stable and will not break between minor versions.
