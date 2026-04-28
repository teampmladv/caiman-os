//! xdp.rs — XDP map updates for CNI add/del operations
//! Updates the caiman_net BPF maps when a pod interface is created.
use anyhow::Result;

/// Register a new pod interface in the XDP mac→ifindex map
pub fn register_pod(mac: &str, ifindex: u32, vm_id: u32) -> Result<()> {
    // In production: update BPF map via bpf() syscall or bpftool
    // For now: write to sysfs if caiman_net.ko is loaded
    let sysfs = format!("/sys/module/caiman_net/maps/mac_to_ifindex/{mac}");
    if std::path::Path::new("/sys/module/caiman_net").exists() {
        let _ = std::fs::write(&sysfs, format!("{ifindex},{vm_id}"));
    }
    Ok(())
}

pub fn unregister_pod(mac: &str) -> Result<()> {
    let sysfs = format!("/sys/module/caiman_net/maps/mac_to_ifindex/{mac}");
    let _ = std::fs::remove_file(&sysfs);
    Ok(())
}
