// SPDX-License-Identifier: GPL-2.0-only
/*
 * kernel/ebpf/xdp_vm_router.c  — XDP program: route packets to VMs
 *
 * Attached to the physical uplink NIC by caiman_net_xdp_attach().
 * For each incoming frame the program looks up the destination MAC in
 * the mac_to_ifindex map and XDP_REDIRECT-s directly to the tap device
 * of the matching VM — bypassing the kernel network stack entirely.
 * Non-VM traffic is XDP_PASS-ed to continue through the normal stack.
 *
 * Compile:
 *   clang -O2 -g -target bpf \
 *     -I/usr/include/x86_64-linux-gnu \
 *     -c xdp_vm_router.c -o xdp_vm_router.o
 *
 * Pin:
 *   bpftool prog load xdp_vm_router.o /sys/fs/bpf/caiman/vm<ID> \
 *     map name mac_to_ifindex pinned /sys/fs/bpf/caiman/mac_map
 */

#include <linux/bpf.h>
#include <linux/if_ether.h>
#include <linux/ip.h>
#include <linux/ipv6.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_endian.h>

#include "maps.h"

/* Redirect helper map — kernel manages the actual ifindex table */
struct {
        __uint(type,        BPF_MAP_TYPE_DEVMAP_HASH);
        __uint(max_entries, MAX_VMS);
        __type(key,         __u32);   /* ifindex of tap device   */
        __type(value,       struct bpf_devmap_val);
        __uint(pinning,     LIBBPF_PIN_BY_NAME);
} tx_port SEC(".maps");

/*
 * mac_to_ifindex: ETH_ALEN-byte MAC -> tap ifindex
 * Populated by the CNI plugin / VMM via bpftool or the Rust netlink client.
 */
struct {
        __uint(type,        BPF_MAP_TYPE_HASH);
        __uint(max_entries, MAX_VMS);
        __type(key,         __u8[ETH_ALEN]);
        __type(value,       __u32);    /* tap device ifindex      */
        __uint(pinning,     LIBBPF_PIN_BY_NAME);
} mac_to_ifindex SEC(".maps");

/* Per-VM stats: ifindex -> {rx_packets, rx_bytes} */
struct vm_stats {
        __u64 rx_packets;
        __u64 rx_bytes;
};

struct {
        __uint(type,        BPF_MAP_TYPE_PERCPU_HASH);
        __uint(max_entries, MAX_VMS);
        __type(key,         __u32);
        __type(value,       struct vm_stats);
        __uint(pinning,     LIBBPF_PIN_BY_NAME);
} vm_rx_stats SEC(".maps");

/* ------------------------------------------------------------------ */

SEC("xdp")
int xdp_vm_router(struct xdp_md *ctx)
{
        void *data_end = (void *)(long)ctx->data_end;
        void *data     = (void *)(long)ctx->data;

        struct ethhdr *eth = data;
        if ((void *)(eth + 1) > data_end)
                return XDP_ABORTED;

        /* Look up destination MAC in our VM map */
        __u32 *ifindex = bpf_map_lookup_elem(&mac_to_ifindex, eth->h_dest);
        if (!ifindex)
                return XDP_PASS;   /* Not a VM MAC — let kernel handle it */

        /* Update per-VM RX stats (lock-free percpu) */
        struct vm_stats *stats = bpf_map_lookup_elem(&vm_rx_stats, ifindex);
        if (stats) {
                stats->rx_packets++;
                stats->rx_bytes += (data_end - data);
        }

        /* Zero-copy redirect to tap device */
        return bpf_redirect_map(&tx_port, *ifindex, XDP_PASS);
}

/* TC egress: mirrors VM->host traffic counters (attached via tc filter) */
SEC("tc")
int tc_vm_egress(struct __sk_buff *skb)
{
        void *data_end = (void *)(long)skb->data_end;
        void *data     = (void *)(long)skb->data;

        struct ethhdr *eth = data;
        if ((void *)(eth + 1) > data_end)
                return TC_ACT_OK;

        /* Source MAC identifies the originating VM */
        __u32 *ifindex = bpf_map_lookup_elem(&mac_to_ifindex, eth->h_source);
        if (!ifindex)
                return TC_ACT_OK;

        /* Could update tx stats here if we add a tx_stats map */
        return TC_ACT_OK;
}

char _license[] SEC("license") = "GPL";
