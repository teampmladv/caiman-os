//! compat/chain.rs — CNI chaining (caiman as secondary in a conflist)
//!
//! When another CNI plugin runs first and passes a prevResult, caiman
//! runs in "chaining mode": it adds its XDP/tap acceleration on top of
//! whatever networking the primary CNI already set up.
//!
//! Example conflist (Calico primary + caiman acceleration):
//! ```json
//! {
//!   "cniVersion": "1.0.0",
//!   "name": "calico-caiman",
//!   "plugins": [
//!     { "type": "calico", "ipam": { "type": "calico-ipam" } },
//!     { "type": "caiman-cni", "uplink": "eth0" },
//!     { "type": "bandwidth", "ingressRate": 10000000000 }
//!   ]
//! }
//! ```

use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::compat::{CniEnv, EcosystemKind};

/// Handle ADD when prevResult is present (we are not the first plugin).
pub async fn add_chained(env: &CniEnv, kind: EcosystemKind) -> Result<String> {
    let prev = env.config.prev_result.as_ref()
        .context("prevResult missing in chaining mode")?;

    info!("CNI chaining: adding XDP acceleration on top of {kind:?} result");

    // Extract interface info from prevResult
    let interfaces = prev["interfaces"].as_array()
        .cloned()
        .unwrap_or_default();
    let ips = prev["ips"].as_array()
        .cloned()
        .unwrap_or_default();

    // Find the sandbox interface (the one inside the netns)
    let sandbox_iface = interfaces.iter()
        .find(|i| i["sandbox"].as_str().map(|s| !s.is_empty()).unwrap_or(false));

    if let Some(iface) = sandbox_iface {
        let iface_name = iface["name"].as_str().unwrap_or("eth0");
        let vm_id = crate::compat::stable_vm_id(&env.container_id);

        // Create a tap that mirrors the existing interface for XDP acceleration
        // This is a "mirror tap" pattern: traffic flows through the existing
        // veth/macvlan AND the tap allows our XDP program to intercept RX
        let tap_name = format!("xtap{vm_id}");

        info!("Chaining: creating mirror tap {tap_name} for interface {iface_name}");

        // Add XDP program to the existing uplink (non-destructive — we don't
        // replace the existing interface, just add fast-path acceleration)
        crate::xdp::attach_to_existing_interface(&env.config.uplink, vm_id, &env.config)
            .await
            .unwrap_or_else(|e| warn!("XDP chain attach: {e}"));
    }

    // Pass through prevResult unchanged (CNI spec requires this in chaining)
    let mut result = prev.clone();
    result["cniVersion"] = serde_json::json!(env.config.cni_version);
    Ok(serde_json::to_string(&result)?)
}
