//! cni/src/bpf_maps.rs — Manage XDP BPF maps from the CNI plugin
//!
//! Populates the `mac_to_ifindex` and `tx_port` BPF maps that are pinned at
//! /sys/fs/bpf/caiman/ by the eBPF program Makefile (`make pin-maps`).
//! Uses libbpf-rs to update map entries without reloading the program.

use anyhow::{Context, Result};
use libbpf_rs::MapHandle;

const MAC_TO_IFINDEX_PIN: &str = "/sys/fs/bpf/caiman/mac_to_ifindex";
const TX_PORT_PIN:         &str = "/sys/fs/bpf/caiman/tx_port";

/// Insert `mac -> ifindex` into the XDP routing map and add
/// the tap device to the DEVMAP for XDP_REDIRECT.
pub fn add_vm_mac(mac: &[u8; 6], ifindex: u32, _bpf_pin_base: &str) -> Result<()> {
        // Open pinned mac_to_ifindex map
        let mac_map = MapHandle::from_pinned_path(MAC_TO_IFINDEX_PIN)
                .context("opening mac_to_ifindex map")?;

        mac_map
                .update(mac, &ifindex.to_le_bytes(), libbpf_rs::MapFlags::ANY)
                .context("inserting MAC entry")?;

        // Open pinned tx_port DEVMAP and add the tap ifindex
        let tx_map = MapHandle::from_pinned_path(TX_PORT_PIN)
                .context("opening tx_port devmap")?;

        // BPF_DEVMAP value: { ifindex: u32, bpf_prog_id: u32 } = 8 bytes
        let mut devmap_val = [0u8; 8];
        devmap_val[..4].copy_from_slice(&ifindex.to_le_bytes());
        tx_map
                .update(&ifindex.to_le_bytes(), &devmap_val, libbpf_rs::MapFlags::ANY)
                .context("inserting tx_port entry")?;

        Ok(())
}

/// Remove a VM's MAC entry from the XDP maps on CNI DEL.
pub fn del_vm_mac(mac: &[u8; 6], ifindex: u32) -> Result<()> {
        let mac_map = MapHandle::from_pinned_path(MAC_TO_IFINDEX_PIN)
                .context("opening mac_to_ifindex map")?;
        mac_map.delete(mac).ok(); // best-effort

        let tx_map = MapHandle::from_pinned_path(TX_PORT_PIN)
                .context("opening tx_port devmap")?;
        tx_map.delete(&ifindex.to_le_bytes()).ok();

        Ok(())
}
