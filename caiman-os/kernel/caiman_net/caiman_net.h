/* SPDX-License-Identifier: GPL-2.0-only */
#ifndef _KVM_NET_MOD_H
#define _KVM_NET_MOD_H

#include <linux/hashtable.h>
#include <linux/atomic.h>
#include <linux/spinlock.h>
#include <linux/netdevice.h>
#include <linux/bpf.h>
#include <linux/vhost.h>

#define KVM_NET_HT_BITS     8   /* 256-bucket hashtable */
#define KVM_NET_MAX_VMS     256
#define KVM_NET_GENL_NAME   "caiman_net"
#define KVM_NET_GENL_VER    1

/* Per-VM networking context, lives entirely in kernel space */
struct caiman_net_ctx {
        u32                  vm_id;
        u8                   mac[ETH_ALEN];
        struct net_device   *uplink;      /* physical NIC for XDP redirect  */
        struct net_device   *tap_dev;     /* tap device for the guest        */
        struct bpf_prog     *xdp_prog;    /* XDP program pinned to uplink    */
        int                  xdp_map_fd;  /* BPF map: mac -> redirect target */

        /* Stats (updated lock-free via atomics) */
        atomic64_t           rx_packets;
        atomic64_t           tx_packets;
        atomic64_t           rx_bytes;
        atomic64_t           tx_bytes;

        spinlock_t           lock;
        struct hlist_node    hnode;       /* vm_net_table bucket             */
};

/* Netlink attribute IDs (shared with userspace Rust CNI/VMM) */
enum caiman_net_attr {
        KVM_NET_ATTR_UNSPEC,
        KVM_NET_ATTR_VM_ID,       /* u32  */
        KVM_NET_ATTR_MAC,         /* ETH_ALEN bytes */
        KVM_NET_ATTR_UPLINK,      /* string: "eth0" */
        KVM_NET_ATTR_BPF_OBJ,    /* path to pinned BPF object */
        KVM_NET_ATTR_STATS,       /* nested: rx/tx packets+bytes */
        __KVM_NET_ATTR_MAX,
};
#define KVM_NET_ATTR_MAX (__KVM_NET_ATTR_MAX - 1)

/* Netlink commands */
enum caiman_net_cmd {
        KVM_NET_CMD_UNSPEC,
        KVM_NET_CMD_VM_ADD,       /* add VM network context   */
        KVM_NET_CMD_VM_DEL,       /* remove VM network context */
        KVM_NET_CMD_VM_STATS,     /* query stats              */
        KVM_NET_CMD_XDP_ATTACH,   /* attach XDP prog to NIC   */
        KVM_NET_CMD_XDP_DETACH,
        __KVM_NET_CMD_MAX,
};
#define KVM_NET_CMD_MAX (__KVM_NET_CMD_MAX - 1)

/* Exported by main.c */
struct caiman_net_ctx *caiman_net_ctx_alloc(u32 vm_id, const u8 *mac,
                                      struct net_device *uplink);
void                caiman_net_ctx_free(struct caiman_net_ctx *ctx);
struct caiman_net_ctx *caiman_net_ctx_find(u32 vm_id);
int                 caiman_net_kick_tx(struct caiman_net_ctx *ctx,
                                    struct vhost_virtqueue *vq);

/* Exported by netlink.c */
int  caiman_net_netlink_init(void);
void caiman_net_netlink_exit(void);

/* Exported by xdp_loader.c */
int  caiman_net_xdp_init(void);
void caiman_net_xdp_exit(void);
int  caiman_net_xdp_attach(struct caiman_net_ctx *ctx, const char *bpf_obj_path);
void caiman_net_xdp_detach(struct caiman_net_ctx *ctx);

#endif /* _KVM_NET_MOD_H */
