// SPDX-License-Identifier: GPL-2.0-only
/*
 * caiman_net/xdp_loader.c  — attach / detach XDP programs per VM
 *
 * Loads a pre-compiled BPF object from a pinned path in /sys/fs/bpf/
 * and attaches it to the uplink NIC with XDP_FLAGS_DRV_MODE (native XDP).
 * Falls back to XDP_FLAGS_SKB_MODE if the driver doesn't support native.
 *
 * The XDP program (kernel/ebpf/xdp_vm_router.c) inspects the destination
 * MAC and either XDP_REDIRECT-s to the tap device mapped in a BPF hash
 * map, or passes non-VM traffic through.
 */

#include <linux/module.h>
#include <linux/bpf.h>
#include <linux/filter.h>
#include <linux/netdevice.h>
#include <net/xdp.h>

#include "caiman_net.h"

/* Subsystem-level init/exit (global BPF infrastructure if needed) */
int caiman_net_xdp_init(void)
{
        pr_info("caiman_net: XDP subsystem ready\n");
        return 0;
}

void caiman_net_xdp_exit(void)
{
        pr_info("caiman_net: XDP subsystem stopped\n");
}

/*
 * caiman_net_xdp_attach - load a pinned BPF prog and attach it to ctx->uplink.
 *
 * @ctx:          VM networking context
 * @bpf_obj_path: path under /sys/fs/bpf/ e.g. "/sys/fs/bpf/caiman/vm42"
 *
 * The BPF object must export a program section named "xdp_vm_router".
 * The caller (CNI plugin or VMM) is responsible for pinning the object
 * and populating the mac_to_ifindex BPF map before calling this.
 */
int caiman_net_xdp_attach(struct caiman_net_ctx *ctx, const char *bpf_obj_path)
{
        struct bpf_prog *prog;
        int err;

        /* Retrieve already-pinned BPF program by path */
        prog = bpf_prog_get_type_path(bpf_obj_path, BPF_PROG_TYPE_XDP);
        if (IS_ERR(prog)) {
                pr_err("caiman_net: xdp_attach vm_id=%u: cannot get prog '%s': %ld\n",
                       ctx->vm_id, bpf_obj_path, PTR_ERR(prog));
                return PTR_ERR(prog);
        }

        /* Try native XDP first, fall back to generic */
        err = dev_xdp_attach(ctx->uplink, NULL, NULL, prog,
                             NULL, XDP_FLAGS_DRV_MODE);
        if (err == -EOPNOTSUPP) {
                pr_warn("caiman_net: vm_id=%u: NIC %s doesn't support native XDP,"
                        " falling back to SKB mode\n",
                        ctx->vm_id, ctx->uplink->name);
                err = dev_xdp_attach(ctx->uplink, NULL, NULL, prog,
                                     NULL, XDP_FLAGS_SKB_MODE);
        }

        if (err) {
                bpf_prog_put(prog);
                pr_err("caiman_net: xdp_attach vm_id=%u failed: %d\n",
                       ctx->vm_id, err);
                return err;
        }

        spin_lock(&ctx->lock);
        if (ctx->xdp_prog)
                bpf_prog_put(ctx->xdp_prog);
        ctx->xdp_prog = prog;
        spin_unlock(&ctx->lock);

        pr_info("caiman_net: XDP attached vm_id=%u uplink=%s prog=%s\n",
                ctx->vm_id, ctx->uplink->name, bpf_obj_path);
        return 0;
}
EXPORT_SYMBOL_GPL(caiman_net_xdp_attach);

void caiman_net_xdp_detach(struct caiman_net_ctx *ctx)
{
        if (!ctx || !ctx->xdp_prog)
                return;

        dev_xdp_attach(ctx->uplink, NULL, NULL, NULL, NULL, 0);

        spin_lock(&ctx->lock);
        bpf_prog_put(ctx->xdp_prog);
        ctx->xdp_prog = NULL;
        spin_unlock(&ctx->lock);

        pr_info("caiman_net: XDP detached vm_id=%u\n", ctx->vm_id);
}
EXPORT_SYMBOL_GPL(caiman_net_xdp_detach);
