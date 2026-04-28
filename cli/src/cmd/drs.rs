//! cmd/drs.rs
use clap::Subcommand;
use anyhow::Result;
use crate::api::Client;
use crate::output::{self, OutputFormat, new_table, color_sigma, color_score};

#[derive(Subcommand)]
pub enum DrsCmd {
    /// Show cluster balance status and pending recommendations
    Status,
    /// List pending migration recommendations
    Recommendations,
    /// Execute a specific or all DRS migrations
    Exec {
        vm_id: Option<String>,
        #[arg(long)] all: bool,
        #[arg(long)] dry_run: bool,
    },
    /// Get or set DRS operating mode
    Mode {
        #[arg(value_enum)]
        mode: Option<DrsMode>,
    },
    /// Show affinity rules
    Rules,
    /// Show resource pool hierarchy
    Pools,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum DrsMode { Manual, SemiAutomated, FullyAutomated }

pub async fn run(cmd: DrsCmd, client: &Client, out: OutputFormat) -> Result<()> {
    match cmd {
        DrsCmd::Status => {
            let res = client.get("/api/cluster").await?;
            let sigma   = res["balanceSigma"].as_f64().unwrap_or(0.0);
            let mode    = res["drsMode"].as_str().unwrap_or("?");
            let nodes   = res["nodes"].as_array().map(|a| a.len()).unwrap_or(0);
            let imbal   = sigma > 0.10;

            println!();
            println!("  {} DRS Balance Report", output::bright("🐊"));
            println!("  {}", output::dim("─".repeat(40)));
            println!("  Balance σ      {}  {}",
                color_sigma(sigma),
                if imbal { output::amber("⚠ IMBALANCED").to_string() }
                else     { output::bright("✓ BALANCED").to_string() });
            println!("  Mode           {}", output::white(mode));
            println!("  Nodes          {}", output::bright(&nodes.to_string()));

            if imbal {
                let recs = client.get("/api/drs/recommendations").await?;
                let recs = recs["recommendations"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
                println!();
                println!("  {} Recommendations:", output::amber("→"));
                for r in recs {
                    println!("    {} {} → {}  score={}  {}",
                        output::bright("▸"),
                        output::white(r["vmName"].as_str().unwrap_or("")),
                        output::blue(r["toNode"].as_str().unwrap_or("")),
                        color_score(r["score"].as_f64().unwrap_or(0.0)),
                        output::dim(r["reason"].as_str().unwrap_or("")),
                    );
                }
                println!();
                println!("  Run {} to execute all",
                    output::dim("caiman drs exec --all"));
            }
            println!();
        }

        DrsCmd::Recommendations => {
            let res  = client.get("/api/drs/recommendations").await?;
            let recs = res["recommendations"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
            let mut t = new_table(&["Score", "VM", "From", "To", "Blackout", "Reason"]);
            for r in recs {
                t.add_row(vec![
                    color_score(r["score"].as_f64().unwrap_or(0.0)).to_string(),
                    output::white(r["vmName"].as_str().unwrap_or("")).to_string(),
                    r["fromNode"].as_str().unwrap_or("").to_string(),
                    output::blue(r["toNode"].as_str().unwrap_or("")).to_string(),
                    format!("~{}ms", r["estimatedBlackoutMs"].as_u64().unwrap_or(0)),
                    output::dim(r["reason"].as_str().unwrap_or("")).to_string(),
                ]);
            }
            println!("{t}");
        }

        DrsCmd::Exec { vm_id, all, dry_run } => {
            if dry_run {
                println!("{} Dry run — no migrations will be executed", output::amber("!"));
                return Ok(());
            }
            if all {
                let confirm = inquire::Confirm::new(
                    "Execute ALL DRS recommendations? Multiple VMs will experience brief downtime."
                ).with_default(false).prompt()?;
                if !confirm { return Ok(()); }
                client.post_empty("/api/drs/execute-all").await?;
                println!("{} All DRS migrations started", output::bright("✓"));
            } else if let Some(id) = vm_id {
                client.post_empty(&format!("/api/drs/execute/{id}")).await?;
                println!("{} Migration started for {id}", output::bright("✓"));
            } else {
                eprintln!("{} Specify a vm_id or use --all", output::red("✗"));
            }
        }

        DrsCmd::Mode { mode } => {
            if let Some(m) = mode {
                let mode_str = format!("{m:?}");
                client.patch("/api/drs/config", &serde_json::json!({ "mode": mode_str })).await?;
                println!("{} DRS mode set to {}", output::bright("✓"), output::white(&mode_str));
            } else {
                let res = client.get("/api/drs/config").await?;
                println!("Mode: {}", output::bright(res["mode"].as_str().unwrap_or("?")));
            }
        }

        DrsCmd::Rules => {
            let res = client.get("/api/drs/rules").await?;
            println!("{}", output::format_json(&res, out)?);
        }

        DrsCmd::Pools => {
            let res = client.get("/api/drs/pools").await?;
            println!("{}", output::format_json(&res, out)?);
        }
    }
    Ok(())
}
