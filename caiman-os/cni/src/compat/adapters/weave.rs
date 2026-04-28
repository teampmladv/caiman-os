//! adapters/weave.rs — Weave Net mesh overlay

use anyhow::Result;
use async_trait::async_trait;
use tracing::info;

use crate::compat::{CniEnv, adapters::CniAdapter};
use crate::compat::ipam::IpamResult;

pub struct WeaveAdapter;

#[async_trait]
impl CniAdapter for WeaveAdapter {
    async fn setup_network(
        &self, env: &CniEnv, tap_ifindex: u32, tap_mac: &[u8; 6], ip_result: &IpamResult,
    ) -> Result<()> {
        let tap_name = format!("tap{}", crate::compat::stable_vm_id(&env.container_id));
        info!("Weave: attaching {tap_name} to Weave mesh");

        // Attach tap to the Weave bridge (typically `weave`)
        attach_to_weave_bridge(&tap_name).await?;

        // Weave encrypts inter-node traffic via WireGuard (if sleeve/encrypted mode)
        // Our XDP program handles intra-node fast path; Weave handles inter-node tunneling
        info!("Weave: tap attached to mesh, XDP handles local fast path");
        Ok(())
    }

    async fn teardown_network(&self, env: &CniEnv) -> Result<()> {
        let tap_name = format!("tap{}", crate::compat::stable_vm_id(&env.container_id));
        detach_from_weave_bridge(&tap_name).await.ok();
        Ok(())
    }

    async fn check_network(&self, env: &CniEnv) -> Result<()> {
        if !std::path::Path::new("/sys/class/net/weave").exists() {
            anyhow::bail!("weave bridge not found");
        }
        Ok(())
    }
}

async fn attach_to_weave_bridge(tap_name: &str) -> anyhow::Result<()> {
    // Use `weave attach` or bridge addif
    let _ = tokio::process::Command::new("ip")
        .args(["link", "set", tap_name, "master", "weave"])
        .output().await;
    Ok(())
}

async fn detach_from_weave_bridge(tap_name: &str) -> anyhow::Result<()> {
    let _ = tokio::process::Command::new("ip")
        .args(["link", "set", tap_name, "nomaster"])
        .output().await;
    Ok(())
}
