# Hyperconvergence in Caimán OS

## What is HCI?

Hyperconverged Infrastructure (HCI) collapses three traditionally separate infrastructure layers into a single software platform running on commodity x86 hardware:

```
Traditional (3 separate systems)          HCI (1 platform)
────────────────────────────────          ────────────────
 [Compute]  [Storage]  [Network]    →     [Caimán OS]
 vSphere    NetApp      Cisco              single codebase
 QEMU       SAN        OVS                single team
 3 vendors  3 licenses  3 configs         $0 license
```

The key insight: compute, storage, and networking don't need to be physically separate. On modern NVMe servers with 100GbE NICs, you can run all three on the same machine — and at lower latency than traditional SAN/NAS because you eliminate multiple network hops.

---

## How Caimán OS implements HCI

### Compute layer — caiman-vmm

Unlike every other HCI solution on the market, Caimán's VMM talks directly to `/dev/kvm` without QEMU:

```
Traditional HCI stack:      Caimán OS:
────────────────────        ──────────
Guest VM                    Guest VM
    │                           │
  QEMU     250 MB/VM         caiman-vmm   1.8 MB/VM
    │                           │
  KVM                         KVM
    │                           │
Hardware                    Hardware
```

The VMM implements in Rust:
- **bzImage loader** — Linux x86 boot protocol, E820 memory map, zero page
- **virtio-net** — TAP datapath with split-ring virtqueue (no vm-memory crate)
- **virtio-blk** — raw disk image I/O via pread/pwrite
- **serial 16550A** — ttyS0 via kvm_run mmap (zero overhead console)
- **VFIO-PCI** — full GPU passthrough, direct IOMMU assignment

### Storage layer — caiman-storage (VSAN)

VSAN distributes VM disk images across the local NVMe drives of multiple cluster nodes:

```
Node 1 (2× NVMe)    Node 2 (2× NVMe)    Node 3 (2× NVMe)
┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
│ [Vol A] replica │  │ [Vol A] primary │  │ [Vol A] witness │
│ [Vol B] primary │  │ [Vol B] witness │  │ [Vol B] replica │
└────────┬────────┘  └────────┬────────┘  └────────┬────────┘
         └───────────── NVMe-oF over TCP ───────────┘
```

Replication policies:
- `FTT=0` — single copy (dev/test)
- `FTT=1` — 2 copies + witness (default, tolerates 1 node failure)
- `FTT=2` — 3 copies + witness (tolerates 2 node failures)
- `RAID-5/6` — erasure coding for capacity efficiency

Storage backends supported:
- **VSAN** — NVMe-oF TCP/RDMA between nodes (built-in)
- **iSCSI** — hardware arrays, software initiator
- **NVMe-oF** — NVMe fabric (highest performance)
- **NFS v4.1** — pNFS with parallel data paths
- **vVols** — storage-policy-based management

### Networking layer — caiman_net.ko + XDP

The kernel module `caiman_net.ko` attaches an XDP program to the uplink NIC. It routes VM traffic at XDP native hook before the kernel network stack — zero-copy, zero-overhead:

```
NIC hardware → XDP hook (caiman_net.ko) → TAP fd (VM interface)
                   │
                   ├── lookup mac_to_ifindex[dst_mac]  (~8µs)
                   ├── check policy_map[{src,dst}]     (~5µs micro-seg)
                   └── XDP_REDIRECT → zero-copy to TAP
```

Without Caimán's XDP module, traffic goes through:
```
NIC → driver → socket buffer → netfilter → bridge → TAP  (~100µs)
```

The 12× latency improvement comes entirely from bypassing the kernel network stack.

---

## HCI vs. alternatives

### vs. VMware vSAN + NSX-T + vSphere

vSAN/NSX-T/vSphere is the original commercial HCI stack. Caimán OS is its open-source replacement:

| Dimension | Caimán OS | VMware HCI |
|-----------|-----------|-----------|
| Annual license | $0 | $10K–$20K per socket |
| VMM overhead | 1.8 MB | ~250 MB (QEMU) |
| Network latency | 8µs | ~100µs |
| Micro-seg latency | 5µs | ~50µs (NSX-T) |
| Install time | 5 min | Days (vCenter, vSAN, NSX wizard) |
| Configuration | One YAML | Three separate products |

VMware's main advantage is a larger ecosystem and more hardware certifications. Caimán OS runs on any x86 server with a KVM-capable CPU.

### vs. Nutanix AHV

Nutanix invented the term "hyperconverged infrastructure" in 2011. Caimán OS is architecturally similar but open-source and significantly lighter:

| Dimension | Caimán OS | Nutanix AHV |
|-----------|-----------|------------|
| Annual license | $0 | $8K–$15K per socket |
| VMM | Direct KVM | QEMU/KVM |
| Storage | VSAN (NVMe-oF) | Nutanix DSF (iSCSI-based) |
| Network | XDP native | OVS + Flow |
| AI integration | MCP server | None |
| Source available | Apache 2.0 | Proprietary |

### vs. Proxmox VE

Proxmox is the closest open-source alternative — also free, also KVM-based. Key differences:

| Dimension | Caimán OS | Proxmox VE |
|-----------|-----------|-----------|
| VMM | Direct KVM (no QEMU) | QEMU/KVM |
| Distributed storage | VSAN built-in | Ceph (separate cluster) |
| Network latency | 8µs (XDP) | ~100µs (Linux bridge) |
| Resource scheduler | σ-balancer DRS | Basic (no auto-balance) |
| Live migration | <200ms | ~2–10s |
| AI integration | MCP server | None |

Proxmox is more mature (10+ years, large community). Caimán OS is faster and lighter, with automatic DRS balancing and AI integration.

---

## When to use Caimán OS

**Best fit:**
- Replacing VMware after Broadcom price increases
- GPU-intensive workloads (AI/ML inference clusters)
- Latency-sensitive applications (trading, real-time, gaming)
- Kubernetes-based infrastructure (native CNI + scheduler extender)
- Organizations wanting full control over their hypervisor stack

**Consider alternatives if:**
- You need VMware ecosystem compatibility (SRM, vRA, NSX-T integrations)
- Your hardware requires ESXi certified drivers
- You need FIPS 140-2 certification (coming in v1.2.0)
- You have a large existing Proxmox/Nutanix deployment (migration cost)

---

## Sizing guide

### Single node (development / small production)

```
CPU:     8+ cores (Intel Xeon or AMD EPYC)
RAM:     32 GiB minimum, 128 GiB recommended
Storage: 2× NVMe 2 TB (OS on 1st, VM data on 2nd)
Network: 1 GbE management + 10 GbE VM traffic
GPU:     optional (NVIDIA RTX/A-series for MIG/vGPU)
```

### 3-node cluster (VSAN FTT=1, full HA)

```
Each node:
  CPU:     16+ cores
  RAM:     128–256 GiB
  Storage: 1× NVMe (OS) + 2× NVMe 4 TB (VSAN data)
  Network: 1 GbE management + 25 GbE VSAN + 25 GbE VM

Capacity after FTT=1:
  Total raw: 3 nodes × 2 × 4 TB = 24 TB
  Usable:    ~12 TB (50% overhead for replication)
  VMs:       ~500–1000 VMs (512 MiB each)
```

### Storage calculation

```
VSAN effective capacity = (raw capacity × nodes) / (FTT + 1)

Example: 3 nodes × 8 TB raw, FTT=1
  Effective = (24 TB) / 2 = 12 TB usable
```
