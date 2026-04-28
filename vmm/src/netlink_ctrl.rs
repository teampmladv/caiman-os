//! vmm/src/netlink_ctrl.rs — communicate with caiman_net.ko
//! Uses tokio::process to delegate to caiman-netctl CLI (no genetlink dep).

use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::debug;

pub async fn vm_add(vm_id: u32, mac: &[u8; 6], uplink: &str) -> Result<()> {
    let mac_str = fmt_mac(mac);
    debug!("vm_add: vm_id={vm_id} mac={mac_str} uplink={uplink}");
    if which("caiman-netctl") {
        Command::new("caiman-netctl")
            .args(["vm-add", "--vm-id", &vm_id.to_string(), "--mac", &mac_str, "--uplink", uplink])
            .status().await.context("caiman-netctl vm-add")?;
    } else {
        tracing::warn!("caiman_net.ko not available — vm {vm_id} without XDP");
    }
    Ok(())
}

pub async fn vm_del(vm_id: u32) -> Result<()> {
    debug!("vm_del: vm_id={vm_id}");
    if which("caiman-netctl") {
        let _ = Command::new("caiman-netctl")
            .args(["vm-del", "--vm-id", &vm_id.to_string()])
            .status().await;
    }
    Ok(())
}

pub async fn xdp_attach(vm_id: u32, pin_path: &str) -> Result<()> {
    debug!("xdp_attach: vm_id={vm_id}");
    if which("caiman-netctl") {
        let _ = Command::new("caiman-netctl")
            .args(["xdp-attach", "--vm-id", &vm_id.to_string(), "--pin-path", pin_path])
            .status().await;
    }
    Ok(())
}

pub async fn xdp_detach(vm_id: u32) -> Result<()> {
    debug!("xdp_detach: vm_id={vm_id}");
    if which("caiman-netctl") {
        let _ = Command::new("caiman-netctl")
            .args(["xdp-detach", "--vm-id", &vm_id.to_string()])
            .status().await;
    }
    Ok(())
}

fn fmt_mac(mac: &[u8; 6]) -> String {
    mac.map(|b| format!("{b:02x}")).join(":")
}
fn which(cmd: &str) -> bool {
    std::process::Command::new("which").arg(cmd).output()
        .map(|o| o.status.success()).unwrap_or(false)
}
