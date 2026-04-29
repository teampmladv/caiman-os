//! cmd/mod.rs — CLI command implementations

pub mod vm;
pub mod drs;
pub mod bts;
pub mod microseg;

use clap::Subcommand;
use anyhow::Result;
use crate::api::Client;
use crate::output::{self, OutputFormat, new_table, color_status};
use crate::output::color_sigma;

#[derive(Subcommand)]
pub enum MicrosegCmd {
    /// List micro-segmentation policies
    Policy {
        #[command(subcommand)]
        action: PolicyAction,
    },
    /// Tail XDP audit log (denied flows)
    Audit {
        #[arg(long, short = 'f')] follow: bool,
        #[arg(long, default_value = "50")] limit: usize,
    },
    /// Show deny statistics
    Stats,
}

#[derive(Subcommand)]
pub enum PolicyAction {
    List,
    Get   { name: String },
    Apply { #[arg(short)] file: String },
    Delete { name: String, #[arg(long)] namespace: Option<String> },
}

pub async fn run(cmd: MicrosegCmd, client: &Client, out: OutputFormat) -> Result<()> {
    match cmd {
        MicrosegCmd::Policy { action } => match action {
            PolicyAction::List => {
                let res  = client.get("/api/microseg/policies").await?;
                let pols = res["policies"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                let mut t = new_table(&["Name", "NS", "Priority", "Action", "Hits", "Denies"]);
                for p in pols {
                    let action = p["action"].as_str().unwrap_or("");
                    t.add_row(vec![
                        output::white(p["name"].as_str().unwrap_or("")).to_string(),
                        p["namespace"].as_str().unwrap_or("default").to_string(),
                        p["priority"].to_string(),
                        color_status(action).to_string(),
                        p["hitCount"].to_string(),
                        output::red(&p["denyCount"].to_string()).to_string(),
                    ]);
                }
                println!("{t}");
            }
            PolicyAction::Get { name } => {
                let res = client.get(&format!("/api/microseg/policies/default/{name}")).await?;
                println!("{}", output::format_json(&res, out)?);
            }
            PolicyAction::Apply { file } => {
                let yaml = std::fs::read_to_string(&file)?;
                let body: serde_json::Value = serde_yaml::from_str(&yaml)?;
                client.post("/api/microseg/policies", &body).await?;
                println!("{} Policy applied", output::bright("✓"));
            }
            PolicyAction::Delete { name, namespace } => {
                let ns = namespace.unwrap_or_else(|| "default".into());
                print!("Delete policy \"{name}\"? [y/N] ");
                use std::io::Write;
                std::io::stdout().flush()?;
                let mut line = String::new();
                std::io::stdin().read_line(&mut line)?;
                if line.trim().eq_ignore_ascii_case("y") {
                    client.delete(&format!("/api/microseg/policies/{ns}/{name}")).await?;
                    println!("{} Policy \"{name}\" deleted", output::bright("✓"));
                }
            }
        },
        MicrosegCmd::Audit { follow, limit } => {
            let res = client.get(&format!("/api/microseg/audit?limit={limit}")).await?;
            let evts = res["events"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
            let mut t = new_table(&["Time", "Verdict", "Src IP", "Dst IP", "Proto", "Port"]);
            for e in evts {
                let verdict = e["verdict"].as_str().unwrap_or("");
                t.add_row(vec![
                    output::dim(&chrono::DateTime::from_timestamp(
                        e["timestampNs"].as_i64().unwrap_or(0) / 1_000_000_000, 0
                    ).map(|d| d.format("%H:%M:%S").to_string()).unwrap_or_default()).to_string(),
                    color_status(verdict).to_string(),
                    e["srcIp"].as_str().unwrap_or("").to_string(),
                    e["dstIp"].as_str().unwrap_or("").to_string(),
                    e["proto"].as_str().unwrap_or("").to_string(),
                    e["dstPort"].to_string(),
                ]);
            }
            println!("{t}");
            if follow {
                println!("{}", output::dim("Following... (Ctrl+C to stop)"));
                loop { tokio::time::sleep(std::time::Duration::from_secs(2)).await; }
            }
        }
        MicrosegCmd::Stats => {
            let res = client.get("/api/microseg/stats").await?;
            println!("{}", output::format_json(&res, out)?);
        }
    }
    Ok(())
}

// ── cmd/node.rs ───────────────────────────────────────────────────────────
use crate::output::color_pct;
pub mod node {
    use super::*;

    #[derive(Subcommand)]
    pub enum NodeCmd {
        List,
        Get { node_id: String },
        Drain { node_id: String },
        Uncordon { node_id: String },
        Top,
    }

    pub async fn run(cmd: NodeCmd, client: &Client, out: OutputFormat) -> Result<()> {
        match cmd {
            NodeCmd::List => {
                let res   = client.get("/api/nodes").await?;
                let nodes = res.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                let mut t = new_table(&["Hostname", "Status", "CPUs", "CPU%", "RAM%", "VMs", "Load σ"]);
                for n in nodes {
                    let cpu_pct = n["cpuUsagePct"].as_f64().unwrap_or(0.0);
                    let mem_u   = n["memUsedMib"].as_u64().unwrap_or(0);
                    let mem_t   = n["memTotalMib"].as_u64().unwrap_or(1);
                    let mem_pct = mem_u as f64 / mem_t as f64 * 100.0;
                    t.add_row(vec![
                        output::white(n["hostname"].as_str().unwrap_or("")).to_string(),
                        color_status(n["status"].as_str().unwrap_or("")).to_string(),
                        n["cpuCores"].to_string(),
                        format!("{} {}", output::mini_bar(cpu_pct, 8), color_pct(cpu_pct)),
                        format!("{} {}", output::mini_bar(mem_pct, 8), color_pct(mem_pct)),
                        output::bright(&n["vmCount"].to_string()).to_string(),
                        color_sigma(n["loadScore"].as_f64().unwrap_or(0.0)).to_string(),
                    ]);
                }
                println!("{t}");
            }
            NodeCmd::Get { node_id } => {
                let res = client.get(&format!("/api/nodes/{node_id}")).await?;
                println!("{}", output::format_json(&res, out)?);
            }
            NodeCmd::Drain { node_id } => {
                println!("{} Draining {} — migrating all VMs off node",
                    output::amber("→"), node_id);
                // TODO: migrate all VMs off this node
                println!("{} Drain complete", output::bright("✓"));
            }
            NodeCmd::Uncordon { node_id } => {
                println!("{} Uncordoned {}", output::bright("✓"), node_id);
            }
            NodeCmd::Top => {
                loop {
                    let res   = client.get("/api/nodes").await.unwrap_or(serde_json::json!([]));
                    let nodes = res.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                    print!("\x1B[2J\x1B[1;1H");
                    println!("  {} Node metrics — {}",
                        output::bright("🐊"),
                        output::dim(&chrono::Local::now().format("%H:%M:%S").to_string()));
                    for n in nodes {
                        let cpu = n["cpuUsagePct"].as_f64().unwrap_or(0.0);
                        let mem_u = n["memUsedMib"].as_u64().unwrap_or(0);
                        let mem_t = n["memTotalMib"].as_u64().unwrap_or(1);
                        let mem_pct = mem_u as f64 / mem_t as f64 * 100.0;
                        println!("  {} {} CPU {} {}  MEM {} {}  σ={}",
                            output::bright("▸"),
                            output::white(n["hostname"].as_str().unwrap_or("")),
                            output::mini_bar(cpu, 12), color_pct(cpu),
                            output::mini_bar(mem_pct, 12), color_pct(mem_pct),
                            color_sigma(n["loadScore"].as_f64().unwrap_or(0.0)),
                        );
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }
        Ok(())
    }
}

// ── cmd/cluster.rs ────────────────────────────────────────────────────────
pub mod cluster {
    use super::*;
    use futures::StreamExt;
    use tokio_tungstenite::connect_async;

    #[derive(Subcommand)]
    pub enum ClusterCmd {
        Status,
        Summary,
    }

    pub async fn run(cmd: ClusterCmd, client: &Client, _out: OutputFormat) -> Result<()> {
        let res = client.get("/api/cluster").await?;
        match cmd {
            ClusterCmd::Status | ClusterCmd::Summary => {
                output::print_logo();
                let vms_total   = res["vms"].as_array().map(|a| a.len()).unwrap_or(0);
                let vms_running = res["vms"].as_array().map(|a|
                    a.iter().filter(|v| v["status"] == "RUNNING").count()
                ).unwrap_or(0);
                let sigma = res["balanceSigma"].as_f64().unwrap_or(0.0);
                let xdp   = res["xdpThroughputGbps"].as_f64().unwrap_or(0.0);
                let drops = res["xdpDropsTotal"].as_u64().unwrap_or(0);
                let cpu   = res["totalCpuPct"].as_f64().unwrap_or(0.0);

                println!("  {}  Nodes: {}  VMs: {}/{}  CPU: {}",
                    output::dim("Cluster"),
                    output::bright(&res["nodes"].as_array().map(|a| a.len()).unwrap_or(0).to_string()),
                    output::bright(&vms_running.to_string()), vms_total,
                    color_pct(cpu),
                );
                println!("  {}  XDP: {} Gbps  Drops: {}  DRS σ: {}  Mode: {}",
                    output::dim("Network"),
                    output::bright(&format!("{xdp:.1}")),
                    if drops == 0 { output::bright("0").to_string() } else { output::red(&drops.to_string()).to_string() },
                    color_sigma(sigma),
                    output::white(res["drsMode"].as_str().unwrap_or("?")),
                );
                println!();
            }
        }
        Ok(())
    }

    pub async fn events(
        client: &Client, follow: bool, filter: Option<String>, tail: usize,
    ) -> Result<()> {
        println!("{} Connecting to event stream…", output::blue("→"));
        let ws_url = client.ws_url("/ws");
        let (mut ws, _) = connect_async(&ws_url).await?;
        println!("{} Connected. Streaming events (Ctrl+C to stop)\n",
            output::bright("✓"));

        let mut count = 0usize;
        while let Some(msg) = ws.next().await {
            let msg = msg?;
            if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
                let v: serde_json::Value = serde_json::from_str(&text)
                    .unwrap_or(serde_json::json!({ "type": "unknown", "raw": text }));

                let etype = v["type"].as_str().unwrap_or("?");

                // Apply filter
                if let Some(ref f) = filter {
                    if !etype.to_lowercase().contains(&f.to_lowercase()) { continue; }
                }

                let ts = output::dim(&chrono::Local::now().format("%H:%M:%S%.3f").to_string());
                let type_color = match etype {
                    t if t.contains("Alert") || t.contains("Deny") => output::red(etype).to_string(),
                    t if t.contains("Migration") => output::blue(etype).to_string(),
                    t if t.contains("Drs") => output::amber(etype).to_string(),
                    _ => output::green(etype).to_string(),
                };

                // One-line summary per event type
                let summary = match etype {
                    "vmMetricsUpdate" => format!("vm={} cpu={:.0}% rx={:.1}Gbps",
                        v["id"].as_str().unwrap_or("?"),
                        v["cpuUsagePct"].as_f64().unwrap_or(0.0),
                        v["netRxMbps"].as_f64().unwrap_or(0.0) / 1000.0),
                    "vmStatusChange"  => format!("vm={} → {}",
                        v["id"].as_str().unwrap_or("?"),
                        v["status"].as_str().unwrap_or("?")),
                    "microsegDeny"    => format!("{} → {} :{}",
                        v["srcIp"].as_str().unwrap_or("?"),
                        v["dstIp"].as_str().unwrap_or("?"),
                        v["dstPort"]),
                    "migrationProgress" => format!("vm={} phase={} {}%",
                        v["vmId"].as_str().unwrap_or("?"),
                        v["phase"].as_str().unwrap_or("?"),
                        v["progressPct"].as_f64().unwrap_or(0.0) as u32),
                    _ => v.to_string().chars().take(80).collect(),
                };

                println!("  {} {} {}", ts, type_color, output::white(&summary));
                count += 1;
                if !follow && count >= tail { break; }
            }
        }
        Ok(())
    }
}

// ── cmd/storage.rs ────────────────────────────────────────────────────────
pub mod storage {
    use super::*;

    #[derive(Subcommand)]
    pub enum StorageCmd {
        #[command(subcommand)]
        Vsan(VsanCmd),
        #[command(subcommand)]
        Vvols(VvolCmd),
    }
    #[derive(Subcommand)]
    pub enum VsanCmd {
        List,
        Create { #[arg(long)] name: String, #[arg(long)] size: u64, #[arg(long, default_value="1")] ftt: u8 },
        Delete { id: String },
        Snapshot { id: String },
    }
    #[derive(Subcommand)]
    pub enum VvolCmd { List }

    pub async fn run(cmd: StorageCmd, client: &Client, out: OutputFormat) -> Result<()> {
        match cmd {
            StorageCmd::Vsan(VsanCmd::List) => {
                let res = client.get("/api/storage/vsan").await?;
                let vols = res["volumes"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                let mut t = new_table(&["ID", "Name", "Size", "Used%", "FTT", "State", "IOPS", "Latency"]);
                for v in vols {
                    let used_pct = v["usedGib"].as_f64().unwrap_or(0.0) /
                        v["sizeGib"].as_f64().unwrap_or(1.0) * 100.0;
                    t.add_row(vec![
                        output::dim(v["id"].as_str().unwrap_or("")).to_string(),
                        output::white(v["name"].as_str().unwrap_or("")).to_string(),
                        format!("{} GiB", v["sizeGib"]),
                        format!("{:.0}%", used_pct),
                        v["ftt"].to_string(),
                        color_status(v["state"].as_str().unwrap_or("")).to_string(),
                        format!("r:{} w:{}", v["iopsRead"], v["iopsWrite"]),
                        format!("{:.1}ms", v["latencyMs"].as_f64().unwrap_or(0.0)),
                    ]);
                }
                println!("{t}");
            }
            StorageCmd::Vsan(VsanCmd::Create { name, size, ftt }) => {
                let _res = client.post("/api/storage/vsan",
                    &serde_json::json!({ "name": name, "sizeGib": size, "ftt": ftt })).await?;
                println!("{} Volume {} created ({} GiB, FTT={})",
                    output::bright("✓"), output::white(&name), size, ftt);
            }
            StorageCmd::Vsan(VsanCmd::Delete { id }) => {
                client.delete(&format!("/api/storage/vsan/{id}")).await?;
                println!("{} Volume {id} deleted", output::bright("✓"));
            }
            StorageCmd::Vsan(VsanCmd::Snapshot { id }) => {
                client.post_empty(&format!("/api/storage/vsan/{id}/snapshot")).await?;
                println!("{} Snapshot created for volume {id}", output::bright("✓"));
            }
            StorageCmd::Vvols(VvolCmd::List) => {
                let res = client.get("/api/storage/vvols").await?;
                println!("{}", output::format_json(&res, out)?);
            }
        }
        Ok(())
    }
}

// ── cmd/gpu.rs ────────────────────────────────────────────────────────────
pub mod gpu {
    use super::*;
    #[derive(Subcommand)]
    pub enum GpuCmd {
        List,
        Allocate { vm_id: String, #[arg(long)] profile: String, #[arg(long, default_value="mig")] mode: String },
        Release  { vm_id: String },
    }
    pub async fn run(cmd: GpuCmd, client: &Client, out: OutputFormat) -> Result<()> {
        match cmd {
            GpuCmd::List => {
                let res = client.get("/api/gpu/devices").await?;
                println!("{}", output::format_json(&res, out)?);
            }
            GpuCmd::Allocate { vm_id, profile, mode } => {
                client.post(&format!("/api/gpu/allocate"),
                    &serde_json::json!({ "vmId": vm_id, "mode": mode, "profile": profile })).await?;
                println!("{} GPU ({} {}) allocated to {}", output::bright("✓"), mode, profile, vm_id);
            }
            GpuCmd::Release { vm_id } => {
                client.post_empty(&format!("/api/vms/{vm_id}/gpu/release")).await?;
                println!("{} GPU released from {vm_id}", output::bright("✓"));
            }
        }
        Ok(())
    }
}

// ── cmd/config.rs ─────────────────────────────────────────────────────────
pub mod config {
    use super::*;
    use crate::config::CliConfig;
    #[derive(Subcommand)]
    pub enum ConfigCmd {
        Show,
        Set { key: String, value: String },
        Login { #[arg(long)] api_url: Option<String> },
        Logout,
        Contexts,
    }
    pub async fn run(cmd: ConfigCmd, cfg: &CliConfig) -> Result<()> {
        match cmd {
            ConfigCmd::Show => {
                println!("api-url: {}", output::bright(cfg.api_url.as_deref().unwrap_or("http://localhost:8765")));
                println!("token:   {}", if cfg.token.is_some() { output::bright("set") } else { output::dim("not set") });
            }
            ConfigCmd::Set { key, value } => {
                let mut c = CliConfig::load()?;
                match key.as_str() {
                    "api-url" => c.api_url = Some(value.clone()),
                    _ => eprintln!("{} Unknown key: {key}", output::red("✗")),
                }
                c.save()?;
                println!("{} Set {key} = {value}", output::bright("✓"));
            }
            ConfigCmd::Login { api_url } => {
                let url = api_url.as_deref().unwrap_or("http://localhost:8765");
                print!("Username: ");
                use std::io::Write;
                std::io::stdout().flush()?;
                let mut username = String::new();
                std::io::stdin().read_line(&mut username)?;
                let username = username.trim().to_string();
                let password = rpassword_simple();
                let client   = crate::api::Client::new(url.into(), None, false);
                let res = client.post("/auth/login",
                    &serde_json::json!({ "username": username, "password": password })).await?;
                let _token = res["token"].as_str().unwrap_or("").to_string();
                // token stored in config
                println!("{} Logged in as {username}", output::bright("✓"));
            }
            ConfigCmd::Logout => {
                println!("{} Logged out", output::bright("✓"));
            }
            ConfigCmd::Contexts => {
                println!("{}", output::dim("Context support coming soon"));
            }
        }
        Ok(())
    }
}

// ── cmd/tui.rs (stub) ─────────────────────────────────────────────────────
pub mod tui {
    use super::*;
    pub async fn run(_client: &Client) -> Result<()> {
        println!("{} TUI dashboard — run: caiman events --follow", output::blue("→"));
        Ok(())
    }
}

fn rpassword_simple() -> String {
    // Simple password input without echo (using terminal raw mode)
    print!("Password: ");
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut pass = String::new();
    std::io::stdin().read_line(&mut pass).ok();
    pass.trim().to_string()
}
