//! caiman-cni — Universal CNI plugin
//!
//! Detects the active CNI ecosystem at runtime and activates the
//! appropriate adapter. Supports all four compatibility modes:
//!
//!  1. Primary CNI      — caiman owns the interface, delegates IPAM
//!  2. CNI chaining     — runs after/before another CNI in a chain list
//!  3. Multus secondary — provides extra interfaces (NetworkAttachmentDef)
//!  4. SR-IOV passthrough — bypasses virtual networking entirely
//!
//! Detection order (highest precedence first):
//!   SRIOV_RESOURCE env → sr-iov adapter
//!   prevResult present → chaining mode
//!   MULTUS_SR_IOV_* env → multus secondary
//!   cilium-agent socket exists → cilium coexistence adapter
//!   calico-node socket exists  → calico adapter
//!   flannel subnet file exists → flannel adapter
//!   antrea-agent socket exists → antrea adapter
//!   weave-net socket exists    → weave adapter
//!   default                    → standalone (host-local IPAM)

use std::io::{self, Read};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

mod bpf_maps;
mod compat;
mod ipam;
mod tap;
mod xdp;

use compat::{
    chain, ipam as ipam_compat, multus,
    adapters::{self, CniAdapter},
    CniEnv, EcosystemKind,
};

// ── CNI wire types ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CniConfig {
    pub cni_version:  String,
    pub name:         String,
    #[serde(rename = "type")]
    pub plugin_type:  String,
    // caiman specific
    #[serde(default = "default_uplink")]
    pub uplink:       String,
    #[serde(default = "default_bpf_pin")]
    pub bpf_pin_path: String,
    // IPAM delegation
    pub ipam:         Option<serde_json::Value>,
    // CNI chaining
    #[serde(rename = "prevResult")]
    pub prev_result:  Option<serde_json::Value>,
    // Multus
    #[serde(rename = "runtimeConfig")]
    pub runtime_cfg:  Option<RuntimeConfig>,
    // SR-IOV
    #[serde(rename = "deviceID")]
    pub device_id:    Option<String>,
    // Adapter overrides (optional — auto-detected if absent)
    pub adapter:      Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeConfig {
    pub mac:          Option<String>,
    pub bandwidth:    Option<BandwidthEntry>,
    pub port_mappings: Option<Vec<PortMapping>>,
    #[serde(rename = "deviceID")]
    pub device_id:    Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BandwidthEntry {
    #[serde(rename = "ingressRate")]  pub ingress_rate:  u64,
    #[serde(rename = "egressRate")]   pub egress_rate:   u64,
    #[serde(rename = "ingressBurst")] pub ingress_burst: u64,
    #[serde(rename = "egressBurst")]  pub egress_burst:  u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PortMapping {
    pub host_port:      u16,
    pub container_port: u16,
    pub protocol:       String,
}

fn default_uplink()  -> String { "eth0".into() }
fn default_bpf_pin() -> String { "/sys/fs/bpf/caiman".into() }

// ── Entry point ────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // CNI plugins must not pollute stdout — log to stderr
    tracing_subscriber::fmt()
        .with_env_filter("caiman_cni=info")
        .with_writer(std::io::stderr)
        .init();

    match run().await {
        Ok(output) => print!("{output}"),
        Err(e) => {
            let err = serde_json::json!({
                "cniVersion": "1.0.0",
                "code": 100,
                "msg": e.to_string(),
                "details": format!("{e:#}")
            });
            print!("{}", serde_json::to_string(&err).unwrap());
            std::process::exit(1);
        }
    }
}

async fn run() -> Result<String> {
    let command      = std::env::var("CNI_COMMAND").context("CNI_COMMAND not set")?;
    let container_id = std::env::var("CNI_CONTAINERID").context("CNI_CONTAINERID not set")?;
    let netns        = std::env::var("CNI_NETNS").unwrap_or_default();
    let if_name      = std::env::var("CNI_IFNAME").unwrap_or_else(|_| "eth0".into());

    let mut stdin = String::new();
    io::stdin().read_to_string(&mut stdin)?;
    let config: CniConfig = serde_json::from_str(&stdin)
        .context("parsing CNI config JSON")?;

    let env = CniEnv {
        command: command.clone(),
        container_id: container_id.clone(),
        netns:    netns.clone(),
        if_name:  if_name.clone(),
        config:   config.clone(),
    };

    // ── Detect ecosystem and pick adapter ──────────────────────────────────
    let kind = if let Some(ref name) = config.adapter {
        EcosystemKind::from_str(name)?
    } else {
        EcosystemKind::detect(&config).await
    };

    info!("CNI {command} container={container_id} ecosystem={kind:?}");

    // ── Dispatch to correct mode ───────────────────────────────────────────
    match command.as_str() {
        "ADD" => {
            // SR-IOV: bypass virtual networking, use hardware VF
            if kind == EcosystemKind::SrIov {
                return adapters::sriov::add(&env).await;
            }

            // Multus secondary interface
            if multus::is_multus_secondary(&config) {
                return multus::add_secondary(&env, kind).await;
            }

            // CNI chaining: another plugin ran first
            if config.prev_result.is_some() {
                return chain::add_chained(&env, kind).await;
            }

            // Primary CNI with ecosystem adapter
            cmd_add(&env, kind).await
        }
        "DEL" => {
            if kind == EcosystemKind::SrIov {
                return adapters::sriov::del(&env).await;
            }
            if multus::is_multus_secondary(&config) {
                return multus::del_secondary(&env).await;
            }
            cmd_del(&env, kind).await
        }
        "CHECK" => cmd_check(&env, kind).await,
        "VERSION" => Ok(version_output()),
        other => bail!("unknown CNI_COMMAND: {other}"),
    }
}

// ── ADD: primary interface setup ───────────────────────────────────────────

async fn cmd_add(env: &CniEnv, kind: EcosystemKind) -> Result<String> {
    info!("ADD mode: ecosystem={kind:?}");

    // 1. Create tap device (common to all adapters)
    let vm_id    = compat::stable_vm_id(&env.container_id);
    let tap_name = format!("tap{vm_id}");
    let (tap_ifindex, tap_mac) = tap::create_tap(&tap_name, &env.netns)
        .await
        .context("creating tap device")?;

    // 2. IPAM: delegate to external plugin (calico-ipam, host-local, cilium, etc.)
    let ip_result = ipam_compat::allocate(&env.config, &env.container_id, &env.netns)
        .await
        .context("IPAM allocation")?;

    // 3. Ecosystem-specific adapter (BGP route, XDP coexistence, VXLAN, OVS, etc.)
    let adapter = adapters::get(kind);
    adapter.setup_network(env, tap_ifindex, &tap_mac, &ip_result)
        .await
        .with_context(|| format!("adapter setup ({kind:?})"))?;

    // 4. XDP: register VM in BPF maps + attach program
    xdp::register_vm(vm_id, &tap_mac, tap_ifindex, &env.config.bpf_pin_path)
        .context("registering VM in XDP maps")?;
    xdp::attach_if_needed(vm_id, &env.config)
        .await
        .context("XDP attach")?;

    // 5. Build CNI result
    let result = build_cni_result(&env.config.cni_version, &tap_name, &tap_mac, &ip_result);
    Ok(serde_json::to_string(&result)?)
}

async fn cmd_del(env: &CniEnv, kind: EcosystemKind) -> Result<String> {
    let vm_id    = compat::stable_vm_id(&env.container_id);
    let tap_name = format!("tap{vm_id}");

    // Best-effort cleanup in reverse order
    if let Err(e) = xdp::detach_and_unregister(vm_id, &env.config.bpf_pin_path).await {
        warn!("XDP cleanup: {e}");
    }

    let adapter = adapters::get(kind);
    if let Err(e) = adapter.teardown_network(env).await {
        warn!("adapter teardown: {e}");
    }

    if let Err(e) = ipam_compat::release(&env.config, &env.container_id, &env.netns).await {
        warn!("IPAM release: {e}");
    }

    if let Err(e) = tap::delete_tap(&tap_name).await {
        warn!("delete_tap: {e}");
    }

    Ok("{}".into())
}

async fn cmd_check(env: &CniEnv, kind: EcosystemKind) -> Result<String> {
    let vm_id    = compat::stable_vm_id(&env.container_id);
    let tap_name = format!("tap{vm_id}");

    // Verify tap device still exists
    if !tap::tap_exists(&tap_name).await {
        bail!("tap device {tap_name} not found for container {}", env.container_id);
    }

    // Verify XDP program still attached
    xdp::check_attached(vm_id, &env.config.bpf_pin_path)?;

    // Adapter-specific health check
    let adapter = adapters::get(kind);
    adapter.check_network(env).await?;

    Ok("{}".into())
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn version_output() -> String {
    r#"{"cniVersion":"1.0.0","supportedVersions":["0.3.0","0.3.1","0.4.0","1.0.0"]}"#.into()
}

fn build_cni_result(
    cni_version: &str,
    tap_name: &str,
    mac: &[u8; 6],
    ip_result: &ipam_compat::IpamResult,
) -> serde_json::Value {
    serde_json::json!({
        "cniVersion": cni_version,
        "interfaces": [{
            "name": tap_name,
            "mac":  format_mac(mac),
        }],
        "ips": ip_result.ips,
        "routes": ip_result.routes,
        "dns": ip_result.dns,
    })
}

pub fn format_mac(mac: &[u8; 6]) -> String {
    mac.map(|b| format!("{b:02x}")).join(":")
}
