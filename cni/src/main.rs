//! caiman-cni — CNI plugin for Caimán OS
//! Called by the container runtime with CNI_COMMAND=ADD|DEL|CHECK
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Deserialize)]
struct CniInput {
    cniVersion: String,
    name: String,
}

#[derive(Serialize)]
struct CniResult {
    cniVersion: String,
    interfaces: Vec<serde_json::Value>,
    ips: Vec<serde_json::Value>,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cmd = std::env::var("CNI_COMMAND").unwrap_or_default();
    let container = std::env::var("CNI_CONTAINERID").unwrap_or_default();
    info!("caiman-cni CNI_COMMAND={cmd} container={container}");

    match cmd.as_str() {
        "ADD" => {
            let result = CniResult {
                cniVersion: "1.0.0".into(),
                interfaces: vec![],
                ips: vec![],
            };
            println!("{}", serde_json::to_string(&result)?);
        }
        "DEL" | "CHECK" => {}
        "VERSION" => {
            println!(r#"{{"cniVersion":"1.0.0","supportedVersions":["0.3.1","0.4.0","1.0.0"]}}"#);
        }
        _ => eprintln!("Unknown CNI_COMMAND: {cmd}"),
    }
    Ok(())
}
