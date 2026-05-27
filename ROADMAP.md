# Caimán OS — Roadmap

This document is the single source of truth for where Caimán OS is going and in
what order. It supersedes the short milestone list in the README. Where the two
disagree, this file wins, and the README will be updated to match.

Caimán OS is **alpha software**. The VMM runs real VMs without QEMU and boots
standard Linux distributions, but most higher-level subsystems are still
scaffolding. This roadmap is honest about that: each item is labelled with its
real state, and dates are directional, not commitments.

---

## How to read this

- **Milestones are capability-based, not version-numbered.** A milestone lands
  when the capability works, not when a tag is cut.
- **Release tags (ISO builds, container images) are decoupled from maturity
  milestones.** A `vX.Y.Z` tag is a build artifact for a given day; it does not
  imply the project as a whole has reached that maturity. The project as a whole
  remains pre-1.0 until the Stable Release criteria at the bottom are met.
- **State labels** follow the README convention: `works` · `partial` ·
  `scaffolding` · `skeleton` · `target`.
- **Ordering is deliberate.** We finish the things that deliver value first
  (migration off existing hypervisors), then the platform depth that makes
  Caimán a production destination, and we leave the self-service layer for last.

---

## Where we are now (alpha)

| Capability | State |
| --- | --- |
| caiman-vmm — KVM without QEMU, virtio-blk / virtio-net / serial | works |
| Boots standard distros (Alpine, Debian 12 with custom 6.6 kernel) | works |
| caiman-api — REST + JWT + WebSocket, single-node lifecycle | works |
| caiman-ui — React dashboard, interactive console | works |
| caiman-cni — NAT + bridge networking | works |
| Import wizard — discovery against Proxmox / vSphere / oVirt / Nutanix / Oracle | partial (discovery only; disk conversion stubbed) |
| Distributed storage, live migration, DRS, HA, micro-seg | scaffolding / skeleton |

The runtime exists and works on a single node. Everything above the single-node
runtime needs implementation. That is what the phases below sequence.

---

## Phase 1 — Solidify the single-node core

**Goal:** everything we already claim actually works and holds up to scrutiny.
This is the credibility phase; nothing new is promised until the existing surface
is solid.

- [ ] Fix the boot-latency regression (HLT handler exits to userspace; the
      `sleep` in the vCPU loop breaks timing). Publish a *measured* boot time
      only after this lands.
- [ ] virtio-net working in the default guest images (kernel built with
      `CONFIG_VIRTIO_NET`).
- [ ] Real authentication via PAM (replace the alpha `admin/admin123` default).
- [ ] LVM thin-pool local storage with snapshots.
- [ ] Internal cleanup: unify `state.json` vocabulary, normalise per-VM file
      layout, unify component versions, repository hygiene.
- [ ] ISO rebuild against the current VMM.

**Outcome:** a single-node hypervisor that is honest, reproducible, and safe to
hand to an evaluator.

---

## Phase 2 — Caimán Bridge: cold import (Proxmox, libvirt)

**Goal:** move real workloads off an existing hypervisor and onto Caimán, with a
scheduled maintenance window.

The discovery layer already calls real source APIs. The missing piece is the
disk-conversion data path — and it is shared across all source types, so
finishing it once unlocks every source.

- [ ] Complete the disk-conversion data path (the currently stubbed step).
- [ ] End-to-end **cold** import for **Proxmox** and **libvirt/KVM** sources
      (these are KVM-native, so disk formats are closest to ours).
- [ ] Migrated VMs boot and run on Caimán.

**Scope note:** this phase is *cold* migration only — power off at the source,
convert, boot on Caimán. Warm / continuous sync depends on live migration
(Phase 4). Cold migration with a maintenance window already covers the large
majority of real-world "move off my old hypervisor" needs.

**Outcome:** the first genuinely useful, demonstrable capability beyond the
single-node runtime — a migration path in, not just a place to create VMs.

---

## Phase 3 — Caimán Bridge: vSphere and broader sources

**Goal:** cover the migration sources people most urgently want to leave.

- [ ] Cold import for **vSphere** (VMDK / VDDK handling — harder than KVM-native
      sources, hence sequenced after Proxmox/libvirt).
- [ ] Then **Nutanix**, **OpenStack**, **Harvester**, **Oracle**, as demand
      dictates.

**Outcome:** Caimán Bridge becomes a credible "exit any hypervisor" tool. Still
cold-only; live/continuous modes arrive with Phase 4.

---

## Phase 4 — Cluster, distributed storage, live migration, HA

**Goal:** turn the single-node runtime into something that can host production at
scale — and unlock the warm/continuous Bridge modes.

- [ ] Cluster federation (3+ nodes; gossip + Raft consensus).
- [ ] Caimán Storage — distributed block storage (vSAN-style).
- [ ] Live migration end-to-end (pre-copy; dirty-page tracking; sub-second
      blackout target, to be *measured*).
- [ ] HA / auto-failover; DRS active across nodes.

**Outcome:** Caimán graduates from "single-node, good for pilots and smaller
deployments" to a runtime that can carry production-scale, highly-available
workloads. This is also what enables warm and continuous Bridge sync.

---

## Phase 5 — Sovereign runtime: identity, audit, portability

**Goal:** make Caimán deployable in regulated and public-sector environments.

- [ ] Identity federation (Keycloak; SPID / CIE / eIDAS connectors for
      public-sector identity).
- [ ] Tamper-evident audit logging, per-tenant scoping.
- [ ] Workload portability (clean export/import, no lock-in).
- [ ] Service-level indicator and SLA reporting (built on the existing
      monitoring stack).
- [ ] Hardening to the security posture regulated deployments require.

**Outcome:** Caimán becomes a runtime that an operator can deploy on sovereign
infrastructure and stand behind in front of an auditor.

---

## Phase 6 — Self-service (PaaS): autodespliegue

**Goal:** the developer-facing layer — push code, get a running service — on top
of the now-hardened sovereign runtime.

- [ ] Git push / Dockerfile / buildpack -> build -> deploy.
- [ ] Managed data services (PostgreSQL, Redis) provisioned on-cluster.
- [ ] Self-service developer portal and environments.

This is intentionally **last**. Self-service deployment is greenfield
convenience; it depends on everything below it being solid, and it matters less
than migration and reliability for the environments Caimán targets first.

---

## Off the critical path (parallel / opportunistic)

These are real and wanted, but they do not gate the value sequence above and are
not on the critical path:

- **XDP networking** (`caiman_net.ko`) and **micro-segmentation** enforcement.
- **MCP / AI integration** (MCP server + embedded assistant). Compelling, but it
  does not move migration or reliability forward, so it is deliberately not
  pulled ahead of the phases above.

---

## Stable Release (v2.0) criteria

The project leaves alpha and makes production claims only when:

- Live migration, HA, and distributed storage are `works`, not `scaffolding`.
- Performance targets are replaced by **measured, published benchmarks**.
- A clean upgrade path exists and breaking changes are no longer expected.

Until then: pre-1.0, breaking changes expected, not for production.

---

## A note on honesty

Caimán is early. Established platforms are mature and battle-tested; Caimán is
not, yet. The one thing this project will not do is overstate where it is. Every
capability is labelled with its real state, performance numbers are targets until
measured, and this roadmap is sequenced by what actually delivers value rather
than by what demos well.

*Apache 2.0. Contributions welcome — start with the component README to
understand current vs. designed state.*
