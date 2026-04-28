//! compat/ipam.rs — Generic IPAM delegation
//!
//! Executes any IPAM plugin found in the CNI config's "ipam" block by
//! calling it as a subprocess (CNI spec §4.2).
//!
//! Supported IPAM plugins (auto-detected from config type):
//!   host-local       — static ranges on each node
//!   dhcp             — DHCP-based allocation
//!   calico-ipam      — Calico block affinity IPAM
//!   cilium-cni       — Cilium IPAM (cluster-pool / kubernetes)
//!   whereabouts      — cluster-wide range allocation
//!   static           — fixed IP from config
//!   flannel          — reads /run/flannel/subnet.env

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

use crate::CniConfig;

// ── IPAM result (subset of CNI result) ─────────────────────────────────────

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct IpamResult {
    #[serde(rename = "cniVersion")]
    pub cni_version: String,
    pub ips:    Vec<Value>,
    pub routes: Vec<Value>,
    pub dns:    Value,
}

// ── Public API ──────────────────────────────────────────────────────────────

/// Run the IPAM plugin specified in config.ipam and return the allocated IPs.
pub async fn allocate(
    config:       &CniConfig,
    container_id: &str,
    netns:        &str,
) -> Result<IpamResult> {
    let ipam_cfg = match &config.ipam {
        Some(v) => v.clone(),
        None => {
            info!("No IPAM config — returning empty result");
            return Ok(IpamResult {
                cni_version: config.cni_version.clone(),
                ..Default::default()
            });
        }
    };

    let plugin_type = ipam_cfg["type"]
        .as_str()
        .context("ipam.type missing")?
        .to_string();

    info!("IPAM allocate: plugin={plugin_type} container={container_id}");

    // Build the config to pass to the IPAM plugin
    let mut ipam_input = config_for_ipam(config, &ipam_cfg);

    // Special handling per IPAM type
    match plugin_type.as_str() {
        "flannel" => {
            // Flannel IPAM reads /run/flannel/subnet.env and fills in the range
            inject_flannel_subnet(&mut ipam_input).await?;
        }
        "calico-ipam" => {
            // Calico IPAM needs the Calico datastore env vars
            // These are typically set by the calico-node DaemonSet
        }
        _ => {}
    }

    let result = exec_ipam_plugin(&plugin_type, "ADD", &ipam_input, container_id, netns)
        .await
        .with_context(|| format!("IPAM plugin {plugin_type} ADD failed"))?;

    Ok(result)
}

/// Release IPs previously allocated.
pub async fn release(
    config:       &CniConfig,
    container_id: &str,
    netns:        &str,
) -> Result<()> {
    let ipam_cfg = match &config.ipam {
        Some(v) => v.clone(),
        None => return Ok(()),
    };

    let plugin_type = ipam_cfg["type"]
        .as_str()
        .context("ipam.type missing")?
        .to_string();

    let ipam_input = config_for_ipam(config, &ipam_cfg);
    exec_ipam_plugin(&plugin_type, "DEL", &ipam_input, container_id, netns)
        .await
        .map(|_| ())
        .or_else(|e| {
            // Best-effort: log but don't fail DEL
            debug!("IPAM DEL error (ignored): {e}");
            Ok(())
        })
}

// ── Subprocess execution ────────────────────────────────────────────────────

async fn exec_ipam_plugin(
    plugin_type:  &str,
    command:      &str,
    config_json:  &Value,
    container_id: &str,
    netns:        &str,
) -> Result<IpamResult> {
    // Search standard CNI bin dirs for the plugin binary
    let bin = find_cni_binary(plugin_type)
        .with_context(|| format!("IPAM binary not found: {plugin_type}"))?;

    debug!("Executing IPAM: {bin} (CMD={command})");

    let output = Command::new(&bin)
        .env("CNI_COMMAND",     command)
        .env("CNI_CONTAINERID", container_id)
        .env("CNI_NETNS",       netns)
        .env("CNI_IFNAME",      "eth0")
        .env("CNI_PATH",        cni_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?
        .wait_with_output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("IPAM plugin {plugin_type} exited {:?}: {stderr}", output.status.code());
    }

    if command == "DEL" {
        return Ok(IpamResult::default());
    }

    let stdout = String::from_utf8(output.stdout)?;
    debug!("IPAM result: {stdout}");
    let result: IpamResult = serde_json::from_str(&stdout)
        .with_context(|| format!("parsing IPAM result: {stdout}"))?;

    Ok(result)
}

fn find_cni_binary(name: &str) -> Option<String> {
    for dir in ["/opt/cni/bin", "/usr/libexec/cni", "/usr/local/lib/cni"] {
        let path = format!("{dir}/{name}");
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }
    None
}

fn cni_path() -> String {
    std::env::var("CNI_PATH")
        .unwrap_or_else(|_| "/opt/cni/bin:/usr/libexec/cni:/usr/local/lib/cni".into())
}

/// Build the minimal CNI config blob to feed to the IPAM subprocess.
fn config_for_ipam(config: &CniConfig, ipam_cfg: &Value) -> Value {
    serde_json::json!({
        "cniVersion": config.cni_version,
        "name":       config.name,
        "type":       ipam_cfg["type"],
        "ipam":       ipam_cfg,
    })
}

/// Read /run/flannel/subnet.env and inject subnet/gateway into the IPAM config.
async fn inject_flannel_subnet(config: &mut Value) -> Result<()> {
    let env_path = "/run/flannel/subnet.env";
    let content  = tokio::fs::read_to_string(env_path)
        .await
        .with_context(|| format!("reading {env_path}"))?;

    let mut subnet  = String::new();
    let mut gateway = String::new();

    for line in content.lines() {
        if let Some(v) = line.strip_prefix("FLANNEL_SUBNET=") {
            subnet = v.trim().to_string();
        }
        if let Some(v) = line.strip_prefix("FLANNEL_GATEWAY=") {
            gateway = v.trim().to_string();
        }
    }

    if subnet.is_empty() {
        bail!("FLANNEL_SUBNET not found in {env_path}");
    }

    config["ipam"]["subnet"]  = Value::String(subnet);
    config["ipam"]["gateway"] = Value::String(gateway);
    Ok(())
}
