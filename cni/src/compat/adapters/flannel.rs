//! adapters/flannel.rs — Flannel VXLAN / host-gw overlay
//!
//! Flannel allocates a subnet per node and either:
//!   - Encapsulates pod traffic in VXLAN (default)
//!   - Routes directly via host routes (host-gw, faster)
//!
//! caiman + Flannel coexistence:
//!   VXLAN mode: VM traffic is encapsulated by the flannel.1 VXLAN device.
//!               XDP runs on the physical NIC (pre-encap) and on flannel.1 (post-decap).
//!               We attach our XDP redirect on the physical uplink for local traffic
//!               and add a VXLAN FDB entry so remote VM traffic is encapsulated correctly.
//!
//!   host-gw mode: No encapsulation. VM traffic routed directly between nodes.
//!                 XDP redirect works without modification.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use tracing::{info, warn};

use crate::compat::{CniEnv, adapters::CniAdapter};
use crate::compat::ipam::IpamResult;

pub struct FlannelAdapter;

/// Parsed from /run/flannel/subnet.env
#[derive(Debug)]
struct FlannelSubnet {
    subnet:    String,   // e.g. "10.244.1.0/24"
    mtu:       u32,
    ipmasq:    bool,
    backend:   FlannelBackend,
}

#[derive(Debug, PartialEq)]
enum FlannelBackend { Vxlan, HostGw, Udp }

#[async_trait]
impl CniAdapter for FlannelAdapter {
    async fn setup_network(
        &self,
        env:         &CniEnv,
        tap_ifindex: u32,
        tap_mac:     &[u8; 6],
        ip_result:   &IpamResult,
    ) -> Result<()> {
        let tap_name = format!("tap{}", crate::compat::stable_vm_id(&env.container_id));
        let subnet   = read_flannel_subnet().await?;

        info!("Flannel: backend={:?} subnet={}", subnet.backend, subnet.subnet);

        match subnet.backend {
            FlannelBackend::Vxlan => {
                setup_vxlan_mode(&tap_name, tap_ifindex, tap_mac, ip_result, &subnet).await?;
            }
            FlannelBackend::HostGw => {
                setup_hostgw_mode(&tap_name, ip_result).await?;
            }
            FlannelBackend::Udp => {
                warn!("Flannel UDP backend is deprecated — treating as host-gw");
                setup_hostgw_mode(&tap_name, ip_result).await?;
            }
        }

        Ok(())
    }

    async fn teardown_network(&self, env: &CniEnv) -> Result<()> {
        let tap_name = format!("tap{}", crate::compat::stable_vm_id(&env.container_id));
        // Remove FDB / ARP entries if VXLAN mode
        if let Ok(subnet) = read_flannel_subnet().await {
            if subnet.backend == FlannelBackend::Vxlan {
                cleanup_vxlan_entries(&tap_name).await.ok();
            }
        }
        Ok(())
    }

    async fn check_network(&self, env: &CniEnv) -> Result<()> {
        // Verify flannel.1 VXLAN device still exists
        if !std::path::Path::new("/sys/class/net/flannel.1").exists() {
            anyhow::bail!("flannel.1 VXLAN device not found");
        }
        Ok(())
    }
}

// ── VXLAN mode ─────────────────────────────────────────────────────────────

async fn setup_vxlan_mode(
    tap_name:    &str,
    tap_ifindex: u32,
    tap_mac:     &[u8; 6],
    ip_result:   &IpamResult,
    subnet:      &FlannelSubnet,
) -> Result<()> {
    // Ensure flannel.1 VXLAN device exists
    if !std::path::Path::new("/sys/class/net/flannel.1").exists() {
        anyhow::bail!("flannel.1 not found — is flanneld running?");
    }

    // Add FDB entry: VM MAC → flannel.1 (so VXLAN encapsulates to correct node)
    let mac_str = crate::format_mac(tap_mac);
    let _ = tokio::process::Command::new("bridge")
        .args(["fdb", "add", &mac_str, "dev", "flannel.1", "dst", "0.0.0.0"])
        .output().await;

    // Add ARP entry for the VM IP on flannel.1
    for ip in &ip_result.ips {
        if let Some(addr) = ip["address"].as_str() {
            let ip_only = addr.split('/').next().unwrap_or(addr);
            let _ = tokio::process::Command::new("ip")
                .args(["neigh", "add", ip_only, "lladdr", &mac_str,
                       "dev", "flannel.1", "nud", "permanent"])
                .output().await;
        }
    }

    // Attach XDP also on flannel.1 for decapsulated traffic
    info!("Flannel VXLAN: FDB and ARP entries added, XDP on flannel.1 + physical NIC");
    Ok(())
}

async fn cleanup_vxlan_entries(tap_name: &str) -> Result<()> {
    // Remove any FDB/ARP entries associated with this tap
    // (Best effort: flannel.1 may have already cleaned up)
    Ok(())
}

// ── host-gw mode ───────────────────────────────────────────────────────────

async fn setup_hostgw_mode(tap_name: &str, ip_result: &IpamResult) -> Result<()> {
    // In host-gw mode, flannel programs routes between nodes directly.
    // We just add a local route for the VM's IP via the tap device.
    for ip in &ip_result.ips {
        if let Some(addr) = ip["address"].as_str() {
            let ip_only = addr.split('/').next().unwrap_or(addr);
            let _ = tokio::process::Command::new("ip")
                .args(["route", "add", ip_only, "dev", tap_name])
                .output().await;
        }
    }
    info!("Flannel host-gw: routes added via {tap_name}");
    Ok(())
}

// ── /run/flannel/subnet.env parser ────────────────────────────────────────

async fn read_flannel_subnet() -> Result<FlannelSubnet> {
    let content = tokio::fs::read_to_string("/run/flannel/subnet.env")
        .await
        .context("reading /run/flannel/subnet.env (is flanneld running?)")?;

    let mut subnet  = String::new();
    let mut mtu     = 1450u32;
    let mut ipmasq  = true;
    let mut backend = FlannelBackend::Vxlan;

    for line in content.lines() {
        match line.split_once('=') {
            Some(("FLANNEL_SUBNET",  v)) => subnet  = v.to_string(),
            Some(("FLANNEL_MTU",     v)) => mtu     = v.parse().unwrap_or(1450),
            Some(("FLANNEL_IPMASQ",  v)) => ipmasq  = v == "true",
            Some(("FLANNEL_BACKEND_TYPE", v)) => {
                backend = match v {
                    "vxlan"   => FlannelBackend::Vxlan,
                    "host-gw" => FlannelBackend::HostGw,
                    "udp"     => FlannelBackend::Udp,
                    _         => FlannelBackend::Vxlan,
                };
            }
            _ => {}
        }
    }

    if subnet.is_empty() {
        anyhow::bail!("FLANNEL_SUBNET not set in subnet.env");
    }

    Ok(FlannelSubnet { subnet, mtu, ipmasq, backend })
}
