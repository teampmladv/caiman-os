//! drs/src/balancer.rs — Imbalance detection + migration planning + execution
//!
//! Algorithm (equivalent to vSphere DRS):
//!
//!   Every monitor cycle:
//!   1. Compute σ (stddev) of normalized load across nodes
//!   2. If σ < threshold → cluster is balanced, skip
//!   3. For each VM on the most-loaded node:
//!        For each candidate destination node:
//!          Simulate the migration: would it improve balance?
//!          Score = (Δσ * 10) / (migration_cost_factor)
//!          migration_cost_factor ≈ vm_ram_gib * 0.1 (time to migrate)
//!   4. Keep top-K migrations with score > min_score
//!   5. In FullyAutomated mode: execute migrations (max_concurrent per node)
//!   6. In Manual/SemiAutomated: publish as recommendations

use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::process::Command;
use tracing::{info, warn};

use crate::monitor::ClusterSnapshot;
use crate::affinity::AffinityChecker;
use crate::types::{DrsConfig, DrsMode, MigrationCandidate, MigrationPlan, NodeMetrics};

// ── Balancer run loop ──────────────────────────────────────────────────────

pub async fn run(cluster: Arc<RwLock<ClusterSnapshot>>, cfg: DrsConfig) {
    let interval = std::time::Duration::from_secs(cfg.monitor_interval_secs);

    loop {
        tokio::time::sleep(interval).await;

        let snap = cluster.read().await.clone();
        if snap.nodes.is_empty() { continue; }

        let score = snap.balance_score();
        if !snap.is_imbalanced(cfg.imbalance_threshold) {
            info!("Cluster balanced (σ={score:.3} < threshold={:.3})",
                  cfg.imbalance_threshold);
            continue;
        }

        info!("Imbalance detected (σ={score:.3}) — computing migration plan");
        let plan = compute_plan(&snap, &cfg);

        if plan.migrations.is_empty() {
            info!("No beneficial migrations found");
            continue;
        }

        info!("Migration plan: {} recommendations", plan.migrations.len());
        for m in &plan.migrations {
            info!("  VM {} ({}) : {} → {}  score={:.2}  reason={}",
                  m.vm_id, m.vm_name, m.from_node, m.to_node, m.score, m.reason);
        }

        if cfg.mode == DrsMode::FullyAutomated {
            execute_plan(plan, &cfg).await;
        }
    }
}

// ── Migration plan computation ─────────────────────────────────────────────

pub fn compute_plan(snap: &ClusterSnapshot, cfg: &DrsConfig) -> MigrationPlan {
    let mut candidates: Vec<MigrationCandidate> = Vec::new();
    let checker = AffinityChecker::new();

    // Work from most-loaded to least-loaded nodes
    let mut sorted_nodes = snap.nodes.clone();
    sorted_nodes.sort_by(|a, b| b.load_score.partial_cmp(&a.load_score).unwrap());

    let baseline_sigma = snap.balance_score();

    for src_node in &sorted_nodes {
        // Only try to migrate VMs off overloaded nodes
        if src_node.load_score < 0.6 { continue; }

        for vm in &src_node.vms {
            for dst_node in &snap.nodes {
                if dst_node.hostname == src_node.hostname { continue; }

                // Check affinity rules first
                if !checker.migration_allowed(vm, &dst_node.hostname, snap) {
                    continue;
                }

                // Check destination has enough resources
                if !fits_on_node(vm, dst_node) { continue; }

                // Simulate the migration and compute new σ
                let new_sigma = simulate_migration(snap, vm, &src_node.hostname,
                                                   &dst_node.hostname, &cfg.weights);
                let delta_sigma = baseline_sigma - new_sigma;
                if delta_sigma <= 0.0 { continue; }

                // Migration cost: proportional to RAM size (higher RAM = longer migration)
                let ram_gib = vm.mem_mib as f64 / 1024.0;
                let cost_factor = 1.0 + (ram_gib * 0.05);
                let score = delta_sigma / cost_factor;

                if score >= cfg.min_migration_score {
                    candidates.push(MigrationCandidate {
                        vm_id:     vm.vm_id,
                        vm_name:   vm.name.clone(),
                        from_node: src_node.hostname.clone(),
                        to_node:   dst_node.hostname.clone(),
                        reason:    format!(
                            "node load {:.0}% → rebalance (Δσ={delta_sigma:.3})",
                            src_node.load_score * 100.0
                        ),
                        score,
                        estimated_blackout_ms: estimate_blackout_ms(vm.mem_mib),
                    });
                }
            }
        }
    }

    // Sort by score descending, deduplicate (each VM migrates once)
    candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    let mut seen_vms = std::collections::HashSet::new();
    candidates.retain(|c| seen_vms.insert(c.vm_id));

    // Limit to top 5 migrations per rebalance cycle
    candidates.truncate(5);

    MigrationPlan { migrations: candidates }
}

// ── Simulation ─────────────────────────────────────────────────────────────

fn simulate_migration(
    snap:     &ClusterSnapshot,
    vm:       &crate::types::VmMetrics,
    from:     &str,
    to:       &str,
    weights:  &crate::types::ResourceWeights,
) -> f64 {
    // Clone node metrics and shift the VM's resource contribution
    let mut nodes = snap.nodes.clone();

    if let Some(src) = nodes.iter_mut().find(|n| n.hostname == from) {
        src.cpu_usage_pct  -= vm.cpu_usage_pct / src.cpu_cores as f64;
        src.mem_used_mib   -= vm.mem_mib.min(src.mem_used_mib);
        src.compute_load(weights);
    }

    if let Some(dst) = nodes.iter_mut().find(|n| n.hostname == to) {
        dst.cpu_usage_pct  += vm.cpu_usage_pct / dst.cpu_cores as f64;
        dst.mem_used_mib   += vm.mem_mib;
        dst.compute_load(weights);
    }

    // Recompute σ
    if nodes.len() < 2 { return 0.0; }
    let loads: Vec<f64> = nodes.iter().map(|n| n.load_score).collect();
    let mean = loads.iter().sum::<f64>() / loads.len() as f64;
    let variance = loads.iter().map(|l| (l - mean).powi(2)).sum::<f64>() / loads.len() as f64;
    variance.sqrt()
}

fn fits_on_node(vm: &crate::types::VmMetrics, node: &NodeMetrics) -> bool {
    // Check RAM headroom (keep 10% buffer)
    let mem_headroom = node.mem_free_mib;
    if vm.mem_mib > mem_headroom.saturating_sub(node.mem_total_mib / 10) {
        return false;
    }
    // Check CPU headroom
    let cpu_headroom = 100.0 - node.cpu_usage_pct;
    let needed_cpu_pct = vm.cpu_usage_pct / node.cpu_cores as f64;
    if needed_cpu_pct > cpu_headroom - 10.0 {
        return false;
    }
    true
}

fn estimate_blackout_ms(mem_mib: u64) -> u32 {
    // Empirical estimate: ~1ms per 512 MiB of dirty pages (network speed ~10 Gbps)
    // Stop-and-copy phase is typically 50-200ms
    50 + (mem_mib as u32 / 512)
}

// ── Migration execution (FullyAutomated) ───────────────────────────────────

async fn execute_plan(plan: MigrationPlan, cfg: &DrsConfig) {
    // Rate-limit: max N concurrent migrations
    use tokio::sync::Semaphore;
    let sem = Arc::new(Semaphore::new(cfg.max_concurrent_migrations as usize));

    let mut handles = Vec::new();
    for m in plan.migrations {
        let sem    = sem.clone();
        let binary = cfg.livemig_binary.clone();
        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            execute_migration(m, &binary).await;
        }));
    }

    for h in handles { let _ = h.await; }
}

async fn execute_migration(m: MigrationCandidate, binary: &str) {
    info!("Executing migration: VM {} → {}", m.vm_id, m.to_node);
    let result = Command::new(binary)
        .args([
            "--vm-id",     &m.vm_id.to_string(),
            "--destination", &m.to_node,
        ])
        .output()
        .await;

    match result {
        Ok(out) if out.status.success() =>
            info!("Migration VM {} complete", m.vm_id),
        Ok(out) =>
            warn!("Migration VM {} failed: {}",
                  m.vm_id, String::from_utf8_lossy(&out.stderr)),
        Err(e) =>
            warn!("Migration VM {} error: {e}", m.vm_id),
    }
}
