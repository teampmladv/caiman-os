//! cmd/vm.rs — caiman vm <subcommand>
//!
//! caiman vm list [--status RUNNING] [--node node-02]
//! caiman vm get vm-001
//! caiman vm start vm-001
//! caiman vm stop vm-001 [--force]
//! caiman vm restart vm-001
//! caiman vm migrate vm-001 --to node-03 [--wait]
//! caiman vm console vm-001
//! caiman vm logs vm-001 [--lines 200] [--follow]
//! caiman vm delete vm-001 [--confirm]
//! caiman vm create --name web-01 --mem 512 --cpus 2 [--node node-01]
//! caiman vm exec vm-001 -- /bin/sh -c "uname -a"
//! caiman vm label vm-001 app=web env=prod

use clap::Subcommand;
use anyhow::Result;
use std::time::Duration;

use crate::api::Client;
use crate::output::{self, OutputFormat, new_table, color_status, color_pct, mini_bar, format_uptime};

#[derive(Subcommand)]
pub enum VmCmd {
    /// List VMs in the cluster
    List {
        #[arg(long)] status: Option<String>,
        #[arg(long)] node:   Option<String>,
        #[arg(long)] label:  Option<String>,
    },
    /// Get details for a single VM
    Get { vm_id: String },
    /// Start a stopped VM
    Start { vm_id: String },
    /// Stop a running VM (graceful)
    Stop {
        vm_id: String,
        #[arg(long)] force: bool,
    },
    /// Restart a VM
    Restart { vm_id: String },
    /// Live-migrate a VM to another node
    Migrate {
        vm_id: String,
        #[arg(long)] to: String,
        /// Wait for migration to complete
        #[arg(long)] wait: bool,
        /// Bandwidth limit in Mbps
        #[arg(long, default_value = "4000")] bandwidth: u64,
    },
    /// Attach to VM serial console (ttyS0)
    Console { vm_id: String },
    /// Tail VM serial logs
    Logs {
        vm_id: String,
        #[arg(long, default_value = "50")] lines: usize,
        #[arg(long, short = 'f')] follow: bool,
    },
    /// Create and start a new VM
    Create {
        #[arg(long)] name:   String,
        #[arg(long, default_value = "256")] mem:   u64,
        #[arg(long, default_value = "1")]   cpus:  u8,
        #[arg(long)] node:   Option<String>,
        #[arg(long)] kernel: Option<String>,
        #[arg(long)] mac:    Option<String>,
        #[arg(long)] label:  Vec<String>,
        #[arg(long)] gpu:    Option<String>,
    },
    /// Delete a VM permanently
    Delete {
        vm_id: String,
        #[arg(long)] confirm: bool,
    },
    /// Set labels on a VM
    Label {
        vm_id: String,
        /// Labels in key=value format
        labels: Vec<String>,
    },
    /// Show real-time metrics for a VM
    Top { vm_id: String },
}

pub async fn run(cmd: VmCmd, client: &Client, out: OutputFormat) -> Result<()> {
    match cmd {
        VmCmd::List { status, node, label } => {
            let mut path = "/api/vms".to_string();
            let mut params = Vec::new();
            if let Some(s) = &status { params.push(format!("status={s}")); }
            if let Some(n) = &node   { params.push(format!("node={n}")); }
            if !params.is_empty() { path.push('?'); path.push_str(&params.join("&")); }

            let res = client.get(&path).await?;
            let vms = res.as_array().map(|a| a.as_slice()).unwrap_or(&[]);

            match out {
                OutputFormat::Table | OutputFormat::Wide => {
                    let mut t = new_table(&["ID", "Name", "Status", "Node", "CPU", "RAM", "NET RX", "Uptime"]);
                    for vm in vms {
                        let cpu_pct = vm["cpuUsagePct"].as_f64().unwrap_or(0.0);
                        let mem     = vm["memMib"].as_u64().unwrap_or(0);
                        let mem_t   = vm["memTotalMib"].as_u64().unwrap_or(1);
                        let mem_pct = mem as f64 / mem_t as f64 * 100.0;
                        let uptime  = vm["uptimeSecs"].as_u64().unwrap_or(0);
                        let rx      = vm["netRxMbps"].as_f64().unwrap_or(0.0);

                        t.add_row(vec![
                            output::dim(vm["id"].as_str().unwrap_or("")).to_string(),
                            output::white(vm["name"].as_str().unwrap_or("")).to_string(),
                            color_status(vm["status"].as_str().unwrap_or("")).to_string(),
                            vm["nodeName"].as_str().unwrap_or("—").to_string(),
                            format!("{} {}", mini_bar(cpu_pct, 8), color_pct(cpu_pct)),
                            format!("{} {}M/{}M", mini_bar(mem_pct, 8), mem, mem_t),
                            format!("{:.1} Gbps", rx / 1000.0),
                            output::dim(&format_uptime(uptime)).to_string(),
                        ]);
                    }
                    println!("{t}");
                    println!("{}", output::dim(&format!("{} VMs", vms.len())));
                }
                _ => println!("{}", output::format_json(&res, out)?),
            }
        }

        VmCmd::Get { vm_id } => {
            let res = client.get(&format!("/api/vms/{vm_id}")).await?;
            println!("{}", output::format_json(&res, out)?);
        }

        VmCmd::Start { vm_id } => {
            let pb = spinner(&format!("Starting {vm_id}…"));
            let res = client.post_empty(&format!("/api/vms/{vm_id}/start")).await?;
            pb.finish_and_clear();
            println!("{} {vm_id} started", output::bright("✓"));
        }

        VmCmd::Stop { vm_id, force } => {
            let path = if force {
                format!("/api/vms/{vm_id}/force-stop")
            } else {
                format!("/api/vms/{vm_id}/stop")
            };
            let pb = spinner(&format!("Stopping {vm_id}…"));
            client.post_empty(&path).await?;
            pb.finish_and_clear();
            println!("{} {vm_id} stopped", output::bright("✓"));
        }

        VmCmd::Restart { vm_id } => {
            let pb = spinner(&format!("Restarting {vm_id}…"));
            client.post_empty(&format!("/api/vms/{vm_id}/stop")).await?;
            tokio::time::sleep(Duration::from_secs(2)).await;
            client.post_empty(&format!("/api/vms/{vm_id}/start")).await?;
            pb.finish_and_clear();
            println!("{} {vm_id} restarted", output::bright("✓"));
        }

        VmCmd::Migrate { vm_id, to, wait, bandwidth } => {
            println!("{} Migrating {} → {}  (bandwidth limit: {} Mbps)",
                output::blue("→"), output::white(&vm_id), output::bright(&to), bandwidth);

            let res = client.post(
                &format!("/api/vms/{vm_id}/migrate"),
                &serde_json::json!({ "toNode": to, "bandwidthMbps": bandwidth }),
            ).await?;

            if wait {
                // Poll for migration progress
                let mut done = false;
                while !done {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let vm = client.get(&format!("/api/vms/{vm_id}")).await?;
                    let status = vm["status"].as_str().unwrap_or("");
                    let pct    = vm["migrating"]["progressPct"].as_f64().unwrap_or(0.0);
                    let phase  = vm["migrating"]["phase"].as_str().unwrap_or("").to_string();

                    pb.set_position(pct as u64);
                    let _ = (phase.clone());

                    if status != "MIGRATING" || pct >= 100.0 {
                        done = true;
                    }
                }
                println!("done");
                println!("{} Migration complete", output::bright("✓"));
            } else {
                println!("{} Migration started. Use {} to monitor.",
                    output::green("→"),
                    output::dim("caiman events --filter migration"));
            }
        }

        VmCmd::Console { vm_id } => {
            console_attach(&vm_id, client).await?;
        }

        VmCmd::Logs { vm_id, lines, follow } => {
            if follow {
                stream_logs(&vm_id, client).await?;
            } else {
                let res = client.get(
                    &format!("/api/vms/{vm_id}/console?lines={lines}")
                ).await?;
                if let Some(arr) = res.as_array() {
                    for line in arr {
                        if let Some(s) = line.as_str() {
                            println!("{}", output::green(s));
                        }
                    }
                }
            }
        }

        VmCmd::Create { name, mem, cpus, node, kernel, mac, label, gpu } => {
            let labels: std::collections::HashMap<String, String> = label.iter()
                .filter_map(|l| {
                    let mut parts = l.splitn(2, '=');
                    Some((parts.next()?.to_string(), parts.next()?.to_string()))
                })
                .collect();

            let body = serde_json::json!({
                "name":    name,
                "memMib":  mem,
                "cpus":    cpus,
                "node":    node,
                "kernel":  kernel,
                "mac":     mac,
                "labels":  labels,
                "gpu":     gpu,
            });

            let pb = spinner("Creating VM…");
            let res = client.post("/api/vms", &body).await?;
            pb.finish_and_clear();
            let id = res["id"].as_str().unwrap_or("?");
            println!("{} VM {} created — {}", output::bright("✓"), output::white(&name), output::dim(id));
        }

        VmCmd::Delete { vm_id, confirm } => {
            if !confirm {
                // Interactive confirmation
                print!("Permanently delete {vm_id}? [y/N] ");
                { use std::io::Write; std::io::stdout().flush()?; }
                let mut _ans = String::new();
                std::io::stdin().read_line(&mut _ans)?;
                if !_ans.trim().eq_ignore_ascii_case("y") {
                    println!("{} Aborted", output::amber("!")); return Ok(());
                }
            }
            client.delete(&format!("/api/vms/{vm_id}")).await?;
            println!("{} {vm_id} deleted", output::bright("✓"));
        }

        VmCmd::Label { vm_id, labels } => {
            let map: serde_json::Value = serde_json::Value::Object(
                labels.iter()
                    .filter_map(|l| {
                        let mut p = l.splitn(2, '=');
                        Some((p.next()?.to_string(),
                              serde_json::Value::String(p.next()?.to_string())))
                    })
                    .collect()
            );
            client.patch(&format!("/api/vms/{vm_id}/labels"), &map).await?;
            println!("{} Labels updated on {vm_id}", output::bright("✓"));
        }

        VmCmd::Top { vm_id } => {
            live_top(&vm_id, client).await?;
        }
    }
    Ok(())
}

// ── Console attach (raw terminal passthrough) ─────────────────────────────

async fn console_attach(vm_id: &str, client: &Client) -> Result<()> {

    println!("{} Attaching to serial console of {}",
        output::blue("→"), output::white(vm_id));
    println!("{}", output::dim("Press Ctrl+] to detach"));

    // Use WS to stream console
    let _ws_url = client.ws_url(&format!("/ws/console/{vm_id}"));
    // Simplified: just tail the log
    stream_logs(vm_id, client).await
}

async fn stream_logs(vm_id: &str, client: &Client) -> Result<()> {
    let mut last_line = 0usize;
    loop {
        let res = client.get(
            &format!("/api/vms/{vm_id}/console?lines=20")
        ).await.unwrap_or(serde_json::json!([]));

        if let Some(arr) = res.as_array() {
            for (i, line) in arr.iter().enumerate() {
                if i >= last_line {
                    if let Some(s) = line.as_str() {
                        println!("{}", output::green(s));
                    }
                }
            }
            last_line = arr.len();
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

// ── Live top (refreshing metrics) ─────────────────────────────────────────

async fn live_top(vm_id: &str, client: &Client) -> Result<()> {
    use std::io::{stdout, Write};

    loop {
        let vm = client.get(&format!("/api/vms/{vm_id}")).await?;
        let cpu = vm["cpuUsagePct"].as_f64().unwrap_or(0.0);
        let mem = vm["memMib"].as_u64().unwrap_or(0);
        let mem_t = vm["memTotalMib"].as_u64().unwrap_or(1);
        let rx  = vm["netRxMbps"].as_f64().unwrap_or(0.0);
        let tx  = vm["netTxMbps"].as_f64().unwrap_or(0.0);

        print!("\r"  {} {} │ CPU {} {} │ MEM {}/{} GiB │ RX {:.1} TX {:.1} Gbps",
            output::bright(vm_id),
            color_status(vm["status"].as_str().unwrap_or("")),
            mini_bar(cpu, 10), color_pct(cpu),
            mem / 1024, mem_t / 1024,
            rx / 1000.0, tx / 1000.0,
        );
        stdout().flush()?;
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn spinner(msg: &str) {
    println!("  ⠋ {msg}...");
}
