//! bpf_maps.rs — BPF map helpers (no libbpf-rs dependency)
use anyhow::Result;

/// Update mac_to_ifindex map entry via bpftool subprocess
pub fn update_mac_map(pin_path: &str, mac: &[u8; 6], ifindex: u32) -> Result<()> {
    let mac_str = mac.map(|b| format!("{b:02x}")).join(":");
    let _ = std::process::Command::new("bpftool")
        .args(["map", "update", "pinned", &format!("{pin_path}/mac_to_ifindex"),
               "key", "hex", &mac_str,
               "value", "hex", &format!("{ifindex:08x}")])
        .status();
    Ok(())
}
