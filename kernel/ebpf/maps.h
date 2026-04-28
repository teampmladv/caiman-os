/* SPDX-License-Identifier: GPL-2.0-only */
/*
 * kernel/ebpf/maps.h — shared constants for all eBPF programs
 *
 * Keep in sync with:
 *   - vmm/src/ebpf.rs   (Rust constants for map population)
 *   - cni/src/ebpf.rs   (CNI map setup)
 */

#ifndef _KVM_NET_MAPS_H
#define _KVM_NET_MAPS_H

/* Maximum number of simultaneous VMs per host */
#define MAX_VMS  256

/* BPF pinning base path — must match vmm/cni Rust constants */
#define BPF_PIN_BASE  "/sys/fs/bpf/caiman"

#endif /* _KVM_NET_MAPS_H */
