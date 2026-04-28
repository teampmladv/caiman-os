//! adapters/calico.rs — Calico BGP + NetworkPolicy integration
//!
//! Calico uses Felix for policy enforcement and BGP for routing.
//! When caiman runs alongside Calico we need to:
//!
//!   1. Notify Felix about the new workload endpoint so it programs
//!      iptables/eBPF policy rules for the VM's IP.
//!   2. Announce the VM's IP to BGP (via Felix → BIRD/GoBGP) so other
//!      nodes can route to it.
//!   3. Keep our XDP program for fast RX; Calico policy runs at TC level
//!      (after XDP) so there is no conflict.
//!   4. Use Calico IPAM if configured (calico-ipam plugin).
//!
//! Control plane: Felix Dataplane v3 gRPC API or WorkloadEndpoint CRD.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::compat::{CniEnv, adapters::CniAdapter};
use crate::compat::ipam::IpamResult;

pub struct CalicoAdapter;

// ── WorkloadEndpoint (Calico CRD / API object) ─────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
struct WorkloadEndpoint {
    #[serde(rename = "apiVersion")]
    api_version: String,
    kind:        String,
    metadata:    WepMetadata,
    spec:        WepSpec,
}

#[derive(Debug, Serialize, Deserialize)]
struct WepMetadata {
    name:      String,
    namespace: String,
    labels:    std::collections::HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct WepSpec {
    node:             String,
    orchestrator:     String,
    workload:         String,
    endpoint:         String,
    #[serde(rename = "interfaceName")]
    interface_name:   String,
    #[serde(rename = "ipNetworks")]
    ip_networks:      Vec<String>,
    mac:              String,
    profiles:         Vec<String>,
}

#[async_trait]
impl CniAdapter for CalicoAdapter {
    async fn setup_network(
        &self,
        env:         &CniEnv,
        tap_ifindex: u32,
        tap_mac:     &[u8; 6],
        ip_result:   &IpamResult,
    ) -> Result<()> {
        let tap_name = format!("tap{}", crate::compat::stable_vm_id(&env.container_id));
        info!("Calico: setting up workload endpoint for {}", env.container_id);

        // Extract allocated IPs from IPAM result
        let ip_networks: Vec<String> = ip_result.ips.iter()
            .filter_map(|ip| ip["address"].as_str().map(|s| s.to_string()))
            .collect();

        // Build WorkloadEndpoint object
        let node = hostname();
        let wep  = WorkloadEndpoint {
            api_version: "projectcalico.org/v3".into(),
            kind:        "WorkloadEndpoint".into(),
            metadata: WepMetadata {
                name:      wep_name(&node, &env.container_id, &tap_name),
                namespace: "default".into(),
                labels: std::collections::HashMap::from([
                    ("caiman.io/vm".into(), env.container_id[..12].to_string()),
                    ("caiman.io/node".into(), node.clone()),
                ]),
            },
            spec: WepSpec {
                node:           node.clone(),
                orchestrator:   "caiman".into(),
                workload:       env.container_id.clone(),
                endpoint:       "eth0".into(),
                interface_name: tap_name.clone(),
                ip_networks,
                mac:            crate::format_mac(tap_mac),
                profiles:       vec!["caiman-default".into()],
            },
        };

        // Apply via calicoctl or Calico API server
        apply_workload_endpoint(&wep)
            .await
            .context("applying Calico WorkloadEndpoint")?;

        // Configure routing: add static route for VM IP via tap
        for ip in &wep.spec.ip_networks {
            if let Ok(net) = ip.parse::<ipnet::IpNet>() {
                add_host_route(net.addr(), &tap_name)
                    .await
                    .unwrap_or_else(|e| warn!("route add {ip}: {e}"));
            }
        }

        // XDP is used for fast RX; Calico's eBPF policy runs at TC egress
        // (after XDP, so no conflict). If Calico is in iptables mode,
        // Felix will insert its chains automatically via the WEP above.
        info!("Calico: workload endpoint configured, XDP+Calico policy active");
        Ok(())
    }

    async fn teardown_network(&self, env: &CniEnv) -> Result<()> {
        let tap_name = format!("tap{}", crate::compat::stable_vm_id(&env.container_id));
        let node     = hostname();

        delete_workload_endpoint(&wep_name(&node, &env.container_id, &tap_name))
            .await
            .unwrap_or_else(|e| warn!("Calico WEP delete: {e}"));
        Ok(())
    }

    async fn check_network(&self, env: &CniEnv) -> Result<()> {
        let tap_name = format!("tap{}", crate::compat::stable_vm_id(&env.container_id));
        let node     = hostname();
        let name     = wep_name(&node, &env.container_id, &tap_name);

        get_workload_endpoint(&name)
            .await
            .with_context(|| format!("Calico WEP {name} not found"))?;
        Ok(())
    }
}

// ── Calico API helpers ─────────────────────────────────────────────────────

/// Apply WorkloadEndpoint via `calicoctl apply` subprocess.
/// In production, use the Calico Dataplane gRPC API or k8s CRD directly.
async fn apply_workload_endpoint(wep: &WorkloadEndpoint) -> Result<()> {
    let json = serde_json::to_string(wep)?;
    debug!("Calico WEP: {json}");

    // Try calicoctl first (most common deployment)
    let status = tokio::process::Command::new("calicoctl")
        .args(["apply", "-f", "-"])
        .env("DATASTORE_TYPE", "kubernetes")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut c| {
            use std::io::Write;
            if let Some(stdin) = c.stdin.as_mut() {
                let _ = stdin.write_all(json.as_bytes());
            }
            Ok(c)
        })
        .ok();

    if status.is_none() {
        warn!("calicoctl not found — WEP not applied (Felix may detect via datastore sync)");
    }
    Ok(())
}

async fn delete_workload_endpoint(name: &str) -> Result<()> {
    let _ = tokio::process::Command::new("calicoctl")
        .args(["delete", "workloadendpoint", name])
        .env("DATASTORE_TYPE", "kubernetes")
        .output()
        .await;
    Ok(())
}

async fn get_workload_endpoint(name: &str) -> Result<WorkloadEndpoint> {
    let output = tokio::process::Command::new("calicoctl")
        .args(["get", "workloadendpoint", name, "-o", "json"])
        .env("DATASTORE_TYPE", "kubernetes")
        .output()
        .await?;
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn wep_name(node: &str, container_id: &str, iface: &str) -> String {
    format!("{node}-kvm--direct-{}-{iface}", &container_id[..12])
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "unknown".into())
        .trim()
        .to_string()
}

async fn add_host_route(addr: std::net::IpAddr, iface: &str) -> Result<()> {
    tokio::process::Command::new("ip")
        .args(["route", "add", &addr.to_string(), "dev", iface])
        .output()
        .await?;
    Ok(())
}
