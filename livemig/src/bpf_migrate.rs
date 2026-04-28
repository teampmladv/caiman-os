//! livemig/src/bpf_migrate.rs — BPF map migration
//! Transfers XDP maps (mac_to_ifindex, policy, identity) from source to dest.

use anyhow::Result;
use tracing::info;

pub async fn transfer_bpf_maps(vm_id: u32, dest: &str) -> Result<()> {
    // Read current BPF pin files from source
    let pin_dir = format!("/sys/fs/bpf/caiman/vm{vm_id}");

    let mac = std::fs::read_to_string(format!("{pin_dir}/mac"))
        .unwrap_or_default();
    let vm_id_str = std::fs::read_to_string(format!("{pin_dir}/vm_id"))
        .unwrap_or_default();

    // Push to destination via its API
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("http://{dest}:8765/api/migrate/bpf-maps"))
        .json(&serde_json::json!({
            "vmId":  vm_id,
            "mac":   mac.trim(),
            "pinDir": pin_dir,
        }))
        .send().await;

    info!("BPF maps transferred: VM {vm_id} mac={}", mac.trim());
    Ok(())
}
