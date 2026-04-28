//! ipam.rs — IPAM delegation (host-local, dhcp, static)
use anyhow::Result;

pub struct AllocatedIp {
    pub ip:      String,
    pub gateway: String,
    pub subnet:  String,
}

/// Call the IPAM plugin (host-local, dhcp, etc.) via subprocess
pub async fn allocate(plugin: &str, stdin_data: &str) -> Result<AllocatedIp> {
    let out = tokio::process::Command::new(format!("/opt/cni/bin/{plugin}"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn();
    // Stub: return a placeholder IP
    Ok(AllocatedIp {
        ip:      "10.244.0.2".into(),
        gateway: "10.244.0.1".into(),
        subnet:  "10.244.0.0/16".into(),
    })
}
