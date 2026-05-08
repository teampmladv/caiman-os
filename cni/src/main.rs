//! caiman-cni v2 -- VM networking with auto NAT/bridge detection
//!
//! Modes (CAIMAN_NET_MODE env):
//!   nat    -- masquerade via host uplink (default, works everywhere)
//!   bridge -- VM gets real LAN IP
//!   none   -- isolated

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

mod bridge;
mod bpf_maps;
mod ipam;
mod standalone;
mod tap;
mod xdp;

#[allow(non_snake_case)]
#[derive(Deserialize, Default)]
struct CniInput {
    cniVersion: String,
    name:       String,
    #[serde(default)]
    netMode:    String,
}

#[derive(Serialize)]
struct CniResult {
    cniVersion:  String,
    interfaces:  Vec<serde_json::Value>,
    ips:         Vec<serde_json::Value>,
    dns:         serde_json::Value,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cmd      = std::env::var("CNI_COMMAND").unwrap_or_default();
    let vm_id    = std::env::var("CNI_CONTAINERID").unwrap_or_default();
    let ifname   = std::env::var("CNI_IFNAME").unwrap_or_else(|_| "eth0".into());
    let netns    = std::env::var("CNI_NETNS").unwrap_or_default();

    info!("caiman-cni v2 cmd={cmd} vm={vm_id}");

    match cmd.as_str() {
        "ADD" => {
            let ip_config = standalone::add(&vm_id, &ifname, &netns).await
                .unwrap_or_else(|e| {
                    tracing::error!("ADD failed: {e}");
                    String::new()
                });

            let alloc = ipam::list();
            let ip = alloc.get(&vm_id).cloned().unwrap_or_else(|| "10.100.0.2".into());

            let result = CniResult {
                cniVersion: "1.0.0".into(),
                interfaces: vec![serde_json::json!({
                    "name": standalone::tap_name(&vm_id),
                    "mac":  "",
                    "sandbox": ""
                })],
                ips: vec![serde_json::json!({
                    "address": format!("{ip}/24"),
                    "gateway": "10.100.0.1",
                    "interface": 0
                })],
                dns: serde_json::json!({
                    "nameservers": ["1.1.1.1", "8.8.8.8"]
                }),
            };

            println!("{}", serde_json::to_string(&result)?);

            // Print ip_config to stderr for caiman-vmm to pick up
            if !ip_config.is_empty() {
                eprintln!("CAIMAN_IP_CONFIG={ip_config}");
            }
        }

        "DEL" => {
            standalone::del(&vm_id, &ifname).await?;
        }

        "CHECK" => {
            // Verify TAP exists
            let tap = standalone::tap_name(&vm_id);
            if !std::path::Path::new(&format!("/sys/class/net/{tap}")).exists() {
                anyhow::bail!("TAP interface {tap} not found");
            }
        }

        "VERSION" => {
            println!(r#"{{"cniVersion":"1.0.0","supportedVersions":["0.3.1","0.4.0","1.0.0"]}}"#);
        }

        "STATUS" => {
            // caiman extension: show network status
            let uplink = bridge::detect_uplink();
            let allocs = ipam::list();
            let mode   = standalone::NetMode::from_env();
            println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                "bridge":    bridge::bridge_name(),
                "uplink":    uplink,
                "mode":      format!("{mode:?}"),
                "allocations": allocs,
            }))?);
        }

        _ => eprintln!("Unknown CNI_COMMAND: {cmd}"),
    }

    Ok(())
}
