# 🐊 Caimán OS

> *Born in Cuba. Built for the cloud.*

**Caimán OS** is a minimal bootable hypervisor operating system that replaces VMware vSphere with a kernel-native stack — no QEMU, no vhost-net, no license fees.

Named after the **Cuban crocodile** (*Crocodylus rhombifer*) — endemic exclusively to Cuba, one of the most resilient and territorial reptiles on the planet, and the reason the island itself is called *"el caimán verde"*.

---

## What Caimán replaces

| VMware component | Caimán equivalent |
|---|---|
| ESXi hypervisor | Linux 6.6 + KVM (in-kernel IRQ + PIT) |
| QEMU process model | `caiman-vmm` — minimal Rust VMM |
| vHost-net | XDP/eBPF zero-copy datapath |
| NSX-T micro-segmentation | `caiman-microseg` — XDP enforcement < 5µs |
| vSphere CNI | `caiman-cni` — Calico · Cilium · Flannel · Antrea · SR-IOV · Weave |
| DRS scheduler | `caiman-drs` — σ-based rebalancer + K8s scheduler extender |
| vMotion | `caiman-livemig` — pre-copy + stop-and-copy < 200ms blackout |
| VSAN | Distributed block storage, FTT=1/2, RAID-5/6, NVMe-oF |
| vVols | iSCSI · NVMe-oF · NFS v4.2 · FC · SMB3 |
| vGPU / MIG | NVIDIA MIG slices + vGPU + full passthrough |
| vCenter | `caiman-mcp` — MCP server for Claude / AI management |

---

## Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        Caimán OS — Boot                          │
│  GRUB (BIOS+UEFI) → Linux 6.6 → systemd → caiman-init.service  │
└────────────────────────────┬─────────────────────────────────────┘
         ┌──────────────────┬┴─────────────────┐
┌────────▼───────┐  ┌───────▼──────────┐  ┌────▼──────────────┐
│  caiman-vmm    │  │  caiman_net.ko   │  │  caiman-microseg  │
│  (Rust VMM)    │◄►│  (kernel module) │  │  (XDP zero-trust) │
│  /dev/kvm      │  │  Netlink + XDP   │  │  < 5µs enforce    │
└────────────────┘  └──────────────────┘  └───────────────────┘
                              │
┌─────────────────────────────▼────────────────────────────────────┐
│                    Kubernetes (kubeadm + containerd)              │
│  caiman-cni  caiman-drs  caiman-mcp  caiman-livemig  caiman-gpu  │
└──────────────────────────────────────────────────────────────────┘
```

---

## Repository layout

```
caiman-os/
├── kernel/
│   ├── caiman_net/         Kernel module C: netlink + XDP datapath
│   └── ebpf/               XDP programs: routing + micro-segmentation
├── vmm/                    Minimal KVM VMM (Rust) — no QEMU
├── cni/                    CNI plugin + adapters for all ecosystems
├── microseg/               XDP micro-segmentation + MicroSegPolicy CRD
├── drs/                    Distributed Resource Scheduler
├── storage/                VSAN + vVols (iSCSI/NVMe-oF/NFS/FC)
├── livemig/                Live migration (vMotion equivalent)
├── gpu/                    NVIDIA MIG + vGPU + passthrough
├── caiman-mcp/             MCP server for AI-driven management
├── buildroot/              OS build system → bootable ISO
├── kernel/caiman_kernel_defconfig  Linux 6.6 defconfig
├── k8s/                    kubeadm config + Kubernetes manifests
└── .github/workflows/      CI: build · test · publish · release · security
```

---

## Quickstart

```bash
# Build the OS image (~2h first run)
make setup && make iso

# Flash to bare metal
dd if=caiman.iso of=/dev/sdX bs=4M status=progress && sync

# Bootstrap Kubernetes cluster
sudo kubeadm init --config k8s/kubeadm-config.yaml
kubectl apply -f k8s/

# Start a VM
sudo caiman-vmm --kernel /var/lib/caiman/vmlinux --mem-mib 512 --cpus 2

# Connect to Claude Desktop (MCP)
# Add to claude_desktop_config.json:
# "caiman": { "command": "/usr/local/bin/caiman-mcp" }
```

---

## Performance vs VMware vSphere

| Metric | vSphere | Caimán OS |
|---|---|---|
| Network latency P50 | ~100 µs | **~8 µs** |
| Network latency P99 | ~400 µs | **~40 µs** |
| Throughput | ~10 Gbps | **~40–100 Gbps** |
| Copies per RX packet | 3–4 | **0** (zero-copy) |
| VMM RAM / VM | ~250 MB | **~10 MB** |
| Micro-seg latency | ~50 µs (NSX-T) | **~5 µs** (XDP) |
| License cost | $4K–$10K/socket/yr | **$0** |

---

## License

Rust crates: **Apache-2.0** · Kernel module + eBPF: **GPL-2.0-only**

---

*El caimán cubano no necesita permiso para ser el depredador ápice de su ecosistema.*
