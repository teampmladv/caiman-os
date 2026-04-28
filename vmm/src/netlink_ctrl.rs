//! vmm/src/netlink_ctrl.rs — Rust client for the caiman_net_mod Generic Netlink family
//!
//! Mirrors the attribute IDs and command numbers from kernel/caiman_net_mod/caiman_net_mod.h
//! Uses the `genetlink` crate to send/receive Generic Netlink messages.

use anyhow::{bail, Context, Result};
use genetlink::GenetlinkHandle;
use netlink_proto::sys::{AsyncSocket, SocketAddr};
use tracing::debug;

// ── Attribute IDs (must match caiman_net_mod.h) ──────────────────────────────

const KVM_NET_ATTR_VM_ID:   u16 = 1;
const KVM_NET_ATTR_MAC:     u16 = 2;
const KVM_NET_ATTR_UPLINK:  u16 = 3;
const KVM_NET_ATTR_BPF_OBJ: u16 = 4;
const KVM_NET_ATTR_STATS:   u16 = 5;

// ── Command IDs (must match caiman_net_mod.h) ────────────────────────────────

const KVM_NET_CMD_VM_ADD:    u8 = 1;
const KVM_NET_CMD_VM_DEL:    u8 = 2;
const KVM_NET_CMD_VM_STATS:  u8 = 3;
const KVM_NET_CMD_XDP_ATTACH:u8 = 4;
const KVM_NET_CMD_XDP_DETACH:u8 = 5;

const KVM_NET_GENL_NAME: &str = "caiman_net";

// ── Public API ─────────────────────────────────────────────────────────────

/// Register a new VM network context with the kernel module.
pub async fn vm_add(vm_id: u32, mac: &[u8; 6], uplink: &str) -> Result<()> {
        let mut handle = open_socket().await?;
        let family_id  = resolve_family(&mut handle).await?;

        use netlink_packet_generic::GenlMessage;
        use netlink_packet_utils::nla::DefaultNla;

        let attrs = vec![
                DefaultNla::new(KVM_NET_ATTR_VM_ID,  vm_id.to_le_bytes().to_vec()),
                DefaultNla::new(KVM_NET_ATTR_MAC,    mac.to_vec()),
                DefaultNla::new(KVM_NET_ATTR_UPLINK, uplink_bytes(uplink)),
        ];

        send_cmd(&mut handle, family_id, KVM_NET_CMD_VM_ADD, attrs)
                .await
                .context("KVM_NET_CMD_VM_ADD")?;

        debug!("netlink: vm_add vm_id={vm_id} mac={} uplink={uplink}",
               fmt_mac(mac));
        Ok(())
}

/// Remove a VM network context from the kernel module.
pub async fn vm_del(vm_id: u32) -> Result<()> {
        let mut handle = open_socket().await?;
        let family_id  = resolve_family(&mut handle).await?;

        use netlink_packet_utils::nla::DefaultNla;
        let attrs = vec![
                DefaultNla::new(KVM_NET_ATTR_VM_ID, vm_id.to_le_bytes().to_vec()),
        ];

        send_cmd(&mut handle, family_id, KVM_NET_CMD_VM_DEL, attrs)
                .await
                .context("KVM_NET_CMD_VM_DEL")?;
        debug!("netlink: vm_del vm_id={vm_id}");
        Ok(())
}

/// Attach the XDP program at `bpf_path` to the VM's uplink NIC.
pub async fn xdp_attach(vm_id: u32, bpf_path: &str) -> Result<()> {
        let mut handle = open_socket().await?;
        let family_id  = resolve_family(&mut handle).await?;

        use netlink_packet_utils::nla::DefaultNla;
        let attrs = vec![
                DefaultNla::new(KVM_NET_ATTR_VM_ID,   vm_id.to_le_bytes().to_vec()),
                DefaultNla::new(KVM_NET_ATTR_BPF_OBJ, path_bytes(bpf_path)),
        ];

        send_cmd(&mut handle, family_id, KVM_NET_CMD_XDP_ATTACH, attrs)
                .await
                .context("KVM_NET_CMD_XDP_ATTACH")?;
        debug!("netlink: xdp_attach vm_id={vm_id} path={bpf_path}");
        Ok(())
}

/// Detach the XDP program from the VM's uplink NIC.
pub async fn xdp_detach(vm_id: u32) -> Result<()> {
        let mut handle = open_socket().await?;
        let family_id  = resolve_family(&mut handle).await?;

        use netlink_packet_utils::nla::DefaultNla;
        let attrs = vec![
                DefaultNla::new(KVM_NET_ATTR_VM_ID, vm_id.to_le_bytes().to_vec()),
        ];

        send_cmd(&mut handle, family_id, KVM_NET_CMD_XDP_DETACH, attrs)
                .await
                .context("KVM_NET_CMD_XDP_DETACH")?;
        debug!("netlink: xdp_detach vm_id={vm_id}");
        Ok(())
}

/// Query per-VM RX/TX statistics from the kernel module.
pub async fn vm_stats(vm_id: u32) -> Result<VmStats> {
        // Full implementation parses the KVM_NET_ATTR_STATS nested NLA.
        // Stub returns zeros for now.
        let _ = vm_id;
        Ok(VmStats::default())
}

#[derive(Debug, Default)]
pub struct VmStats {
        pub rx_packets: u64,
        pub tx_packets: u64,
        pub rx_bytes:   u64,
        pub tx_bytes:   u64,
}

// ── Private helpers ────────────────────────────────────────────────────────

async fn open_socket() -> Result<GenetlinkHandle> {
        let (conn, handle, _) = genetlink::new_connection()?;
        tokio::spawn(conn);
        Ok(handle)
}

async fn resolve_family(handle: &mut GenetlinkHandle) -> Result<u16> {
        handle
                .resolve_family_id(KVM_NET_GENL_NAME)
                .await
                .with_context(|| format!(
                        "caiman_net_mod not loaded? Couldn't resolve genl family '{KVM_NET_GENL_NAME}'"
                ))
}

async fn send_cmd(
        handle: &mut GenetlinkHandle,
        family_id: u16,
        cmd: u8,
        _attrs: Vec<netlink_packet_utils::nla::DefaultNla>,
) -> Result<()> {
        // Full implementation encodes attrs into a GenlMessage and calls
        // handle.request(msg).await. Stub here for compilation.
        let _ = (handle, family_id, cmd);
        Ok(())
}

fn uplink_bytes(s: &str) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.push(0); // NUL-terminate for NLA_STRING
        v
}

fn path_bytes(s: &str) -> Vec<u8> {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        v
}

fn fmt_mac(mac: &[u8; 6]) -> String {
        mac.map(|b| format!("{b:02x}")).join(":")
}
