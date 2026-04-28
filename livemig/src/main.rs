//! livemig/src/main.rs — caiman live migration (vMotion equivalent)
//!
//! Migrates a running VM from one node to another with < 1 second downtime.
//!
//! Algorithm (pre-copy, same as KVM/QEMU live migration):
//!
//!   Phase 0 — Setup
//!     Source node opens TCP connection to destination (port 7777)
//!     Destination creates a new VM shell (same config, no vCPUs started)
//!
//!   Phase 1 — Iterative memory copy (VM still running)
//!     Dirty page tracking enabled via KVM_CAP_DIRTY_LOG_RING
//!     Copy all RAM to destination (bandwidth-limited to avoid starving VM)
//!     Subsequent passes copy only pages dirtied since last pass
//!     Converge when dirty set < threshold (default: 50 MiB)
//!
//!   Phase 2 — Stop-and-copy (VM paused, blackout begins)
//!     Pause source VM (stop all vCPUs)
//!     Copy remaining dirty pages + vCPU register state + device state
//!     Migrate XDP BPF maps (identity, policy, mac_to_ifindex)
//!     Transfer on destination: ~50-200ms blackout
//!
//!   Phase 3 — Switchover
//!     Destination resumes VM
//!     Gratuitous ARP sent from destination (IP unchanged)
//!     XDP program on destination NIC updated with VM MAC entry
//!     Source VM deleted
//!
//!   Post-migration
//!     Storage: if VSAN/NVMe-oF, reconnect initiator from destination
//!     Networking: caiman_net_mod notified via netlink on both nodes

use anyhow::{bail, Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

mod proto;
mod memory;
mod network;
mod storage;
mod coord;
mod bpf_migrate;

#[derive(Parser)]
struct Args {
    /// VM ID to migrate
    #[arg(long)]
    vm_id: u32,
    /// Destination node hostname or IP
    #[arg(long)]
    destination: String,
    /// Migration bandwidth limit in Mbps (0 = unlimited)
    #[arg(long, default_value_t = 4000)]
    bandwidth_mbps: u64,
    /// Dirty page convergence threshold in MiB
    #[arg(long, default_value_t = 50)]
    convergence_mib: u64,
    /// Max seconds to spend in iterative phase before forcing stop-and-copy
    #[arg(long, default_value_t = 300)]
    max_iter_secs: u64,
}

// ── Migration state machine ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MigrationPhase {
    Setup,
    IterativeCopy { pass: u32, dirty_pages: u64 },
    Converging,
    StopAndCopy,
    Switchover,
    Complete,
    Failed(String),
}

#[derive(Debug)]
pub struct MigrationJob {
    pub vm_id:      u32,
    pub source:     String,
    pub dest:       String,
    pub phase:      MigrationPhase,
    pub total_ram:  u64,
    pub transferred: u64,
    pub started_at: std::time::Instant,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().init();
    let args = Args::parse();

    info!("Live migration: VM {} → {}", args.vm_id, args.destination);

    let mut job = MigrationJob {
        vm_id:       args.vm_id,
        source:      hostname(),
        dest:        args.destination.clone(),
        phase:       MigrationPhase::Setup,
        total_ram:   0,
        transferred: 0,
        started_at:  std::time::Instant::now(),
    };

    run_migration(&mut job, &args).await
}

async fn run_migration(job: &mut MigrationJob, args: &Args) -> Result<()> {

    // ── Phase 0: Setup ────────────────────────────────────────────────────
    info!("[Phase 0] Connecting to destination {}...", job.dest);
    let (mut src_conn, dest_shell) = coord::setup(job).await
        .context("migration setup")?;
    job.total_ram = dest_shell.ram_bytes;
    job.phase = MigrationPhase::IterativeCopy { pass: 0, dirty_pages: u64::MAX };

    // Enable dirty page ring tracking on source VM
    memory::enable_dirty_tracking(job.vm_id).await
        .context("enabling dirty tracking")?;

    // ── Phase 1: Iterative memory copy ───────────────────────────────────
    info!("[Phase 1] Iterative copy ({} MiB RAM)...", job.total_ram / (1024*1024));
    let deadline = std::time::Instant::now() +
        std::time::Duration::from_secs(args.max_iter_secs);
    let converge_threshold = args.convergence_mib * 1024 * 1024 / 4096; // in pages

    let mut pass = 0u32;
    loop {
        let dirty_pages = memory::copy_dirty_pages(
            job.vm_id, &mut src_conn, args.bandwidth_mbps
        ).await.context("copying dirty pages")?;

        pass += 1;
        job.phase = MigrationPhase::IterativeCopy { pass, dirty_pages };
        info!("  Pass {pass}: {dirty_pages} dirty pages remaining");

        if dirty_pages <= converge_threshold {
            info!("  Converged after {pass} passes");
            break;
        }

        if std::time::Instant::now() > deadline {
            warn!("  Iteration timeout — forcing stop-and-copy");
            break;
        }
    }
    job.phase = MigrationPhase::Converging;

    // ── Phase 2: Stop-and-copy (blackout begins) ─────────────────────────
    info!("[Phase 2] Stop-and-copy (VM pausing)...");
    job.phase = MigrationPhase::StopAndCopy;

    // Pause all vCPUs
    memory::pause_vm(job.vm_id).await.context("pausing VM")?;
    let blackout_start = std::time::Instant::now();

    // Copy remaining dirty pages
    let final_dirty = memory::copy_all_remaining(job.vm_id, &mut src_conn).await?;
    info!("  Final dirty pages copied: {final_dirty}");

    // Save and transfer vCPU register state
    memory::transfer_vcpu_state(job.vm_id, &mut src_conn).await
        .context("transferring vCPU state")?;

    // Migrate BPF maps (XDP identity + policy + mac_to_ifindex)
    bpf_migrate::transfer_bpf_maps(job.vm_id, &job.dest).await
        .context("migrating BPF maps")?;

    // ── Phase 3: Switchover ───────────────────────────────────────────────
    info!("[Phase 3] Switchover...");
    job.phase = MigrationPhase::Switchover;

    // Start VM on destination
    coord::start_destination_vm(&mut src_conn).await
        .context("starting destination VM")?;

    // Update network: gratuitous ARP + XDP map update on destination
    network::switch_network(job.vm_id, &job.dest).await
        .context("network switchover")?;

    // Reconnect storage (VSAN/NVMe-oF) on destination
    storage::reconnect_on_destination(job.vm_id, &job.dest).await
        .context("storage reconnect")?;

    let blackout_ms = blackout_start.elapsed().as_millis();
    let total_secs  = job.started_at.elapsed().as_secs_f32();

    // Cleanup source
    coord::cleanup_source(job.vm_id).await.ok();

    job.phase = MigrationPhase::Complete;
    info!(
        "Migration complete: VM {} → {} | blackout={}ms | total={:.1}s",
        job.vm_id, job.dest, blackout_ms, total_secs
    );

    Ok(())
}

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "unknown".into())
        .trim().to_string()
}
