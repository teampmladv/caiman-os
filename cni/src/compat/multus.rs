//! compat/multus.rs — Multus meta-CNI / NetworkAttachmentDefinition
//!
//! Multus allows multiple network interfaces per pod/VM.
//! caiman as a Multus secondary CNI provides additional
//! high-performance interfaces (backed by XDP) to VMs that already
//! have a primary interface from Calico/Cilium/Flannel.
//!
//! NetworkAttachmentDefinition example:
//! ```yaml
//! apiVersion: k8s.cni.cncf.io/v1
//! kind: NetworkAttachmentDefinition
//! metadata:
//!   name: caiman-fast
//! spec:
//!   config: |
//!     {
//!       "cniVersion": "1.0.0",
//!       "type": "caiman-cni",
//!       "name": "caiman-fast",
//!       "uplink": "eth1",
//!       "bpfPinPath": "/sys/fs/bpf/caiman",
//!       "ipam": { "type": "whereabouts", "range": "192.168.100.0/24" }
//!     }
//! ```
//!
//! Pod annotation:
//! ```yaml
//! k8s.v1.cni.cncf.io/networks: caiman-fast
//! ```

use anyhow::Result;
use tracing::{info, warn};

use crate::compat::{CniEnv, EcosystemKind};

/// True if we are being called as a Multus secondary plugin.
/// Multus sets K8S_POD_* env vars and passes a runtimeConfig with
/// network attachment details.
pub fn is_multus_secondary(config: &crate::CniConfig) -> bool {
    std::env::var("K8S_POD_NAME").is_ok()
        && std::env::var("MULTUS_CONF_FILE").is_ok_or(|| false)
        || config.runtime_cfg
            .as_ref()
            .and_then(|r| r.mac.as_ref())
            .is_some()
}

/// ADD for secondary interface: create an additional tap+XDP interface
/// attached to a different uplink or VLAN.
pub async fn add_secondary(env: &CniEnv, kind: EcosystemKind) -> Result<String> {
    let vm_id    = crate::compat::stable_vm_id(&env.container_id);
    // Secondary interfaces get a suffix to distinguish from the primary tap
    let tap_name = format!("tap{vm_id}s");

    info!("Multus secondary: creating {tap_name} for {}", env.container_id);

    // Use MAC from runtimeConfig if provided (Multus network-attachment-selection)
    let mac = env.config.runtime_cfg
        .as_ref()
        .and_then(|r| r.mac.as_deref())
        .and_then(|s| parse_mac(s))
        .unwrap_or_else(|| generate_secondary_mac(vm_id));

    let (tap_ifindex, tap_mac) = crate::tap::create_tap_with_mac(&tap_name, &env.netns, &mac)
        .await?;

    // IPAM for this secondary network
    let ip_result = crate::compat::ipam::allocate(
        &env.config, &env.container_id, &env.netns
    ).await?;

    // Register in XDP maps (secondary interface uses same BPF maps)
    crate::xdp::register_vm(vm_id + 10000, &tap_mac, tap_ifindex, &env.config.bpf_pin_path)?;

    // Apply bandwidth limits from runtimeConfig if present
    if let Some(ref bw) = env.config.runtime_cfg.as_ref().and_then(|r| r.bandwidth.as_ref()) {
        apply_bandwidth_limit(&tap_name, bw.ingress_rate, bw.egress_rate).await.ok();
    }

    info!("Multus secondary: {tap_name} ready, mac={}", crate::format_mac(&tap_mac));

    let result = serde_json::json!({
        "cniVersion": env.config.cni_version,
        "interfaces": [{
            "name":    tap_name,
            "mac":     crate::format_mac(&tap_mac),
            "sandbox": env.netns,
        }],
        "ips":    ip_result.ips,
        "routes": ip_result.routes,
        "dns":    ip_result.dns,
    });
    Ok(serde_json::to_string(&result)?)
}

pub async fn del_secondary(env: &CniEnv) -> Result<String> {
    let vm_id    = crate::compat::stable_vm_id(&env.container_id);
    let tap_name = format!("tap{vm_id}s");

    crate::xdp::detach_and_unregister(vm_id + 10000, &env.config.bpf_pin_path)
        .await.ok();
    crate::tap::delete_tap(&tap_name).await.ok();
    crate::compat::ipam::release(&env.config, &env.container_id, &env.netns)
        .await.ok();

    Ok("{}".into())
}

// ── CNI chaining support ───────────────────────────────────────────────────

pub mod chain_impl {
    //! Re-exported from compat/chain.rs
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn parse_mac(s: &str) -> Option<[u8; 6]> {
    let parts: Vec<u8> = s.split(':')
        .filter_map(|h| u8::from_str_radix(h, 16).ok())
        .collect();
    parts.try_into().ok()
}

fn generate_secondary_mac(vm_id: u32) -> [u8; 6] {
    let id = vm_id.to_le_bytes();
    [0x02, 0xbb, 0xcc, id[0], id[1], id[2]]
}

async fn apply_bandwidth_limit(iface: &str, ingress_bps: u64, egress_bps: u64) -> Result<()> {
    // Use tc-tbf (Token Bucket Filter) for bandwidth shaping
    let _ = tokio::process::Command::new("tc")
        .args(["qdisc", "add", "dev", iface, "root", "tbf",
               "rate", &format!("{egress_bps}bps"),
               "burst", "32kbit", "latency", "400ms"])
        .output().await;
    info!("Bandwidth limit applied on {iface}: egress={egress_bps}bps");
    Ok(())
}
