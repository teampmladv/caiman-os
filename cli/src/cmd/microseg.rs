//! cmd/microseg.rs — micro-segmentation policy commands

use clap::Subcommand;
use anyhow::Result;
use crate::api::Client;
use crate::output::{self, OutputFormat, new_table, color_status};

#[derive(Subcommand)]
pub enum MicrosegCmd {
    /// List micro-segmentation policies
    #[command(subcommand)]
    Policy(PolicyAction),
    /// Show deny statistics
    Stats,
}

#[derive(Subcommand)]
pub enum PolicyAction {
    List,
    Get   { name: String },
    Apply { #[arg(short)] file: String },
}

pub async fn run(cmd: MicrosegCmd, client: &Client, out: OutputFormat) -> Result<()> {
    match cmd {
        MicrosegCmd::Policy(PolicyAction::List) => {
            let res  = client.get("/api/microseg/policies").await?;
            let pols = res["policies"].as_array().map(|a| a.as_slice()).unwrap_or(&[]);
            let mut t = new_table(&["Name", "Priority", "Action", "Hits"]);
            for p in pols {
                t.add_row(vec![
                    output::white(p["name"].as_str().unwrap_or("")).to_string(),
                    p["priority"].to_string(),
                    color_status(p["action"].as_str().unwrap_or("")).to_string(),
                    p["hitCount"].to_string(),
                ]);
            }
            println!("{t}");
        }
        MicrosegCmd::Policy(PolicyAction::Get { name }) => {
            let res = client.get(&format!("/api/microseg/policies/{name}")).await?;
            println!("{}", output::format_json(&res, out)?);
        }
        MicrosegCmd::Policy(PolicyAction::Apply { file }) => {
            let yaml = std::fs::read_to_string(&file)?;
            let body: serde_json::Value = serde_yaml::from_str(&yaml)?;
            client.post("/api/microseg/policies", &body).await?;
            println!("{} Policy applied", output::bright("✓"));
        }
        MicrosegCmd::Stats => {
            let res = client.get("/api/microseg/stats").await?;
            println!("{}", output::format_json(&res, out)?);
        }
    }
    Ok(())
}
