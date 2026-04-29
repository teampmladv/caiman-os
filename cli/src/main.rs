/// caiman — Caimán OS command-line interface
///
/// Usage examples:
///   caiman vm list
///   caiman vm start vm-001
///   caiman vm migrate vm-001 --to node-03
///   caiman vm console vm-001
///   caiman drs status
///   caiman drs exec --all
///   caiman microseg policy list
///   caiman microseg policy apply -f policy.yaml
///   caiman storage vsan create --name pgdata --size 500 --ftt 1
///   caiman gpu list
///   caiman cluster status
///   caiman events --follow
///   caiman config set api-url http://caiman-api:8765

use clap::{Parser, Subcommand};
use anyhow::Result;
use tracing_subscriber::EnvFilter;

mod api;
mod cmd;
mod config;
mod output;

use config::CliConfig;
use output::OutputFormat;

// ── Top-level CLI ─────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name    = "caiman",
    version = env!("CARGO_PKG_VERSION"),
    about   = "🐊 Caimán OS — Hypervisor management CLI",
    long_about = "Manage VMs, DRS, micro-segmentation, storage and GPU\
                  from the terminal.\n\
                  Named after the Cuban crocodile. Built for the cloud.",
    propagate_version = true,
)]
pub struct Cli {
    /// API endpoint (overrides config)
    #[arg(long, env = "CAIMAN_API_URL", global = true)]
    api_url: Option<String>,

    /// Output format: table (default), json, yaml, wide
    #[arg(long, short = 'o', default_value = "table", global = true)]
    output: OutputFormat,

    /// Disable colour output
    #[arg(long, global = true)]
    no_color: bool,

    /// Show verbose request/response info
    #[arg(long, short = 'v', global = true)]
    verbose: bool,

    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// VM lifecycle: list, start, stop, restart, migrate, console, logs, delete
    #[command(subcommand)]
    Vm(cmd::vm::VmCmd),

    /// Cluster nodes: list, status, drain, uncordon
    #[command(subcommand)]
    Node(cmd::node::NodeCmd),

    /// Distributed Resource Scheduler: status, recommendations, exec
    #[command(subcommand)]
    Drs(cmd::drs::DrsCmd),

    /// Micro-segmentation: policy list/apply/delete, audit, stats
    #[command(subcommand)]
    Microseg(cmd::microseg::MicrosegCmd),

    /// Storage: VSAN volumes, vVols, snapshots
    #[command(subcommand)]
    Storage(cmd::storage::StorageCmd),

    /// GPU: list devices, allocate MIG, release
    #[command(subcommand)]
    Gpu(cmd::gpu::GpuCmd),

    /// Cluster overview: status, metrics, events
    #[command(subcommand)]
    Cluster(cmd::cluster::ClusterCmd),

    /// Live event stream (WebSocket tail)
    Events {
        /// Follow the stream (Ctrl+C to stop)
        #[arg(long, short = 'f')]
        follow: bool,
        /// Filter by event type: vm, node, drs, microseg, alert
        #[arg(long)]
        filter: Option<String>,
        /// Number of past events to show before tailing
        #[arg(long, default_value = "20")]
        tail: usize,
    },

    /// Interactive TUI dashboard
    Tui,

    /// Manage CLI configuration
    #[command(subcommand)]
    Config(cmd::config::ConfigCmd),

    /// API health check
    Ping,
}

// ── Entry point ───────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Tracing
    let level = if cli.verbose { "debug" } else { "warn" };
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(level))
        .with_writer(std::io::stderr)
        .without_time()
        .init();

    if cli.no_color {
        colored::control::set_override(false);
    }

    // Load config + resolve API URL
    let cfg  = CliConfig::load()?;
    let base = cli.api_url
        .or_else(|| cfg.api_url.clone())
        .unwrap_or_else(|| "http://localhost:8765".into());

    let client = api::Client::new(base, cfg.token.clone(), cli.verbose);
    let out    = cli.output;

    match cli.cmd {
        Commands::Vm(c)       => cmd::vm::run(c, &client, out).await,
        Commands::Node(c)     => cmd::node::run(c, &client, out).await,
        Commands::Drs(c)      => cmd::drs::run(c, &client, out).await,
        Commands::Microseg(c) => cmd::microseg::run(c, &client, out).await,
        Commands::Storage(c)  => cmd::storage::run(c, &client, out).await,
        Commands::Gpu(c)      => cmd::gpu::run(c, &client, out).await,
        Commands::Cluster(c)  => cmd::cluster::run(c, &client, out).await,

        Commands::Events { follow, filter, tail } =>
            cmd::cluster::events(&client, follow, filter, tail).await,

        Commands::Tui =>
            cmd::tui::run(&client).await,

        Commands::Config(c)   => cmd::config::run(c, &cfg).await,



        Commands::Ping => {
            let res = client.get("/health").await?;
            println!("{}", output::format_json(&res, out)?);
            Ok(())
        }
    }
}
