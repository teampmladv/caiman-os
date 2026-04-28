//! collectors/xdp.rs — XDP per-VM stats via caiman_net Generic Netlink
//!
//! Queries the caiman_net kernel module for RX/TX counters per VM.
//! Falls back to reading pinned BPF maps directly via libbpf-rs.

use std::collections::HashMap;
use anyhow::Result;
use tracing::{debug, warn};

const BPF_PIN_BASE: &str = "/sys/fs/bpf/caiman";

#[derive(Debug, Default, Clone)]
pub struct XdpVmStats {
    pub rx_packets: u64,
    pub tx_packets: u64,
    pub rx_bytes:   u64,
    pub tx_bytes:   u64,
    pub rx_drops:   u64,
    pub rx_mbps:    f64,
    pub tx_mbps:    f64,
}

/// Collect XDP stats for all VMs. Returns map: vm_id → XdpVmStats.
pub async fn collect_all() -> Result<HashMap<String, XdpVmStats>> {
    // Try netlink first, fall back to BPF map read
    match collect_via_netlink().await {
        Ok(stats) => Ok(stats),
        Err(e) => {
            debug!("netlink XDP stats unavailable ({e}), reading BPF map directly");
            collect_via_bpf_map()
        }
    }
}

async fn collect_via_netlink() -> Result<HashMap<String, XdpVmStats>> {
    // Send KVM_NET_CMD_VM_STATS with wildcard vm_id=0
    // Full implementation uses the genetlink crate to talk to caiman_net.
    // Returns a multipart netlink response with KVM_NET_ATTR_STATS per VM.

    // Check if caiman_net genl family exists
    let output = tokio::process::Command::new("genl")
        .args(["ctrl", "getfamily", "caiman_net"])
        .output()
        .await?;

    if !output.status.success() {
        anyhow::bail!("caiman_net genl family not found");
    }

    // TODO: full genl implementation
    // For now, read from BPF maps
    collect_via_bpf_map()
}

fn collect_via_bpf_map() -> Result<HashMap<String, XdpVmStats>> {
    let map_path = format!("{BPF_PIN_BASE}/vm_rx_stats");
    let map = libbpf_rs::MapHandle::from_pinned_path(&map_path)
        .map_err(|e| anyhow::anyhow!("opening {map_path}: {e}"))?;

    let mut result = HashMap::new();
    let mut prev_keys: Option<Vec<u8>> = None;

    loop {
        // Iterate map keys
        let next = if let Some(ref k) = prev_keys {
            map.keys().skip_while(|k2| k2 == k.as_slice()).next()
        } else {
            map.keys().next()
        };

        let key = match next {
            Some(k) => k.to_vec(),
            None => break,
        };

        let ifindex = u32::from_le_bytes(
            key[..4].try_into().unwrap_or([0; 4])
        );

        // Read percpu values
        let vals = map.lookup_percpu(&key, libbpf_rs::MapFlags::ANY)
            .map_err(|e| anyhow::anyhow!("percpu lookup: {e}"))?;

        let (rx_packets, rx_bytes) = if let Some(percpu) = vals {
            percpu.iter().fold((0u64, 0u64), |(p, b), v| {
                let pkts  = u64::from_le_bytes(v.get(0..8).and_then(|s| s.try_into().ok()).unwrap_or([0;8]));
                let bytes = u64::from_le_bytes(v.get(8..16).and_then(|s| s.try_into().ok()).unwrap_or([0;8]));
                (p + pkts, b + bytes)
            })
        } else { (0, 0) };

        // Convert bytes to Mbps (approximate, single-sample)
        let rx_mbps = (rx_bytes * 8) as f64 / 1_000_000.0;

        // Map ifindex → vm_id via /sys/class/net/<iface>/ifindex
        let vm_id = ifindex_to_vm_id(ifindex);

        result.insert(vm_id, XdpVmStats {
            rx_packets,
            rx_bytes,
            rx_mbps,
            ..Default::default()
        });

        prev_keys = Some(key);
    }

    debug!("XDP collector: {} VM entries from BPF map", result.len());
    Ok(result)
}

fn ifindex_to_vm_id(ifindex: u32) -> String {
    // Look up /sys/class/net/*/ifindex to find the interface name,
    // then derive VM ID from the tap name convention: tap<vm_id>
    if let Ok(dir) = std::fs::read_dir("/sys/class/net") {
        for entry in dir.flatten() {
            if let Ok(idx_str) = std::fs::read_to_string(entry.path().join("ifindex")) {
                if idx_str.trim().parse::<u32>().ok() == Some(ifindex) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if let Some(id_str) = name.strip_prefix("tap") {
                        if let Ok(id) = id_str.parse::<u32>() {
                            return format!("vm-{id:03}");
                        }
                    }
                }
            }
        }
    }
    format!("ifindex-{ifindex}")
}
