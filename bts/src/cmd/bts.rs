//! Extends caiman CLI with bts subcommands
//! Add to caiman-cli/src/cmd/mod.rs

// ── caiman snapshot ──────────────────────────────────────────────────────
//
// caiman snapshot list [--vm vm-001]
// caiman snapshot take vm-001 --name "before-upgrade"
// caiman snapshot take vm-001 --name "pre-patch" --consistency quiesced
// caiman snapshot restore snap-abc --to vm-001
// caiman snapshot clone snap-abc --name vm-clone-01
// caiman snapshot chain vm-001
// caiman snapshot seal snap-abc
// caiman snapshot delete snap-abc

pub mod snapshot {
    use anyhow::Result;
    use clap::Subcommand;
    use crate::api::Client;
    use crate::output::{self, OutputFormat, new_table, color_status};

    #[derive(Subcommand)]
    pub enum SnapCmd {
        /// List snapshots
        List {
            #[arg(long)] vm: Option<String>,
        },
        /// Take a new snapshot of a VM
        Take {
            vm_id: String,
            #[arg(long)] name: String,
            #[arg(long)] description: Option<String>,
            /// quiesced | crash-consistent | offline
            #[arg(long, default_value = "crash-consistent")] consistency: String,
            #[arg(long)] label: Vec<String>,
        },
        /// Restore a VM to a snapshot
        Restore {
            snap_id: String,
            #[arg(long)] to: Option<String>,
            #[arg(long)] name: Option<String>,
            #[arg(long)] node: Option<String>,
        },
        /// Clone a snapshot into a new VM (instant COW)
        Clone {
            snap_id: String,
            #[arg(long)] name: String,
            #[arg(long)] node: Option<String>,
            #[arg(long)] start: bool,
        },
        /// Show the COW chain tree for a VM
        Chain { vm_id: String },
        /// Seal a snapshot (make read-only)
        Seal { snap_id: String },
        /// Delete a snapshot (merges delta into children)
        Delete {
            snap_id: String,
            #[arg(long)] confirm: bool,
        },
    }

    pub async fn run(cmd: SnapCmd, client: &Client, out: OutputFormat) -> Result<()> {
        match cmd {
            SnapCmd::List { vm } => {
                let path = vm.as_ref()
                    .map(|v| format!("/api/bts/snapshots?vm_id={v}"))
                    .unwrap_or("/api/bts/snapshots".into());
                let res   = client.get(&path).await?;
                let snaps = res["snapshots"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);

                let mut t = new_table(&["ID", "VM", "Name", "Depth", "Actual", "Consistency", "Sealed", "Created"]);
                for s in snaps {
                    let actual = s["actualMib"].as_u64().unwrap_or(0);
                    let sealed = s["sealed"].as_bool().unwrap_or(false);
                    t.add_row(vec![
                        output::dim(&s["id"].as_str().unwrap_or("")[..8]).to_string(),
                        s["vmName"].as_str().unwrap_or("").to_string(),
                        output::white(s["name"].as_str().unwrap_or("")).to_string(),
                        format!("Δ{}", s["depth"]),
                        format!("{actual} MiB"),
                        output::dim(s["consistency"].as_str().unwrap_or("")).to_string(),
                        if sealed { output::amber("SEALED").to_string() } else { output::dim("no").to_string() },
                        s["createdAt"].as_str().unwrap_or("").chars().take(16).collect::<String>(),
                    ]);
                }
                println!("{t}");
                println!("{}", output::dim(&format!("{} snapshots", snaps.len())));
            }

            SnapCmd::Take { vm_id, name, description, consistency, label } => {
                println!("{} Taking snapshot of {}…", output::blue("→"), output::white(&vm_id));

                let labels: std::collections::HashMap<String, String> = label.iter()
                    .filter_map(|l| { let mut p = l.splitn(2,'='); Some((p.next()?.into(), p.next()?.into())) })
                    .collect();

                let res = client.post(&format!("/api/bts/snapshots/{vm_id}"), &serde_json::json!({
                    "name": name, "description": description,
                    "consistency": consistency, "labels": labels,
                })).await?;

                let id      = res["id"].as_str().unwrap_or("?");
                let actual  = res["actualMib"].as_u64().unwrap_or(0);
                println!("{} Snapshot created: {} ({actual} MiB actual)",
                    output::bright("✓"), output::white(&name));
                println!("  ID:    {}", output::dim(id));
                println!("  Depth: Δ{}", res["depth"]);
                println!("  Mode:  {}", res["consistency"].as_str().unwrap_or(""));
            }

            SnapCmd::Restore { snap_id, to, name, node } => {
                let ok = inquire::Confirm::new(
                    &format!("Restore to snapshot {snap_id}? VM will be stopped briefly.")
                ).with_default(false).prompt()?;
                if !ok { return Ok(()); }

                let res = client.post(&format!("/api/bts/snapshots/{snap_id}/restore"),
                    &serde_json::json!({ "targetVmId": to, "targetName": name, "targetNode": node })
                ).await?;
                println!("{} Restore complete: {}", output::bright("✓"),
                    res["message"].as_str().unwrap_or(""));
            }

            SnapCmd::Clone { snap_id, name, node, start } => {
                println!("{} Cloning snapshot {} → {}…",
                    output::blue("→"), output::dim(&snap_id[..8.min(snap_id.len())]), output::white(&name));
                let res = client.post(&format!("/api/bts/snapshots/{snap_id}/clone"),
                    &serde_json::json!({ "name": name, "node": node, "startAfter": start })
                ).await?;
                println!("{} Clone complete in < 5s: {}",
                    output::bright("✓"),
                    res["resourceId"].as_str().unwrap_or("?"));
            }

            SnapCmd::Chain { vm_id } => {
                let res   = client.get(&format!("/api/bts/snapshots/{vm_id}/chain")).await?;
                let chain = res["chain"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                println!("\n  {} Snapshot chain for {}\n", output::bright("🐊"), output::white(&vm_id));
                for s in chain {
                    let depth = s["depth"].as_u64().unwrap_or(0);
                    let indent = "  ".repeat(depth as usize + 1);
                    let sealed = if s["sealed"].as_bool().unwrap_or(false) { " [SEALED]" } else { "" };
                    println!("{}{}{}  {} {}MiB  {}",
                        indent,
                        if depth == 0 { output::bright("◉").to_string() }
                             else { output::green("└─ ◉").to_string() },
                        sealed,
                        output::white(s["name"].as_str().unwrap_or("")),
                        s["actualMib"],
                        output::dim(s["id"].as_str().unwrap_or("")[..8].as_ref()),
                    );
                }
                println!();
            }

            SnapCmd::Seal { snap_id } => {
                client.post_empty(&format!("/api/bts/snapshots/{snap_id}/seal")).await?;
                println!("{} Snapshot {snap_id} sealed (read-only)", output::bright("✓"));
            }

            SnapCmd::Delete { snap_id, confirm } => {
                if !confirm {
                    let ok = inquire::Confirm::new(
                        &format!("Delete snapshot {snap_id}? Delta will be merged into children.")
                    ).with_default(false).prompt()?;
                    if !ok { return Ok(()); }
                }
                client.delete(&format!("/api/bts/snapshots/{snap_id}")).await?;
                println!("{} Snapshot {snap_id} deleted", output::bright("✓"));
            }
        }
        Ok(())
    }
}

// ── caiman backup ─────────────────────────────────────────────────────────
//
// caiman backup list [--vm vm-001]
// caiman backup start vm-001 --target s3://bucket/prefix
// caiman backup start vm-001 --target nfs://nas01:/exports/caiman --type incremental
// caiman backup restore bup-abc --to vm-001
// caiman backup verify bup-abc
// caiman backup schedule list
// caiman backup schedule create --vm vm-001 --cron "0 2 * * *" --target ...

pub mod backup {
    use anyhow::Result;
    use clap::Subcommand;
    use crate::api::Client;
    use crate::output::{self, OutputFormat, new_table};

    #[derive(Subcommand)]
    pub enum BackupCmd {
        List   { #[arg(long)] vm: Option<String> },
        Start  {
            vm_id: String,
            #[arg(long)] target: String,
            #[arg(long, default_value="full")] r#type: String,
            #[arg(long)] parent: Option<String>,
            #[arg(long)] description: Option<String>,
        },
        Restore { backup_id: String, #[arg(long)] to: Option<String>, #[arg(long)] name: Option<String> },
        Verify  { backup_id: String },
        Delete  { backup_id: String },
        Schedule {
            #[command(subcommand)]
            action: ScheduleAction,
        },
        Stats,
    }

    #[derive(Subcommand)]
    pub enum ScheduleAction {
        List,
        Create {
            #[arg(long)] vm: Option<String>,
            #[arg(long)] name: String,
            #[arg(long)] cron: String,
            #[arg(long)] target: String,
            #[arg(long, default_value="full")] r#type: String,
            #[arg(long, default_value="7")] keep_daily: u32,
            #[arg(long, default_value="4")] keep_weekly: u32,
        },
        Delete { id: String },
    }

    pub async fn run(cmd: BackupCmd, client: &Client, out: OutputFormat) -> Result<()> {
        match cmd {
            BackupCmd::List { vm } => {
                let path = vm.as_ref()
                    .map(|v| format!("/api/bts/backups?vm_id={v}"))
                    .unwrap_or("/api/bts/backups".into());
                let res  = client.get(&path).await?;
                let bups = res["backups"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);

                let mut t = new_table(&["ID", "VM", "Type", "Status", "Size", "Ratio", "Dedup saved", "Duration", "Started"]);
                for b in bups {
                    let size   = b["sizeMib"].as_u64().unwrap_or(0);
                    let ratio  = b["ratio"].as_f64().unwrap_or(1.0);
                    let dedup  = b["dedupMib"].as_u64().unwrap_or(0);
                    let dur    = b["durationSecs"].as_u64().unwrap_or(0);
                    let status = b["status"].as_str().unwrap_or("");
                    t.add_row(vec![
                        output::dim(&b["id"].as_str().unwrap_or("")[..8]).to_string(),
                        b["vmName"].as_str().unwrap_or("").to_string(),
                        b["backupType"].as_str().unwrap_or("").to_string(),
                        color_status_bup(status),
                        format!("{size} MiB"),
                        output::bright(&format!("{ratio:.1}x")).to_string(),
                        format!("{dedup} MiB"),
                        if dur > 0 { format!("{dur}s") } else { "—".into() },
                        b["startedAt"].as_str().unwrap_or("").chars().take(16).collect::<String>(),
                    ]);
                }
                println!("{t}");
            }

            BackupCmd::Start { vm_id, target, r#type, parent, description } => {
                let btype    = r#type;
                let tgt_body = parse_target_string(&target);

                println!("{} Starting {btype} backup of {} → {target}",
                    output::blue("→"), output::white(&vm_id));

                let pb = indicatif::ProgressBar::new_spinner();
                pb.enable_steady_tick(std::time::Duration::from_millis(80));
                pb.set_message("Initializing Restic repo…");

                let res = client.post(&format!("/api/bts/backups/{vm_id}"), &serde_json::json!({
                    "backupType": btype, "target": tgt_body,
                    "parentId": parent, "description": description,
                    "retention": { "keepDaily": 7, "keepWeekly": 4, "keepMonthly": 12 }
                })).await?;

                pb.finish_and_clear();

                let size  = res["sizeMib"].as_u64().unwrap_or(0);
                let ratio = res["ratio"].as_f64().unwrap_or(1.0);
                let dur   = res["durationSecs"].as_u64().unwrap_or(0);
                println!("{} Backup complete: {} MiB compressed  {:.1}x  {}s",
                    output::bright("✓"), size, ratio, dur);
                println!("  ID: {}", output::dim(res["id"].as_str().unwrap_or("?")));
            }

            BackupCmd::Restore { backup_id, to, name } => {
                let ok = inquire::Confirm::new(
                    &format!("Restore from backup {backup_id}?")
                ).with_default(false).prompt()?;
                if !ok { return Ok(()); }

                let res = client.post(&format!("/api/bts/backups/{backup_id}/restore"),
                    &serde_json::json!({ "targetVmId": to, "targetName": name })
                ).await?;
                println!("{} Restore complete: {}",
                    output::bright("✓"), res["message"].as_str().unwrap_or(""));
            }

            BackupCmd::Verify { backup_id } => {
                println!("{} Verifying backup {backup_id}…", output::blue("→"));
                let res = client.post_empty(&format!("/api/bts/backups/{backup_id}/verify")).await?;
                let ok = res["valid"].as_bool().unwrap_or(false);
                if ok {
                    println!("{} Backup integrity verified", output::bright("✓"));
                } else {
                    println!("{} Backup verification FAILED — consider re-running backup", output::red("✗"));
                }
            }

            BackupCmd::Delete { backup_id } => {
                client.delete(&format!("/api/bts/backups/{backup_id}")).await?;
                println!("{} Backup {backup_id} deleted", output::bright("✓"));
            }

            BackupCmd::Schedule { action } => match action {
                ScheduleAction::List => {
                    let res = client.get("/api/bts/schedules").await?;
                    println!("{}", output::format_json(&res, out)?);
                }
                ScheduleAction::Create { vm, name, cron, target, r#type, keep_daily, keep_weekly } => {
                    let res = client.post("/api/bts/schedules", &serde_json::json!({
                        "vmId": vm, "name": name, "cronExpr": cron,
                        "backupType": r#type,
                        "target": parse_target_string(&target),
                        "retention": { "keepDaily": keep_daily, "keepWeekly": keep_weekly }
                    })).await?;
                    println!("{} Schedule created: {name}", output::bright("✓"));
                }
                ScheduleAction::Delete { id } => {
                    client.delete(&format!("/api/bts/schedules/{id}")).await?;
                    println!("{} Schedule deleted", output::bright("✓"));
                }
            },

            BackupCmd::Stats => {
                let res = client.get("/api/bts/stats").await?;
                let snaps = &res["snapshots"];
                let bups  = &res["backups"];
                let tmpls = &res["templates"];
                println!();
                println!("  {}  Snapshots:  {} total  {} MiB on disk",
                    output::bright("◎"), snaps["count"], snaps["totalMib"]);
                println!("  {}  Backups:    {} completed  {} MiB compressed",
                    output::bright("◎"), bups["count"], bups["totalMib"]);
                println!("  {}  Templates:  {} published  {} VMs cloned",
                    output::bright("◎"), tmpls["published"], tmpls["totalClones"]);
                println!();
            }
        }
        Ok(())
    }

    fn color_status_bup(s: &str) -> String {
        match s {
            "Completed"  => output::bright("COMPLETED").to_string(),
            "Running"    => output::blue("RUNNING").to_string(),
            "Failed"     => output::red("FAILED").to_string(),
            "Verifying"  => output::amber("VERIFYING").to_string(),
            _            => output::dim(s).to_string(),
        }
    }

    fn parse_target_string(s: &str) -> serde_json::Value {
        if s.starts_with("s3://") {
            let rest = s.trim_start_matches("s3://");
            let (bucket, prefix) = rest.split_once('/').unwrap_or((rest, "caiman"));
            serde_json::json!({ "type": "s3", "bucket": bucket, "prefix": prefix })
        } else if s.starts_with("nfs://") {
            let rest = s.trim_start_matches("nfs://");
            let (server, export) = rest.split_once(':').unwrap_or((rest, "/backup"));
            serde_json::json!({ "type": "nfs", "server": server, "export": export,
                                "mountPath": format!("/mnt/caiman-backup/{server}") })
        } else {
            serde_json::json!({ "type": "local", "path": s })
        }
    }
}

// ── caiman template ───────────────────────────────────────────────────────
//
// caiman template list
// caiman template get tmpl-abc
// caiman template create --from-snapshot snap-abc --name ubuntu-22.04 --version 1.0.0
// caiman template clone tmpl-abc --name new-vm-01 --start
// caiman template publish tmpl-abc
// caiman template delete tmpl-abc

pub mod template {
    use anyhow::Result;
    use clap::Subcommand;
    use crate::api::Client;
    use crate::output::{self, OutputFormat, new_table};

    #[derive(Subcommand)]
    pub enum TmplCmd {
        List,
        Get  { tmpl_id: String },
        Create {
            #[arg(long)] from_snapshot: String,
            #[arg(long)] name: String,
            #[arg(long, default_value="1.0.0")] version: String,
            #[arg(long)] os_version: String,
            #[arg(long)] description: Option<String>,
            #[arg(long)] cloud_init: Option<String>,
            #[arg(long, default_value="256")] default_mem: u64,
            #[arg(long, default_value="1")]   default_cpus: u8,
        },
        Clone {
            tmpl_id: String,
            #[arg(long)] name: String,
            #[arg(long)] node: Option<String>,
            #[arg(long)] mem: Option<u64>,
            #[arg(long)] cpus: Option<u8>,
            #[arg(long)] start: bool,
        },
        Publish   { tmpl_id: String },
        Unpublish { tmpl_id: String },
        Delete    { tmpl_id: String },
    }

    pub async fn run(cmd: TmplCmd, client: &Client, out: OutputFormat) -> Result<()> {
        match cmd {
            TmplCmd::List => {
                let res   = client.get("/api/bts/templates").await?;
                let tmpls = res["templates"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                let mut t = new_table(&["ID", "Name", "Version", "OS", "Size", "Clones", "Published"]);
                for tmpl in tmpls {
                    let pub_ = tmpl["published"].as_bool().unwrap_or(false);
                    t.add_row(vec![
                        output::dim(&tmpl["id"].as_str().unwrap_or("")[..8]).to_string(),
                        output::white(tmpl["name"].as_str().unwrap_or("")).to_string(),
                        tmpl["version"].as_str().unwrap_or("?").to_string(),
                        tmpl["osVersion"].as_str().unwrap_or("").to_string(),
                        format!("{} MiB", tmpl["imageMib"]),
                        output::bright(&tmpl["cloneCount"].to_string()).to_string(),
                        if pub_ { output::bright("YES").to_string() } else { output::dim("no").to_string() },
                    ]);
                }
                println!("{t}");
            }

            TmplCmd::Get { tmpl_id } => {
                let res = client.get(&format!("/api/bts/templates/{tmpl_id}")).await?;
                println!("{}", output::format_json(&res, out)?);
            }

            TmplCmd::Create { from_snapshot, name, version, os_version, description, cloud_init, default_mem, default_cpus } => {
                println!("{} Creating template '{}' v{} from snapshot {}…",
                    output::blue("→"), name, version,
                    &from_snapshot[..8.min(from_snapshot.len())]);

                let res = client.post("/api/bts/templates", &serde_json::json!({
                    "snapId": from_snapshot, "name": name, "version": version,
                    "osVersion": os_version, "description": description,
                    "cloudInit": cloud_init,
                    "defaultMem": default_mem, "defaultCpus": default_cpus,
                })).await?;

                println!("{} Template created: {}",
                    output::bright("✓"), output::white(res["id"].as_str().unwrap_or("?")));
                println!("  Run {} to make it available for cloning.",
                    output::dim(&format!("caiman template publish {}", res["id"].as_str().unwrap_or(""))));
            }

            TmplCmd::Clone { tmpl_id, name, node, mem, cpus, start } => {
                println!("{} Cloning template {} → {} (COW — < 5s)…",
                    output::blue("→"),
                    output::dim(&tmpl_id[..8.min(tmpl_id.len())]),
                    output::white(&name));

                let res = client.post(&format!("/api/bts/templates/{tmpl_id}/clone"), &serde_json::json!({
                    "sourceId": tmpl_id, "name": name, "node": node,
                    "memMib": mem, "cpus": cpus, "startAfter": start,
                })).await?;

                let vm_id = res["resourceId"].as_str().unwrap_or("?");
                println!("{} VM created: {}  (cloned in < 5s via COW)",
                    output::bright("✓"), output::white(vm_id));
                if start {
                    println!("  VM is booting. {} to watch.",
                        output::dim(&format!("caiman vm logs {vm_id} -f")));
                }
            }

            TmplCmd::Publish { tmpl_id } => {
                client.post_empty(&format!("/api/bts/templates/{tmpl_id}/publish")).await?;
                println!("{} Template {tmpl_id} published — available for cloning",
                    output::bright("✓"));
            }

            TmplCmd::Unpublish { tmpl_id } => {
                client.post_empty(&format!("/api/bts/templates/{tmpl_id}/unpublish")).await?;
                println!("{} Template {tmpl_id} unpublished", output::bright("✓"));
            }

            TmplCmd::Delete { tmpl_id } => {
                let ok = inquire::Confirm::new(
                    &format!("Delete template {tmpl_id}? Existing VMs cloned from it are unaffected.")
                ).with_default(false).prompt()?;
                if ok {
                    client.delete(&format!("/api/bts/templates/{tmpl_id}")).await?;
                    println!("{} Template deleted", output::bright("✓"));
                }
            }
        }
        Ok(())
    }
}
