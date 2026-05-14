# Caiman OS — Architecture

> This document describes the architecture of Caiman OS — what it is today,
> what it is becoming, and the principles that guide every technical
> decision. If you are a new developer, a potential co-founder, an
> investor, or a partner, this is the one document you need to read.
>
>
> Expected read time: 20 minutes.

---

## 1. What Caiman Is

Caiman OS is an open-source hyperconverged infrastructure (HCI) platform
built natively in Rust. It unifies compute, storage, and networking into a
single coherent stack designed to run on commodity x86_64 hardware —
without QEMU, without a SAN, and without a separate network controller.

The defining technical choice is no QEMU. Caiman's virtual machine
monitor (VMM) speaks directly to the Linux KVM subsystem via ioctl(2)
in Rust. The current VMM is approximately 5,000 lines of Rust and boots
Linux 6.6 on AMD bare metal in ~1.3 seconds.

Caiman is built with three audiences in mind, in this order:

1. Self-hosters and homelab operators who want a modern, fast, open
   alternative to existing virtualization platforms.

2. Small and medium businesses (SMB) who need HCI but cannot or do
   not want to pay enterprise licensing.

3. Enterprise teams seeking a path away from legacy stacks, with
   migration assisted natively by the platform.

---

## 2. What Caiman Is Not

Defining what we are not is as important as defining what we are. The
following are explicit non-goals. Contributions that move the project
toward these are not accepted.

- Caiman is not Kubernetes. We do not compete with k8s. Kubernetes
  can run on top of Caiman VMs; Caiman does not orchestrate containers.

- Caiman is not OpenStack. OpenStack offers vast API surface and
  enormous configurability at the cost of operational complexity.
  Caiman deliberately exposes a smaller, opinionated surface.

- Caiman is not a public cloud. Caiman runs on hardware the operator
  controls. We do not aim to replace AWS, GCP, or Azure.

- Caiman is not for ultra-small edge devices. Minimum supported
  hardware is x86_64 with KVM, IOMMU, and at least 4 GiB of RAM. We do
  not target Raspberry Pi or similar.

- Caiman is not a bare-metal provisioning system. We assume a Linux
  host is already installed. We do not compete with MaaS, Tinkerbell,
  or Foreman.

- Caiman does not emulate non-native architectures. KVM acceleration
  is required. We do not provide software emulation. No ARM-on-x86.

- Caiman is not a hypervisor for Xen or Hyper-V. KVM only.

---

## 3. Design Principles

The following principles are non-negotiable. They guide every architectural
decision and every code review.

### 3.1 Rust everywhere on the backend

All backend components (VMM, API, storage, networking, scheduler,
migration, MCP, CLI) are written in Rust. No Go, no Python, no C++ except
where unavoidable (kernel module is C, BPF programs are C). The reasons
are deliberate:

- Memory safety in a privileged stack that talks directly to KVM ioctls
  and runs as root.

- A single language across the codebase reduces operational complexity
  and onboarding time.

- Static binaries (musl) ship with no runtime dependencies.
- The Rust ecosystem for async I/O (tokio), HTTP servers (axum),
  and KVM bindings is mature enough for production systems.

### 3.2 TypeScript and React for the UI

The user-facing dashboard (caiman-ui legacy and ui current) is built
with TypeScript, React, and Tailwind CSS. The marketing site (website/)
is static HTML/CSS/JS. The CLI (cli/) is Rust.

### 3.3 KVM only, Linux only, x86_64 first

- KVM is the only supported hypervisor backend. The VMM is built around
  KVM ioctls; it does not have an abstraction layer that allows
  swapping in Xen, Hyper-V, or others.

- Linux is the only supported host operating system. Kernel 5.15+ is
  required; Kernel 6.x is recommended.

- x86_64 is the target architecture for v1.x. ARM64 (aarch64) is
  planned for v2.x but is not a current priority.

### 3.4 Single source of truth: caiman-api

All persistent state about VMs, nodes, networks, and storage is owned by
caiman-api. No other component reads or writes this state directly.
Components that need information call the API.

The single exception is caiman-vmm, which is the only process that
opens /dev/kvm and writes to its own per-VM working directory under
/var/lib/caiman/vms/{id}/. The API supervises the VMM through PIDs
and Unix sockets, never by directly inspecting KVM state.

This rule has two consequences:

1. Authentication is enforced exactly once, by the API. Any component
   that wants to do something on behalf of a user obtains a JWT and
   calls the API.

2. The state machine for each VM lives in one place. There are no
   race conditions between two components both trying to mutate VM
   state at the same time.

### 3.5 REST and JSON between components

Inter-component communication uses HTTP/REST with JSON bodies. We do
not use gRPC, Cap'n Proto, or other binary protocols. The reasons:

- Debuggable with curl and a terminal.
- Standard tooling (Postman, Insomnia, browser network tab) works
  without plugins.

- Easy to expose to AI assistants via MCP, which speaks JSON.

JSON fields use camelCase exclusively (cpuUsagePct, not
cpu_usage_pct). Rust struct fields use snake_case internally with
#[serde(rename_all = "camelCase")] at the boundary.

### 3.6 Each component is a separate binary

Caiman is not a single monolithic binary. Each major component
(caiman-vmm, caiman-api, caiman-storage, caiman-drs, etc.) is
its own binary, managed by systemd, with its own configuration file
under /etc/caiman/.

The reasons:

- Independent restart and upgrade. A bug in the DRS does not bring down
  the VMM.

- Easier to reason about resource usage and crash boundaries.
- Mirrors how mature systems (PostgreSQL, the Linux kernel) are
  structured.

### 3.7 Static builds with musl

Production binaries are statically linked against musl libc. They have
no runtime dependencies beyond the Linux kernel. A single binary can be
copied to any x86_64 Linux host (kernel 5.15+) and runs.

The musl-1.2.5/ directory in the repository contains the musl source
tree used by the build system to produce the bootable ISO.

### 3.8 Open source forever — Apache 2.0

The entire codebase is licensed Apache 2.0. There is no open-core
business model. There is no enterprise edition with paywalled features.
There are no proprietary plugins.

This is a deliberate choice. We believe HCI for SMB is underserved
because every existing player gates the most useful features behind
expensive licenses. Caiman's commercial model, when it exists, will
be services and support — never code that the community cannot run.

### 3.9 Async everywhere, no global locks

All backend components are built on tokio. We avoid global mutexes
and prefer message passing (tokio::sync::mpsc, broadcast) and
sharded state (Arc<DashMap>, per-VM state machines) over lock
contention. A single contended Mutex<HashMap<VmId, _>> is a code
smell.

### 3.10 Plain ASCII in Rust source files

Source files contain no Unicode characters. Box-drawing characters,
em-dashes, accented letters in comments — none of these. The reason
is that static musl builds break with non-ASCII source under some
toolchain configurations, in ways that are hard to debug. Comments
and string literals stick to ASCII. Externally-visible strings
(user-facing messages, JSON bodies) may use Unicode through the
normal &str path.

---

## 4. System Architecture

### 4.1 High-level view

A Caiman node is a Linux host running a coordinated set of Rust
daemons. From bottom to top:

                  Operator / API client / Claude / Cursor
                                    |
                                    v
    +-----------------------------------------------------+
    |  caiman-ui  (React dashboard, served by nginx)      |
    +---------------------+-------------------------------+
                          |  HTTPS + WebSocket
                          v
    +-----------------------------------------------------+
    |  caiman-api  (axum, JWT, REST, WS)    port 8765     |
    |  Single source of truth for VMs, nodes, networks    |
    +-----------------------------------------------------+
        |             |             |             |
        v             v             v             v
    +------------+  +---------+  +--------+  +-----------+
    | caiman-vmm |  | storage |  |  drs   |  |  bridge   |
    | per VM     |  |         |  |        |  | migration |
    | KVM ioctl  |  |         |  |        |  |  engine   |
    +------+-----+  +----+----+  +---+----+  +-----+-----+
           |             |           |             |
           v             v           v             v
    +-----------------------------------------------------+
    |          Linux kernel (KVM, eBPF, XDP)              |
    +-----------------------------------------------------+
           |
           v
    +-----------------------------------------------------+
    |  Hardware: x86_64 + VT-x/AMD-V, IOMMU,              |
    |  NVMe, 25/100 GbE NIC, optional NVIDIA GPU          |
    +-----------------------------------------------------+

Alongside this, caiman-mcp exposes the same functionality via the
Model Context Protocol so AI assistants can control the cluster.

### 4.2 Files on disk

    /etc/caiman/                  # Configuration files (TOML)
      api.toml                    # API listen address, JWT secret
      vmm.toml                    # Default VMM parameters
      storage.toml                # Storage backend choice and pool config
      cluster.toml                # Federation membership (v1.2+)

    /var/lib/caiman/              # Persistent data
      vms/
        {vm_id}/                  # Per-VM working directory
          state.json              # VM config + status (camelCase JSON)
          disk.img                # Virtual disk (raw)
          console.log             # Captured guest serial output
          console.sock            # Unix socket for live console
        {vm_id}.pty               # PTY host-side
        {vm_id}.vmm.log           # caiman-vmm stderr (debug log)
      kernels/                    # Guest kernels + initrds
        vmlinuz-alpine
        caiman-initrd.img
      base-images/                # Read-only base disk images (v1.0+)
        caiman-base-1.0.img
      bridge-jobs/{job_id}/       # Caiman Bridge migration state (v1.1+)
        plan.json
        progress.log

    /var/run/caiman/              # Runtime sockets and PIDs
      api.sock
      vmm-{vm_id}.pid

    /usr/local/bin/               # Static binaries
      caiman-api
      caiman-vmm
      caiman-storage
      caiman-drs
      caiman-mcp
      caiman                      # CLI

    /etc/systemd/system/          # systemd units
      caiman-api.service
      caiman-storage.service
      caiman-drs.service

Known inconsistency to fix in v1.0: the .pty and .vmm.log files
currently live next to the per-VM directory rather than inside it.
This is a historical artifact and will be consolidated under
{vm_id}/pty and {vm_id}/vmm.log in a future release.

### 4.3 The state.json schema

Each VM state.json contains the authoritative configuration and live
status. Example:

    {
      "id": "vm-04a24dc7",
      "uuid": "013bd593-1242-42b4-a09e-21dbc355da6f",
      "name": "boot-test",
      "status": "ACTIVE",
      "powerState": "Running",
      "taskState": null,
      "pid": 733258,
      "cpus": 1,
      "memMib": 256,
      "diskPath": "/var/lib/caiman/vms/vm-04a24dc7/disk.img",
      "ip": "10.100.0.31",
      "mac": "02:aa:bb:00:04:a",
      "tap": "caim04a24dc7",
      "netMode": "nat",
      "nodeName": "caiman-bare-01",
      "hypervisor": "caiman-vmm",
      "zone": "caiman-zone-1",
      "autostart": false,
      "pty": "/var/lib/caiman/vms/vm-04a24dc7.pty",
      "kernel": "/var/lib/caiman/kernels/vmlinuz-alpine",
      "initrd": "/var/lib/caiman/kernels/caiman-initrd.img",
      "labels": {},
      "projectId": null,
      "userId": null,
      "securityGroups": [],
      "volumes": [],
      "createdAt": "2026-05-11T14:30:30Z",
      "startedAt": "2026-05-14T09:39:28Z",
      "cpuUsagePct": 0.0,
      "memUsedMib": 0,
      "uptimeSecs": 0
    }

The schema deliberately follows OpenStack Nova conventions (powerState,
taskState, flavor, securityGroups) so operators familiar with OpenStack
find Caiman immediately recognizable, and so future OpenStack
compatibility shims are cheap.

Known wart to fix in v1.0: the status field uses OpenStack-style
uppercase values (ACTIVE, SHUT_OFF) at rest but the UI expects more
human-friendly values (RUNNING, STOPPED). A transform layer in the UI
currently bridges this. v1.0 will normalize on a single vocabulary.

---

## 5. Components

### 5.1 caiman-vmm — virtual machine monitor

Status: works. Roughly 5,000 lines of Rust.

caiman-vmm is the only process that opens /dev/kvm. It is spawned by
caiman-api once per VM (as an independent process via
std::process::Command, not a child via fork) and runs until the guest
shuts down or is killed.

What it does:

- Opens /dev/kvm, creates a KVM VM and per-vCPU file descriptors.
- Allocates guest physical memory via mmap and registers it with KVM
  using KVM_SET_USER_MEMORY_REGION.

- Loads a Linux kernel directly into guest memory at a known address
  and constructs the boot parameters block (zero page, E820 map,
  command line).

- Implements three virtio devices: virtio-blk backed by a host file,
  virtio-net backed by a host TAP, and 16550A UART for the serial
  console.

- Runs the vCPU loop, handling KVM_EXIT_IO, KVM_EXIT_MMIO, and
  device-specific exits.

- Forwards the guest serial port to a Unix socket on the host
  (console.sock), which caiman-api exposes as a WebSocket.

What it does not do:

- No QEMU, no SeaBIOS, no GRUB. Caiman boots the kernel directly
  with earlycon=uart8250,io,0x3f8 and a built-in initrd.

- No PCI device emulation beyond the virtio MMIO devices.
- No software emulation fallback. KVM acceleration is required.

Key files: vmm/src/main.rs, vmm/src/virtio/, vmm/src/kvm.rs.

### 5.2 caiman-api — control plane

Status: works. Axum-based.

caiman-api is the single source of truth and the only authenticated
entry point. It exposes:

- REST endpoints for VM lifecycle (POST /api/vms, POST
  /api/vms/{id}/start, POST /api/vms/{id}/stop, DELETE /api/vms/{id}).

- Authentication via JWT (POST /auth/token). Tokens are short-lived
  (24 hours by default).

- WebSocket at /api/vms/{id}/console/ws for interactive serial
  console. The WebSocket upgrade is unauthenticated by header, since
  browsers cannot send Authorization on upgrade; access is instead
  gated by a single-use token obtained from the authenticated REST
  API.

- Bridge API for migration: POST /api/bridge/discover, POST
  /api/bridge/jobs, GET /api/bridge/jobs/{id} (see section 7).

The API supervises caiman-vmm processes through PIDs and Unix
sockets. It never reads or writes KVM state directly.

### 5.3 caiman-cni — networking

Status: works (basic). Sets up TAP devices, host bridges, and NAT
rules for VM networking. Supports NAT mode (VMs share host IP via
iptables NAT) and bridge mode (VMs join the host's L2 network). VXLAN
overlay for multi-node clusters is planned for v1.2.

### 5.4 caiman-storage — distributed storage

Status: scaffolding (~321 lines).

Intended architecture: distributed replicated block store (vSAN-style).
Each node contributes local NVMe to a cluster-wide pool; volumes are
replicated across N nodes (typically 2 or 3); NVMe-oF is the data path
between nodes for cross-node reads.

Today, caiman-storage exists as types, REST API surface, and algorithm
sketches. The replication and data path are not implemented. In v1.0,
local-only LVM thin pool will be the first real backend, providing
snapshots and thin provisioning without distribution.

### 5.5 caiman-drs — distributed resource scheduler

Status: scaffolding (~1260 lines).

Implements the Kubernetes scheduler extender protocol so kube-scheduler
can place pods on the optimal Caiman node. Computes the sigma (standard
deviation) of cluster load and identifies imbalanced clusters that
would benefit from VM migration. Currently dormant in single-node
mode; activates with federation (v1.2+).

### 5.6 caiman-livemig — live migration

Status: scaffolding (~678 lines).

Implements the pre-copy live migration protocol used by KVM/QEMU:
iterative memory copy with dirty page tracking, followed by a brief
stop-and-copy phase. Designed for sub-second blackout but not yet
implemented end-to-end. Handles intra-cluster live migration (Caiman
node to Caiman node). Cross-hypervisor migration is the responsibility
of caiman-bridge (section 7).

### 5.7 caiman-gpu — GPU passthrough and partitioning

Status: partial (~543 lines).

VFIO-PCI passthrough is implemented for full GPU assignment to a VM.
NVIDIA MIG and vGPU support are present as code skeletons but require
NVIDIA's licensed drivers and are not functional today.

### 5.8 caiman-microseg — micro-segmentation

Status: partial (~286 lines).

Translates label-selector policies (Kubernetes CRD style) into BPF map
entries. The XDP enforcement program (kernel/caiman_net.ko) is in
development. Today, identities are visible in the UI but policies are
not enforced.

### 5.9 caiman-bts — backup, templates, snapshots

Status: scaffolding (~737 lines).

REST API for VM snapshots, backups, and templates. Storage backend not
yet implemented. Will rest on top of caiman-storage once distributed
storage is functional.

### 5.10 caiman-mcp — Model Context Protocol server

Status: planned, v1.4.

Exposes Caiman functionality through MCP (the open protocol that allows
AI assistants like Claude Desktop, Cursor, and ChatGPT to control
external systems). The MCP server does not access state directly; it
calls caiman-api with the user JWT.

Tools planned: list_vms, get_vm, create_vm, start_vm, stop_vm,
delete_vm, get_cluster_health, vm_console_read, vm_metrics,
bridge_discover, bridge_migrate, search_logs.

Resources exposed: vm://{id}, node://{name}, cluster://.

The Caiman dashboard chat panel uses the same MCP server internally.
There is exactly one tool surface, used identically by external AI
clients and the embedded UI.

### 5.11 caiman-cli — command-line client

Status: partial. Rust binary "caiman" that calls caiman-api. Mirrors
the UI functionality for scripting and headless use.

### 5.12 caiman-kernel — XDP networking module

Status: skeleton. Kernel module caiman_net.ko in C plus BPF programs
in C, attached to the host physical NIC via XDP. Provides line-rate
packet steering, identity tagging, and micro-segmentation enforcement.

### 5.13 caiman-ui — web dashboard

Status: works. Roughly 15,000 lines of TypeScript.

React + Vite + Tailwind. Pages include Overview, Topology, VMs,
Storage, DRS, Micro-segmentation, GPU, and (planned) Bridge for
migration. JWT auth via localStorage and an Axios interceptor on every
request. Interactive serial console via xterm.js connected to the API
WebSocket.

The legacy caiman-ui/ directory (JSX) coexists with the current ui/
directory (TSX). Legacy is served at the marketing landing path; the
current dashboard is at ui.caimanos.com.

### 5.14 website — marketing site

Status: works. Static HTML/CSS/JS served at caimanos.com. Contains the
install script (install.sh), download links to the ISO, and project
overview. Hosted on a Contabo VPS behind nginx.

---

## 6. Data Flow — Creating and running a VM

The end-to-end flow for "user clicks Create VM in the UI" illustrates
how components interact:

1. User logs in via the UI. The browser sends username and password to
   POST /auth/token and receives a JWT.

2. User fills out the Create VM form and clicks Create. The browser
   sends POST /api/vms with the JWT in the Authorization header.

3. caiman-api validates the JWT, generates a new VM ID (vm-xxxxxxxx),
   and writes initial state to /var/lib/caiman/vms/{id}/state.json.

4. The API copies the base image to /var/lib/caiman/vms/{id}/disk.img
   (CoW clone), provisions a TAP device through caiman-cni, and
   responds with the new VM ID.

5. User clicks Start. The browser sends POST /api/vms/{id}/start.
6. The API spawns caiman-vmm as an independent process with
   environment variables pointing to the VM state file.

7. caiman-vmm opens /dev/kvm, sets up memory and vCPUs, loads the
   kernel, and begins running the vCPU loop. It writes its PID and
   working state back to state.json.

8. The guest boots. Kernel printk output flows out the serial port
   into a host PTY, which caiman-vmm forwards to
   /var/lib/caiman/vms/{id}/console.sock.

9. User clicks Console. The browser opens a WebSocket to
   wss://api.caimanos.com/api/vms/{id}/console/ws.

10. caiman-api accepts the WebSocket, connects to the VM console.sock,
    and proxies bytes bidirectionally. The user interacts with the
    guest in real time via xterm.js.

---

## 7. Caiman Bridge — Migration Architecture

Caiman Bridge is the assisted-migration subsystem that imports VMs
from foreign hypervisors into Caiman with minimum downtime. The
working assumption is that frictionless migration is the most valuable
feature Caiman can offer, because the reason organizations do not
change hypervisor is not the destination — it is the cost of moving.

### 7.1 Goals

- Discover all VMs on a source hypervisor with one operation.
- Convert disk formats automatically (vmdk to raw, qcow2 to raw, etc).
- Map source network configurations (vlan tags, MAC addresses) to
  Caiman equivalents.

- Minimize downtime per VM. Targets:
  - Cold mode: 5-10 minutes per VM (planned downtime).
  - Warm mode (v1.5): under 30 seconds (pre-copy + brief switchover).
  - Continuous sync (v2.0): under 1 second (real-time replication and
    manual failover).
- Roll back automatically if the destination VM fails to boot.

### 7.2 Supported sources

| Source           | Discovery | Cold migration | Warm migration |
|------------------|-----------|----------------|----------------|
| Proxmox VE       | v0.9      | v1.1 target    | v1.5 target    |
| libvirt / KVM    | v0.9      | v1.1 target    | v1.5 target    |
| VMware vSphere   | v0.9      | v1.3 target    | v1.6 target    |
| Nutanix AHV      | v0.9      | v1.3 target    | v1.6 target    |
| Oracle OLVM      | v0.9      | v1.3 target    | not planned    |
| Oracle VM Server | v0.9      | v1.3 target    | not planned    |
| OpenStack        | v0.9      | v1.3 target    | v1.7 target    |
| Harvester        | v0.9      | v1.3 target    | v1.6 target    |
| AWS EC2          | v0.9      | v1.4 target    | not planned    |

Discovery means the source API is reachable and we can list VMs and
their metadata. Cold migration means we can actually transfer the
disk and start the VM in Caiman.

### 7.3 Cold migration flow

Phases:

1. POST /api/bridge/discover with source credentials. Caiman returns
   the list of VMs available on the source.

2. POST /api/bridge/jobs with the selected VMs. Caiman creates a
   migration job with state in /var/lib/caiman/bridge-jobs/{job_id}/.

3. For each VM in the job:
   a. Take a snapshot on the source (live; VM stays running).
   b. Stream the disk to Caiman over HTTP/REST, converting format if

      necessary (qemu-img convert).

   c. Build the Caiman VM config from source metadata.
   d. Schedule planned downtime, notify the operator, wait for

      confirmation.

   e. Shut down the source VM.
   f. Copy the delta (changes since the snapshot).
   g. Boot the VM in Caiman.
   h. Validate that the guest boots. On failure, roll back: delete the

      Caiman VM and restart the source VM.

   i. On success, archive the source VM (do not delete by default).

4. The operator updates DNS or load balancer to point at the new
   address. This is operator action, not Caiman action.

### 7.4 Disk conversion

Caiman Bridge uses qemu-img convert for format conversion and virt-v2v
for Windows driver injection (Windows guests need virtio drivers added
before they can boot on Caiman). These are shelled out to from
caiman-bridge in the current design; long-term, a native Rust qcow2
reader is a possibility.

### 7.5 Network mapping

The Bridge UI prompts the operator to map source networks (vlan tags,
named networks in vSphere, Proxmox bridges) to Caiman networks. The
mapping is stored in the migration plan and applied per-VM during
boot.

### 7.6 What Bridge is not

Bridge is not a real-time replication tool by default. v1.1 and v1.3
target cold migration with planned downtime. v2.0 will introduce
Continuous Sync mode for near-zero-downtime cutover, marketed
separately as a disaster recovery feature.

Bridge is not a generic V2V tool that converts VMs between arbitrary
hypervisors. The destination is always Caiman. Bidirectional support
(Caiman to vSphere, for example) is not a current priority.

---

## 8. Security Model

### 8.1 Authentication

Today: JWT issued by caiman-api after a username/password login. Tokens
are short-lived (24 hours by default), signed with HS256, and validated
on every API call.

v1.0: PAM integration so operators can use OS users instead of a
hardcoded admin:admin123.

v1.2: LDAP and OIDC for enterprise identity providers.

### 8.2 Authorization

Today: all authenticated users have full access. There is no RBAC.

v1.2: Role-based access control with roles viewer, operator, and admin.
Namespaces and multi-tenancy in v1.5.

### 8.3 Transport security

The API and UI are served over HTTPS in production. nginx terminates
TLS. The internal API socket (/var/run/caiman/api.sock) uses Unix
permissions; only the caiman group can read or write it.

### 8.4 Process isolation

Each caiman-vmm runs as a separate process under its own UID. KVM
itself provides the strong hardware-enforced isolation between guest
and host. Host-side processes that supervise VMs do not require root
for most operations; they need CAP_NET_ADMIN for TAP setup and access
to /dev/kvm for VM creation.

### 8.5 Threat model

Caiman is not intended to defend against a compromised host. If the
host is rooted, all VMs are compromised. The threat model is:

In scope: protecting VMs from each other (KVM isolation, per-VM TAP,
micro-segmentation enforcing inter-VM policies). Protecting the control
plane from unauthenticated users and from authenticated users acting
outside their role.

Out of scope: protecting against a malicious operator with shell
access. Operators are trusted. Defending the VMM against a malicious
guest (KVM is assumed correct).

---

## 9. AI / MCP Integration

Caiman is built in the post-LLM era and treats AI assistants as
first-class users, not as an afterthought.

### 9.1 MCP-first design

Every operation a human can perform through the UI is also exposed as
a tool through caiman-mcp (the Model Context Protocol server). An
operator running Claude Desktop, Cursor, or any MCP-capable client can
manage their Caiman cluster with natural language.

This is not chat-on-top-of-an-API. The MCP server is built into the
platform with the same authentication, the same audit log, and the
same authorization model as the REST API. AI assistants are
indistinguishable from a careful operator clicking through the UI.

### 9.2 Embedded chat

The Caiman dashboard includes an optional chat panel that uses
caiman-mcp internally. The operator picks a backend LLM (Anthropic,
OpenAI, or a local Ollama instance running inside Caiman itself). API
keys, when used, are stored encrypted on the backend, never in the
browser.

### 9.3 Agentic operations (future, v1.5+)

Future capabilities under design:

- DRS rebalancing guided by an LLM that explains its reasoning to the
  operator.

- Anomaly detection that suggests remediations.
- Auto-remediation under operator-approved policies.

These are not required for v1.4 launch.

### 9.4 What we do not do

- We do not send VM contents, logs, or sensitive configuration to
  third-party AI providers without explicit operator consent.

- We do not require an internet connection. Local LLM (Ollama on
  Caiman itself) is a first-class option.

- We do not lock operators into one AI provider.

---

## 10. Federation Architecture (target, v1.2+)

Today, Caiman runs on a single node. Federation — clustering 3 or more
Caiman nodes that share state and can run any VM on any node — is the
headline feature for v1.2.

### 10.1 Membership

Each node runs a caiman-cluster daemon that uses the memberlist gossip
protocol to discover and track peers. Nodes join the cluster by
knowing the address of any one existing peer.

### 10.2 Consensus

Cluster-wide state (which VM runs on which node, which volumes exist,
locks held during migrations) is stored in a Raft-replicated log. The
implementation is raft-rs (the etcd-style Rust Raft library) with a
persistent log on each node.

The leader handles all writes. Followers handle reads. Failure of the
leader triggers an election with sub-second convergence.

### 10.3 Cross-node networking

VXLAN overlay between nodes, terminated on each host by the XDP
program. VMs on different physical hosts see each other as if they
were on the same L2 network.

### 10.4 Cross-node storage

caiman-storage exposes volumes that are replicated across at least two
nodes. NVMe-oF is the data path for remote reads; local reads go to
the local replica. Replication is synchronous within the cluster.

### 10.5 Live migration

caiman-livemig transfers a running VM between nodes using pre-copy
memory replication. Target blackout: under 1 second. Requires shared
storage or storage migration to complete first.

### 10.6 High availability

If a node fails, the cluster leader detects the failure (gossip
timeout) and instructs another node to restart the affected VMs from
their last known state. Storage replicas guarantee that disks are
available on at least one remaining node.

---

## 11. Technology Choices and Rationale

| Choice | Rationale |
|---|---|
| Rust for backend | Memory safety in privileged code; static binaries; mature ecosystem |
| tokio async runtime | De facto standard; mature; excellent ecosystem |
| axum web framework | Built on hyper; tower middleware; ergonomic |
| KVM only, no QEMU | Drastically smaller attack surface; native Rust integration; we control the stack |
| musl for static binaries | One binary, no runtime dependencies, deterministic builds |
| React + TypeScript for UI | Strong typing; ecosystem; team familiarity |
| Tailwind for UI styling | Utility-first; design tokens; rapid iteration |
| nginx for TLS termination | Battle-tested; well-understood; minimal config |
| systemd for service management | Standard on every supported distro; native integration |
| JWT for auth | Stateless; standard; debuggable |
| memberlist for gossip | Battle-tested (HashiCorp); decoupled from consensus |
| raft-rs for consensus | Most mature Rust Raft; used in TiKV |
| BPF / XDP for networking | Line-rate forwarding; user-defined logic in kernel |
| Apache 2.0 license | Permissive; enterprise-friendly; community-friendly |

---

## 12. Roadmap reference

See ROADMAP.md for the detailed release plan. At a high level:

- v0.9 (current): single-node alpha. VMM, API, UI, console, create
  start stop delete, import discovery.

- v1.0 (3-4 weeks): LVM thin pool storage, PAM auth, ISO rebuild, base
  image library.

- v1.1 (4 weeks): Caiman Bridge cold migration for Proxmox and
  libvirt.

- v1.2 (4-6 weeks): Federation (3+ nodes, gossip + Raft).
- v1.3 (4 weeks): Caiman Bridge cold migration for vSphere, Nutanix,
  OpenStack, Harvester.

- v1.4 (4 weeks): MCP server + embedded chat in UI.
- v1.5 (6-8 weeks): caiman-storage vSAN-style replication.
- v1.6 (4-6 weeks): Intra-cluster live migration; Caiman Bridge warm
  mode for vSphere.

- v1.7 (3-4 weeks): HA failover; DRS active across nodes.
- v1.8 (4-6 weeks): XDP networking finalized; micro-segmentation
  enforcing.

- v2.0: Stable release with measured benchmarks; Continuous Sync mode
  for Bridge.

---

## 13. Contributing

See CONTRIBUTING.md for the developer guide: setup, workflow, code
style, and the process for submitting changes.

If you are considering joining the project as a co-founder or core
contributor, see also the careers page at caimanos.com/careers.

---

## 14. References

- KVM API documentation:
  https://docs.kernel.org/virt/kvm/api.html

- Rust KVM bindings (kvm-bindings, kvm-ioctls):
  https://crates.io/crates/kvm-ioctls

- The Model Context Protocol:
  https://modelcontextprotocol.io

- Raft consensus algorithm:
  https://raft.github.io

- Linux kernel boot protocol:
  https://docs.kernel.org/x86/boot.html

---

This document is maintained by the core Caiman team. Substantial
changes require approval from the project maintainer. Last updated:
May 2026.
