// SPDX-License-Identifier: GPL-2.0-only
/*
 * caiman_net - KVM direct kernel networking module
 *
 * Replaces QEMU's vhost-net with a zero-copy eBPF/XDP datapath.
 * Exposes a netlink control interface consumed by the Rust VMM and
 * the Kubenet CNI plugin.
 *
 * Architecture:
 *   [Guest vCPU] <-virtio-ring-> [caiman_net] <-XDP-> [Physical NIC]
 *                                      |
 *                                  [netlink]
 *                                      |
 *                             [Rust VMM / CNI plugin]
 */

#include <linux/module.h>
#include <linux/kernel.h>
#include <linux/init.h>
#include <linux/kvm_host.h>
#include <linux/eventfd.h>
#include <linux/vhost.h>
#include <linux/virtio_net.h>
#include <linux/netdevice.h>
#include <linux/bpf.h>
#include <linux/filter.h>
#include <net/genetlink.h>

#include "caiman_net.h"

MODULE_LICENSE("GPL v2");
MODULE_AUTHOR("caiman project");
MODULE_DESCRIPTION("KVM direct kernel networking: eBPF/XDP datapath, no QEMU");
MODULE_VERSION("0.1.0");

/* Global VM network context table — one entry per VM */
static DEFINE_HASHTABLE(vm_net_table, KVM_NET_HT_BITS);
static DEFINE_SPINLOCK(vm_net_lock);

/* ------------------------------------------------------------------ */
/*  VM network context lifecycle                                        */
/* ------------------------------------------------------------------ */

struct caiman_net_ctx *caiman_net_ctx_alloc(u32 vm_id, const u8 *mac,
                                      struct net_device *uplink)
{
        struct caiman_net_ctx *ctx;

        ctx = kzalloc(sizeof(*ctx), GFP_KERNEL);
        if (!ctx)
                return ERR_PTR(-ENOMEM);

        ctx->vm_id  = vm_id;
        ctx->uplink = uplink;
        ether_addr_copy(ctx->mac, mac);
        spin_lock_init(&ctx->lock);
        atomic64_set(&ctx->rx_packets, 0);
        atomic64_set(&ctx->tx_packets, 0);
        atomic64_set(&ctx->rx_bytes,   0);
        atomic64_set(&ctx->tx_bytes,   0);

        hash_add_rcu(vm_net_table, &ctx->hnode, vm_id);
        pr_info("caiman_net: ctx allocated vm_id=%u mac=%pM uplink=%s\n",
                vm_id, mac, uplink->name);
        return ctx;
}
EXPORT_SYMBOL_GPL(caiman_net_ctx_alloc);

void caiman_net_ctx_free(struct caiman_net_ctx *ctx)
{
        if (!ctx)
                return;

        spin_lock(&vm_net_lock);
        hash_del_rcu(&ctx->hnode);
        spin_unlock(&vm_net_lock);

        /* Detach XDP program from uplink if we pinned one */
        if (ctx->xdp_prog) {
                bpf_prog_put(ctx->xdp_prog);
                ctx->xdp_prog = NULL;
        }

        synchronize_rcu();
        pr_info("caiman_net: ctx freed vm_id=%u\n", ctx->vm_id);
        kfree(ctx);
}
EXPORT_SYMBOL_GPL(caiman_net_ctx_free);

struct caiman_net_ctx *caiman_net_ctx_find(u32 vm_id)
{
        struct caiman_net_ctx *ctx;

        rcu_read_lock();
        hash_for_each_possible_rcu(vm_net_table, ctx, hnode, vm_id) {
                if (ctx->vm_id == vm_id) {
                        rcu_read_unlock();
                        return ctx;
                }
        }
        rcu_read_unlock();
        return NULL;
}
EXPORT_SYMBOL_GPL(caiman_net_ctx_find);

/* ------------------------------------------------------------------ */
/*  Virtio-ring doorbell hook (called from VMM via ioctl)              */
/* ------------------------------------------------------------------ */

/*
 * caiman_net_kick_tx - Guest has written to the TX virtqueue.
 * We drain the available ring and push frames directly via XDP redirect.
 * This is called with IRQs disabled from the vCPU thread.
 */
int caiman_net_kick_tx(struct caiman_net_ctx *ctx, struct vhost_virtqueue *vq)
{
        /* Placeholder: full virtqueue drain + XDP_REDIRECT implemented
         * in xdp_loader.c once the BPF map handles are set up. */
        (void)ctx;
        (void)vq;
        return 0;
}
EXPORT_SYMBOL_GPL(caiman_net_kick_tx);

/* ------------------------------------------------------------------ */
/*  Module init / exit                                                  */
/* ------------------------------------------------------------------ */

static int __init caiman_net_init(void)
{
        int err;

        pr_info("caiman_net: loading (eBPF/XDP datapath, no QEMU)\n");

        err = caiman_net_netlink_init();
        if (err) {
                pr_err("caiman_net: netlink init failed: %d\n", err);
                return err;
        }

        err = caiman_net_xdp_init();
        if (err) {
                pr_err("caiman_net: XDP subsystem init failed: %d\n", err);
                caiman_net_netlink_exit();
                return err;
        }

        pr_info("caiman_net: loaded OK\n");
        return 0;
}

static void __exit caiman_net_exit(void)
{
        caiman_net_xdp_exit();
        caiman_net_netlink_exit();
        pr_info("caiman_net: unloaded\n");
}

module_init(caiman_net_init);
module_exit(caiman_net_exit);
