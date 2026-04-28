// SPDX-License-Identifier: GPL-2.0-only
/*
 * caiman_net/netlink.c  — Generic Netlink family "caiman_net"
 *
 * Provides the control-plane socket used by:
 *   - The Rust VMM  (vmm/) to add/remove VM networking contexts
 *   - The CNI plugin (cni/) to attach XDP programs per VM
 *
 * Wire protocol uses struct nlattr TLVs; attribute IDs defined in
 * caiman_net.h and mirrored in the Rust netlink.rs client.
 */

#include <linux/module.h>
#include <net/genetlink.h>
#include <linux/netdevice.h>

#include "caiman_net.h"

/* ------------------------------------------------------------------ */
/*  Generic Netlink family definition                                   */
/* ------------------------------------------------------------------ */

static struct genl_family caiman_net_family;

static const struct nla_policy caiman_net_policy[KVM_NET_ATTR_MAX + 1] = {
        [KVM_NET_ATTR_VM_ID]   = { .type = NLA_U32 },
        [KVM_NET_ATTR_MAC]     = { .type = NLA_BINARY, .len = ETH_ALEN },
        [KVM_NET_ATTR_UPLINK]  = { .type = NLA_STRING, .len = IFNAMSIZ },
        [KVM_NET_ATTR_BPF_OBJ] = { .type = NLA_STRING, .len = PATH_MAX },
        [KVM_NET_ATTR_STATS]   = { .type = NLA_NESTED },
};

/* ------------------------------------------------------------------ */
/*  Command handlers                                                    */
/* ------------------------------------------------------------------ */

static int cmd_vm_add(struct sk_buff *skb, struct genl_info *info)
{
        u32 vm_id;
        u8  mac[ETH_ALEN];
        char ifname[IFNAMSIZ];
        struct net_device   *uplink;
        struct caiman_net_ctx  *ctx;

        if (!info->attrs[KVM_NET_ATTR_VM_ID] ||
            !info->attrs[KVM_NET_ATTR_MAC]   ||
            !info->attrs[KVM_NET_ATTR_UPLINK])
                return -EINVAL;

        vm_id = nla_get_u32(info->attrs[KVM_NET_ATTR_VM_ID]);
        nla_memcpy(mac, info->attrs[KVM_NET_ATTR_MAC], ETH_ALEN);
        nla_strlcpy(ifname, info->attrs[KVM_NET_ATTR_UPLINK], IFNAMSIZ);

        uplink = dev_get_by_name(&init_net, ifname);
        if (!uplink)
                return -ENODEV;

        ctx = caiman_net_ctx_alloc(vm_id, mac, uplink);
        dev_put(uplink);

        return IS_ERR(ctx) ? PTR_ERR(ctx) : 0;
}

static int cmd_vm_del(struct sk_buff *skb, struct genl_info *info)
{
        struct caiman_net_ctx *ctx;
        u32 vm_id;

        if (!info->attrs[KVM_NET_ATTR_VM_ID])
                return -EINVAL;

        vm_id = nla_get_u32(info->attrs[KVM_NET_ATTR_VM_ID]);
        ctx = caiman_net_ctx_find(vm_id);
        if (!ctx)
                return -ENOENT;

        caiman_net_xdp_detach(ctx);
        caiman_net_ctx_free(ctx);
        return 0;
}

static int cmd_vm_stats(struct sk_buff *skb, struct genl_info *info)
{
        struct caiman_net_ctx *ctx;
        struct sk_buff     *reply;
        void               *hdr;
        struct nlattr      *nest;
        u32 vm_id;

        if (!info->attrs[KVM_NET_ATTR_VM_ID])
                return -EINVAL;

        vm_id = nla_get_u32(info->attrs[KVM_NET_ATTR_VM_ID]);
        ctx = caiman_net_ctx_find(vm_id);
        if (!ctx)
                return -ENOENT;

        reply = genlmsg_new(NLMSG_DEFAULT_SIZE, GFP_KERNEL);
        if (!reply)
                return -ENOMEM;

        hdr = genlmsg_put_reply(reply, info, &caiman_net_family,
                                0, KVM_NET_CMD_VM_STATS);
        if (!hdr) {
                nlmsg_free(reply);
                return -EMSGSIZE;
        }

        nla_put_u32(reply, KVM_NET_ATTR_VM_ID, vm_id);

        nest = nla_nest_start(reply, KVM_NET_ATTR_STATS);
        /* Encode as u64 pairs: rx_packets, tx_packets, rx_bytes, tx_bytes */
        nla_put_u64_64bit(reply, 1, atomic64_read(&ctx->rx_packets), 0);
        nla_put_u64_64bit(reply, 2, atomic64_read(&ctx->tx_packets), 0);
        nla_put_u64_64bit(reply, 3, atomic64_read(&ctx->rx_bytes),   0);
        nla_put_u64_64bit(reply, 4, atomic64_read(&ctx->tx_bytes),   0);
        nla_nest_end(reply, nest);

        genlmsg_end(reply, hdr);
        return genlmsg_reply(reply, info);
}

static int cmd_xdp_attach(struct sk_buff *skb, struct genl_info *info)
{
        struct caiman_net_ctx *ctx;
        char obj_path[PATH_MAX];
        u32 vm_id;

        if (!info->attrs[KVM_NET_ATTR_VM_ID] ||
            !info->attrs[KVM_NET_ATTR_BPF_OBJ])
                return -EINVAL;

        vm_id = nla_get_u32(info->attrs[KVM_NET_ATTR_VM_ID]);
        nla_strlcpy(obj_path, info->attrs[KVM_NET_ATTR_BPF_OBJ], PATH_MAX);

        ctx = caiman_net_ctx_find(vm_id);
        if (!ctx)
                return -ENOENT;

        return caiman_net_xdp_attach(ctx, obj_path);
}

static int cmd_xdp_detach(struct sk_buff *skb, struct genl_info *info)
{
        struct caiman_net_ctx *ctx;
        u32 vm_id;

        if (!info->attrs[KVM_NET_ATTR_VM_ID])
                return -EINVAL;

        vm_id = nla_get_u32(info->attrs[KVM_NET_ATTR_VM_ID]);
        ctx = caiman_net_ctx_find(vm_id);
        if (!ctx)
                return -ENOENT;

        caiman_net_xdp_detach(ctx);
        return 0;
}

/* ------------------------------------------------------------------ */
/*  Operations table                                                    */
/* ------------------------------------------------------------------ */

static const struct genl_ops caiman_net_ops[] = {
        {
                .cmd    = KVM_NET_CMD_VM_ADD,
                .doit   = cmd_vm_add,
                .flags  = GENL_ADMIN_PERM,
        },
        {
                .cmd    = KVM_NET_CMD_VM_DEL,
                .doit   = cmd_vm_del,
                .flags  = GENL_ADMIN_PERM,
        },
        {
                .cmd    = KVM_NET_CMD_VM_STATS,
                .doit   = cmd_vm_stats,
        },
        {
                .cmd    = KVM_NET_CMD_XDP_ATTACH,
                .doit   = cmd_xdp_attach,
                .flags  = GENL_ADMIN_PERM,
        },
        {
                .cmd    = KVM_NET_CMD_XDP_DETACH,
                .doit   = cmd_xdp_detach,
                .flags  = GENL_ADMIN_PERM,
        },
};

static struct genl_family caiman_net_family __ro_after_init = {
        .name     = KVM_NET_GENL_NAME,
        .version  = KVM_NET_GENL_VER,
        .maxattr  = KVM_NET_ATTR_MAX,
        .policy   = caiman_net_policy,
        .ops      = caiman_net_ops,
        .n_ops    = ARRAY_SIZE(caiman_net_ops),
        .module   = THIS_MODULE,
};

int caiman_net_netlink_init(void)
{
        return genl_register_family(&caiman_net_family);
}

void caiman_net_netlink_exit(void)
{
        genl_unregister_family(&caiman_net_family);
}
