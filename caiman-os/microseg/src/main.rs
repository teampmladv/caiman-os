//! microseg/src/main.rs — caiman micro-segmentation policy engine
//!
//! Architecture (NSX-T equivalent, kernel-native):
//!
//!   MicroSegPolicy CRD (Kubernetes)
//!         │
//!   Policy compiler (this crate)
//!         │  compiles label selectors → (src_id, dst_id, proto, port) tuples
//!         ▼
//!   BPF maps in /sys/fs/bpf/caiman-microseg/
//!     identity_map      MAC → identity
//!     policy_map        (src,dst,proto,port) → verdict
//!     default_policy    identity → default verdict
//!     deny_stats        identity → dropped packets/bytes
//!     audit_ringbuf     denied flow events → userspace
//!         │
//!   xdp_microseg.o (XDP program — FIRST program on every NIC)
//!         │
//!   Network traffic (enforced at XDP layer, ~5µs decision time)
//!
//! Zero-trust model:
//!   - All VM-to-VM traffic is DENY by default
//!   - Explicit ALLOW rules required for each communication path
//!   - Rules are label-based (not IP-based) — survive VM restarts
//!   - Audit log of every denied flow via BPF ring buffer

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

mod audit;
mod compiler;
mod identity;
mod maps;
mod policy;
mod watcher;

#[derive(Parser)]
#[command(name = "caiman-microseg", about = "caiman micro-segmentation policy engine")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the policy engine (watches CRDs, compiles to BPF maps)
    Run {
        #[arg(long, default_value = "/sys/fs/bpf/caiman-microseg")]
        bpf_pin_path: String,
        #[arg(long, default_value = "eth0")]
        uplink: String,
    },
    /// Print current policy table
    Show,
    /// Print denied flow statistics
    Stats,
    /// Tail the audit log (denied flows in real time)
    Audit,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_writer(std::io::stderr).init();
    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Run { bpf_pin_path, uplink } => run(bpf_pin_path, uplink).await,
        Cmd::Show  => policy::show().await,
        Cmd::Stats => maps::print_deny_stats().await,
        Cmd::Audit => audit::tail_ringbuf().await,
    }
}

async fn run(bpf_pin_path: String, uplink: String) -> Result<()> {
    info!("caiman-microseg: starting policy engine");

    // Initialize BPF maps
    maps::init_maps(&bpf_pin_path).context("initializing BPF maps")?;

    // Load and attach XDP program (replaces plain xdp_vm_router)
    maps::attach_xdp_program(&uplink, &bpf_pin_path)
        .await
        .context("attaching XDP microseg program")?;

    // Start Kubernetes CRD watcher
    let (policy_tx, policy_rx) = tokio::sync::mpsc::channel(64);
    tokio::spawn(watcher::watch_policies(policy_tx));

    // Start audit log consumer
    tokio::spawn(audit::consume_ringbuf(bpf_pin_path.clone()));

    // Main loop: receive policy updates, compile to BPF maps
    compiler::policy_loop(policy_rx, &bpf_pin_path).await
}
