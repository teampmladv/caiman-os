# Architecture Overview

## Design principles

Caimán OS is built around three core principles:

1. **No QEMU** — Direct KVM ioctls in Rust. `caiman-vmm` is 1.8MB vs QEMU's 250MB.
2. **XDP everywhere** — All networking goes through `caiman_net.ko`, an XDP program that achieves <8µs latency by bypassing the kernel network stack.
3. **API-first** — Everything is controlled via REST. The dashboard, CLI, and DRS scheduler all use the same API.

---

## Component diagram

```
┌─────────────────────────────────────────────────────────────┐
│  User interfaces                                            │
│  ┌──────────────┐  ┌───────────┐  ┌────────────────────┐   │
│  │  caiman-ui   │  │ caiman-cli│  │ caiman-mcp (Claude)│   │
│  │  :3000       │  │ terminal  │  │ :8767              │   │
│  └──────┬───────┘  └─────┬─────┘  └──────────┬─────────┘   │
└─────────┼────────────────┼─────────────────── ┼─────────────┘
          └────────────────┼────────────────────┘
                           │ HTTP + WebSocket
┌──────────────────────────▼──────────────────────────────────┐
│  caiman-api  :8765                                          │
│                                                             │
│  POST   /api/vms            spawn caiman-vmm process        │
│  GET    /api/vms            read /var/run/caiman/*.json      │
│  GET    /api/nodes          real metrics from /proc          │
│  GET    /api/cluster        nodes + VMs + sigma             │
│  POST   /api/vms/:id/stop   SIGTERM to caiman-vmm           │
│  POST   /api/vms/:id/migrate  spawn caiman-livemig          │
└──────────────────────────┬──────────────────────────────────┘
                           │ spawn processes
          ┌────────────────┼────────────────────┐
          │                │                    │
┌─────────▼──────┐ ┌───────▼──────┐ ┌──────────▼─────┐
│  caiman-vmm    │ │ caiman-drs   │ │ caiman-bts     │
│                │ │ :8766        │ │ :8768          │
│  /dev/kvm      │ │              │ │                │
│  GuestMemory   │ │  σ-balancer  │ │  Restic backup │
│  vCPU loop     │ │  K8s sched   │ │  COW snapshots │
│  virtio-net    │ │  extender    │ │  Tera templates│
│  virtio-blk    │ └──────────────┘ └────────────────┘
│  serial ttyS0  │
└───────┬────────┘
        │ /dev/kvm ioctls
┌───────▼────────────────────────────────────────────────────┐
│  Linux kernel                                              │
│  ┌────────────────────┐  ┌─────────────────────────────┐  │
│  │  KVM subsystem     │  │  caiman_net.ko + XDP eBPF   │  │
│  │  KVM_CREATE_VM     │  │                             │  │
│  │  KVM_RUN           │  │  xdp_vm_router.o            │  │
│  │  irqchip + PIT2    │  │  mac → ifindex map          │  │
│  │  dirty page ring   │  │  micro-seg policy engine    │  │
│  └────────────────────┘  └─────────────────────────────┘  │
└────────────────────────────────────────────────────────────┘
```

---

## caiman-vmm internals

The VMM speaks directly to `/dev/kvm` without any emulation layer:

### Boot sequence

```
caiman-vmm --kernel /boot/vmlinuz --mem-mib 512 --cpus 2
    │
    ├── 1. GuestMemory::new()
    │      mmap(512MiB, PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS)
    │      KVM_SET_USER_MEMORY_REGION → registers with kernel
    │
    ├── 2. load_bzimage()
    │      parse boot header (offset 0x1F1)
    │      copy kernel to guest address 0x100000 (1 MiB)
    │      build E820 memory map
    │      write zero page (boot_params) at 0x7000
    │      write GDT at 0x5000
    │      write cmdline at 0x20000
    │
    ├── 3. Vm::new()
    │      KVM_CREATE_IRQCHIP
    │      KVM_CREATE_PIT2
    │
    ├── 4. Vcpu::new() × N
    │      KVM_CREATE_VCPU
    │      set CPUID (hypervisor bit)
    │      set MSRs (SYSENTER_CS/ESP/EIP)
    │      set registers (rip=entry, rsi=boot_params)
    │      set sregs (CR0=PE, CR4=PAE, EFER=LME)
    │      mmap kvm_run at vcpu_fd offset 0
    │
    ├── 5. KVM_RUN loop (per vCPU thread)
    │      VcpuExit::Io     → read port/data from kvm_run mmap → Serial
    │      VcpuExit::Mmio   → route to virtio-net or virtio-blk
    │      VcpuExit::Hlt    → sleep 100µs
    │      VcpuExit::Shutdown → exit thread
    │
    ├── 6. virtio-net dataplane thread
    │      Tap::new("tap0") → /dev/net/tun
    │      loop: TX queue → tap.send() / tap.recv() → RX queue → irqfd
    │
    └── 7. virtio-blk dataplane thread
           file.read_at(offset) / file.write_at(offset)
           per request: header(16B) + data + status(1B)
```

### Memory layout (guest physical)

```
0x0000_0000  reset vector, BDA
0x0000_5000  GDT (4 entries: null, code32, data32, code64)
0x0000_7000  boot_params (zero page, E820, cmdline ptr)
0x0002_0000  kernel cmdline
0x0009_F000  end of conventional RAM (640 KiB)
0x000A_0000  VGA/ROM (reserved, not mapped)
0x0010_0000  kernel image (1 MiB = load address)
0x????_????  initrd (optional)
0x3000_0000  initrd max address
```

---

## XDP networking (caiman_net.ko)

The kernel module `caiman_net.ko` attaches an XDP program to the uplink interface. It maintains a BPF hash map (`mac_to_ifindex`) that routes packets directly to the TAP interface of the destination VM — bypassing the kernel bridge and achieving <8µs latency.

```
Incoming packet → NIC → XDP program
                         │
                         ├── lookup mac_to_ifindex[dst_mac]
                         │
                         ├── if found → XDP_REDIRECT to TAP fd
                         │   (zero-copy, ~8µs)
                         │
                         └── if not found → XDP_PASS
                             (normal kernel routing, ~100µs)
```

Micro-segmentation policy is enforced at the same XDP hook, checking `policy_map[{src_vm, dst_vm}]` before redirect. Policy violations are counted in `deny_stats` and exposed via `/sys/module/caiman_net/stats/`.

---

## Live migration (caiman-livemig)

Pre-copy algorithm with 3 phases:

```
Phase 1: Pre-copy (VM running)
  ─ Enable KVM_MEM_LOG_DIRTY_PAGES
  ─ Iterate: GET dirty log → send pages over TCP :7777
  ─ Until: dirty_pages < threshold OR iterations > 5

Phase 2: Stop-and-copy (VM paused, <200ms)
  ─ Pause vCPUs (SIGSTOP to caiman-vmm)
  ─ Copy remaining dirty pages
  ─ Transfer vCPU register state (kvm_regs + kvm_sregs)
  ─ Send DONE to destination

Phase 3: Switchover
  ─ Destination starts vCPUs
  ─ Source sends gratuitous ARP
  ─ Update caiman_net BPF mac_to_ifindex map
  ─ Delete source VM
```

Wire protocol (TCP port 7777):
```
[4B type][4B len][len bytes payload]

Types: HELLO(1) READY(2) PAGE(3) PAUSE(4) VCPU_STATE(5) DONE(6) RUNNING(7)
PAGE payload: [8B GPA][4096B data]
```

---

## DRS (Distributed Resource Scheduler)

The σ-balancer measures cluster imbalance using the standard deviation of per-node load scores:

```
load_score(node) = cpu_pct × 0.6 + mem_pct × 0.4

σ = std_dev([load_score(n) for n in cluster])

if σ > threshold (default 0.10):
    recommend migration of heaviest VM on most loaded node
    → least loaded node
```

Recommendations are generated every 30 seconds and exposed at `GET /api/drs/recommendations`. In `FullyAutomated` mode, migrations are executed automatically.

The DRS also implements the Kubernetes scheduler extender protocol — kubelets ask caiman-drs where to place new VM pods based on current cluster load.
