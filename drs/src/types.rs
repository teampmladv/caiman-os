//! drs/src/types.rs — shared types for DRS

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Operating mode ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DrsMode {
    /// Compute recommendations only — never trigger migrations
    Manual,
    /// Auto-place new VMs; suggest (not execute) rebalance migrations
    SemiAutomated,
    /// Auto-place new VMs + auto-execute rebalance migrations
    FullyAutomated,
}

// ── Configuration ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct DrsConfig {
    /// Operating mode
    #[serde(default = "default_mode")]
    pub mode: DrsMode,

    /// Imbalance threshold: trigger rebalance when σ of normalized load > this
    /// vSphere default equivalent: ~0.1 (10% standard deviation)
    #[serde(default = "default_threshold")]
    pub imbalance_threshold: f64,

    /// Monitor interval in seconds
    #[serde(default = "default_monitor_interval")]
    pub monitor_interval_secs: u64,

    /// Max concurrent live migrations per node
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_migrations: u8,

    /// Minimum score for a migration to be approved
    #[serde(default = "default_min_score")]
    pub min_migration_score: f64,

    /// Weights for the imbalance score function
    #[serde(default)]
    pub weights: ResourceWeights,

    /// Path to livemig binary
    #[serde(default = "default_livemig_bin")]
    pub livemig_binary: String,

    /// Kubernetes namespace to watch for VM pods
    #[serde(default = "default_namespace")]
    pub namespace: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceWeights {
    pub cpu:     f64,
    pub memory:  f64,
    pub storage: f64,
    pub network: f64,
}

impl Default for ResourceWeights {
    fn default() -> Self {
        // vSphere-equivalent default weighting
        Self { cpu: 0.5, memory: 0.3, storage: 0.1, network: 0.1 }
    }
}

fn default_mode()              -> DrsMode { DrsMode::FullyAutomated }
fn default_threshold()         -> f64 { 0.10 }
fn default_monitor_interval()  -> u64 { 30 }
fn default_max_concurrent()    -> u8  { 2 }
fn default_min_score()         -> f64 { 0.25 }
fn default_livemig_bin()       -> String { "/usr/local/bin/kvm-livemig".into() }
fn default_namespace()         -> String { "default".into() }

impl DrsConfig {
    pub fn load() -> anyhow::Result<Self> {
        let cfg = config::Config::builder()
            .add_source(config::File::with_name("caiman-drs").required(false))
            .add_source(config::Environment::with_prefix("CAIMAN_DRS").separator("__"))
            .build()?
            .try_deserialize()?;
        Ok(cfg)
    }
}

// ── Per-VM resource snapshot ───────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmMetrics {
    pub vm_id:          u32,
    pub name:           String,
    pub node:           String,
    pub labels:         HashMap<String, String>,

    // CPU
    pub cpu_cores:      u32,
    pub cpu_usage_pct:  f64,   // 0.0 - 100.0 × cores
    pub cpu_ready_ms:   f64,   // ms/s spent waiting for CPU (like vSphere CPU Ready)

    // Memory
    pub mem_mib:        u64,
    pub mem_active_mib: u64,   // pages accessed in last measurement window
    pub mem_ballooned:  u64,   // ballooned out by guest balloon driver
    pub mem_swapped:    u64,   // swapped to host swap

    // Storage
    pub disk_read_iops:  f64,
    pub disk_write_iops: f64,
    pub disk_read_mbps:  f64,
    pub disk_write_mbps: f64,
    pub disk_latency_ms: f64,

    // Network (from XDP stats via caiman_net_mod netlink)
    pub net_rx_mbps:    f64,
    pub net_tx_mbps:    f64,
    pub net_rx_drops:   u64,   // XDP drops = policy denials or overload
}

// ── Per-node resource snapshot ─────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub hostname:       String,
    pub cpu_cores:      u32,
    pub cpu_usage_pct:  f64,
    pub mem_total_mib:  u64,
    pub mem_used_mib:   u64,
    pub mem_free_mib:   u64,
    pub disk_read_iops: f64,
    pub disk_write_iops:f64,
    pub net_rx_mbps:    f64,
    pub net_tx_mbps:    f64,
    pub vms:            Vec<VmMetrics>,
    /// Normalized composite load score [0.0 - 1.0]
    pub load_score:     f64,
}

impl NodeMetrics {
    /// Compute normalized composite load using configured weights.
    pub fn compute_load(&mut self, weights: &ResourceWeights) {
        let cpu_norm  = (self.cpu_usage_pct / 100.0).min(1.0);
        let mem_norm  = (self.mem_used_mib as f64 / self.mem_total_mib.max(1) as f64).min(1.0);
        let disk_norm = ((self.disk_read_iops + self.disk_write_iops) / 100_000.0).min(1.0);
        let net_norm  = ((self.net_rx_mbps + self.net_tx_mbps) / 40_000.0).min(1.0);

        self.load_score = cpu_norm  * weights.cpu
                        + mem_norm  * weights.memory
                        + disk_norm * weights.storage
                        + net_norm  * weights.network;
    }

    pub fn available_cpu_cores(&self) -> f64 {
        self.cpu_cores as f64 * (1.0 - self.cpu_usage_pct / 100.0)
    }

    pub fn available_mem_mib(&self) -> u64 {
        self.mem_free_mib
    }
}

// ── Migration candidate ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationCandidate {
    pub vm_id:     u32,
    pub vm_name:   String,
    pub from_node: String,
    pub to_node:   String,
    pub reason:    String,
    /// Score [0.0 - 1.0]: higher = more beneficial
    pub score:     f64,
    pub estimated_blackout_ms: u32,
}

#[derive(Debug, Default)]
pub struct MigrationPlan {
    pub migrations: Vec<MigrationCandidate>,
}
