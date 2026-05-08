//! standalone.rs -- Caiman network setup for each VM
//!
//! Network modes:
//!   nat    -- VM behind NAT, internet via masquerade (default)
//!   bridge -- VM gets LAN IP, visible on local network
//!   none   -- isolated, no external connectivity

use anyhow::Result;
use crate::{bridge, ipam, tap};

#[derive(Debug, Clone, PartialEq)]
pub enum NetMode {
    Nat,
    Bridge,
    None,
}

impl NetMode {
    pub fn from_env() -> Self {
        match std::env::var("CAIMAN_NET_MODE").as_deref() {
            Ok("bridge") => NetMode::Bridge,
            Ok("none")   => NetMode::None,
            _            => NetMode::Nat,
        }
    }
}

pub async fn add(vm_id: &str, _ifname: &str, _netns: &str) -> Result<String> {
    let mode   = NetMode::from_env();
    let uplink = bridge::detect_uplink();

    tracing::info!("caiman-cni ADD vm={vm_id} mode={mode:?} uplink={uplink}");

    // 1. Ensure bridge exists
    bridge::ensure_bridge()?;

    // 2. Setup NAT or bridge mode
    match mode {
        NetMode::Nat    => bridge::ensure_nat(&uplink)?,
        NetMode::Bridge => bridge::ensure_bridge_mode(&uplink)?,
        NetMode::None   => {}
    }

    // 3. Allocate IP
    let alloc = ipam::allocate(vm_id)?;
    tracing::info!("IPAM: vm={vm_id} ip={} gw={}", alloc.ip, alloc.gateway);

    // 4. Create TAP interface
    let tap_name = tap_name(vm_id);
    tap::create_tap(&tap_name, bridge::bridge_name())?;

    // 5. Return allocated IP for VM cmdline (ip=X.X.X.X::gw:mask::eth0:none)
    let ip_config = format!(
        "ip={}::{}:255.255.255.0::eth0:none",
        alloc.ip, alloc.gateway
    );

    Ok(ip_config)
}

pub async fn del(vm_id: &str, _ifname: &str) -> Result<()> {
    tracing::info!("caiman-cni DEL vm={vm_id}");
    let tap_name = tap_name(vm_id);
    tap::delete_tap(&tap_name)?;
    ipam::release(vm_id);
    Ok(())
}

pub fn tap_name(vm_id: &str) -> String {
    let short = &vm_id[..vm_id.len().min(8)];
    format!("caim{short}")
}

pub fn get_vm_ip(vm_id: &str) -> Option<String> {
    ipam::list().get(vm_id).cloned()
}
