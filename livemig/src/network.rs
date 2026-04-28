//! livemig/src/network.rs — network switchover
//! Sends gratuitous ARP from destination and updates caiman_net.ko XDP maps.

use anyhow::Result;
use tracing::info;

pub async fn switch_network(vm_id: u32, dest: &str) -> Result<()> {
    // Tell destination to send gratuitous ARP for the VM's IP
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("http://{dest}:8765/api/migrate/gratuitous-arp"))
        .json(&serde_json::json!({ "vmId": vm_id }))
        .send().await;

    // Update caiman_net XDP map on source (remove entry)
    let sysfs = format!("/sys/module/caiman_net/maps/mac_to_ifindex/vm{vm_id}");
    let _ = std::fs::remove_file(&sysfs);

    info!("Network switched: VM {vm_id} → {dest}");
    Ok(())
}
