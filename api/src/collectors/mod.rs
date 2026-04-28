//! collectors/mod.rs — Data collection from all sources
//!
//! CollectorLoop runs every 5 seconds and aggregates:
//!   NodeCollector    → CPU/RAM/disk/net from /proc and sysinfo
//!   VmCollector      → VM state files + KVM ioctl stats
//!   XdpCollector     → caiman_net netlink (RX/TX per VM)
//!   MicrosegCollector→ BPF ring buffer (audit events)
//!   KubeCollector    → Kubernetes API (node labels, pod annotations)
//!   StorageCollector → VSAN + vVols state
//!   GpuCollector     → nvidia-smi + MIG instances

use std::sync::Arc;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};
use tracing::{debug, warn, info};
use chrono::{DateTime, Utc};

use crate::ws::WsEvent;
use crate::state::AppState;

pub mod node;
pub mod vm;
pub mod xdp;
pub mod microseg;
pub mod kube;
pub mod storage;
pub mod gpu;

// ── Cluster state (lives in memory, updated by collectors) ────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterState {
    pub nodes:               Vec<NodeMetrics>,
    pub vms:                 Vec<VmMetrics>,
    pub balance_sigma:       f64,
    pub drs_mode:            String,
    pub total_cpu_pct:       f64,
    pub total_mem_used_mib:  u64,
    pub total_mem_mib:       u64,
    pub xdp_throughput_gbps: f64,
    pub xdp_drops_total:     u64,
    pub microseg_denies_60s: u64,
    pub updated_at:          DateTime<Utc>,
}

impl ClusterState {
    pub fn recompute_totals(&mut self) {
        let running: Vec<&VmMetrics> = self.vms.iter()
            .filter(|v| v.status == "RUNNING")
            .collect();

        self.total_cpu_pct = if running.is_empty() { 0.0 } else {
            running.iter().map(|v| v.cpu_usage_pct).sum::<f64>() / running.len() as f64
        };

        self.total_mem_used_mib = self.nodes.iter().map(|n| n.mem_used_mib).sum();
        self.total_mem_mib = self.nodes.iter().map(|n| n.mem_total_mib).sum();

        self.xdp_throughput_gbps = running.iter()
            .map(|v| (v.net_rx_mbps + v.net_tx_mbps) / 1000.0)
            .sum();

        self.xdp_drops_total = running.iter().map(|v| v.net_rx_drops).sum();

        // Compute DRS balance sigma
        let scores: Vec<f64> = self.nodes.iter().map(|n| n.load_score).collect();
        if scores.len() >= 2 {
            let mean = scores.iter().sum::<f64>() / scores.len() as f64;
            let variance = scores.iter().map(|s| (s - mean).powi(2)).sum::<f64>()
                / scores.len() as f64;
            self.balance_sigma = variance.sqrt();
        }

        self.updated_at = Utc::now();
    }
}

// ── Core metric types ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub id:               String,
    pub hostname:         String,
    pub status:           String,
    pub cpu_cores:        u32,
    pub cpu_usage_pct:    f64,
    pub mem_total_mib:    u64,
    pub mem_used_mib:     u64,
    pub disk_read_iops:   f64,
    pub disk_write_iops:  f64,
    pub net_rx_mbps:      f64,
    pub net_tx_mbps:      f64,
    pub load_score:       f64,
    pub vm_count:         u32,
    pub vms:              Vec<String>,
    pub kernel_version:   String,
    pub caiman_version:   String,
    pub uptime_secs:      u64,
}

impl NodeMetrics {
    pub fn compute_load(&mut self, weights: &LoadWeights) {
        let cpu_n  = (self.cpu_usage_pct / 100.0).min(1.0);
        let mem_n  = if self.mem_total_mib > 0 {
            (self.mem_used_mib as f64 / self.mem_total_mib as f64).min(1.0)
        } else { 0.0 };
        let disk_n = ((self.disk_read_iops + self.disk_write_iops) / 100_000.0).min(1.0);
        let net_n  = ((self.net_rx_mbps + self.net_tx_mbps) / 40_000.0).min(1.0);

        self.load_score = cpu_n  * weights.cpu
                        + mem_n  * weights.memory
                        + disk_n * weights.storage
                        + net_n  * weights.network;

        self.status = if self.load_score > 0.80 { "CRITICAL".into() }
            else if self.load_score > 0.65      { "HIGH_LOAD".into() }
            else                                { "HEALTHY".into() };
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VmMetrics {
    pub id:               String,
    pub name:             String,
    pub status:           String,
    pub node_id:          String,
    pub node_name:        String,
    pub cpu_cores:        u32,
    pub cpu_usage_pct:    f64,
    pub mem_mib:          u64,
    pub mem_total_mib:    u64,
    pub disk_read_iops:   f64,
    pub disk_write_iops:  f64,
    pub net_rx_mbps:      f64,
    pub net_tx_mbps:      f64,
    pub net_rx_drops:     u64,
    pub uptime_secs:      u64,
    pub mac:              String,
    pub labels:           HashMap<String, String>,
    pub started_at:       String,
    pub migrating:        Option<MigrationStatus>,
    pub pid:              Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MigrationStatus {
    pub phase:           String,
    pub from_node:       String,
    pub to_node:         String,
    pub progress_pct:    f64,
    pub elapsed_secs:    u64,
    pub blackout_ms:     Option<u32>,
}

#[derive(Debug, Clone)]
pub struct LoadWeights {
    pub cpu:     f64,
    pub memory:  f64,
    pub storage: f64,
    pub network: f64,
}

impl Default for LoadWeights {
    fn default() -> Self {
        Self { cpu: 0.5, memory: 0.3, storage: 0.1, network: 0.1 }
    }
}

// ── Collector loop ────────────────────────────────────────────────────────

pub struct CollectorLoop {
    state:    Arc<AppState>,
    event_tx: broadcast::Sender<WsEvent>,
    weights:  LoadWeights,
    interval_secs: u64,
}

impl CollectorLoop {
    pub fn new(state: Arc<AppState>, event_tx: broadcast::Sender<WsEvent>) -> Self {
        Self {
            state,
            event_tx,
            weights: LoadWeights::default(),
            interval_secs: std::env::var("CAIMAN_COLLECT_INTERVAL_SECS")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(5),
        }
    }

    pub async fn run(self) {
        info!("CollectorLoop: starting (interval={}s)", self.interval_secs);
        let mut ticker = interval(Duration::from_secs(self.interval_secs));

        loop {
            ticker.tick().await;
            if let Err(e) = self.collect_once().await {
                warn!("CollectorLoop error: {e:#}");
            }
        }
    }

    async fn collect_once(&self) -> anyhow::Result<()> {
        let start = std::time::Instant::now();

        // 1. Collect data from all sources in parallel
        let (nodes_res, vms_res) = tokio::join!(
            node::collect(),
            vm::collect(),
        );

        let mut nodes = nodes_res.unwrap_or_default();
        let vms       = vms_res.unwrap_or_default();

        // 2. Enrich nodes with XDP stats from caiman_net
        if let Ok(xdp_stats) = xdp::collect_all().await {
            for node in &mut nodes {
                for vm_id in &node.vms {
                    if let Some(stats) = xdp_stats.get(vm_id) {
                        node.net_rx_mbps += stats.rx_mbps;
                        node.net_tx_mbps += stats.tx_mbps;
                    }
                }
            }
        }

        // 3. Compute node load scores
        for node in &mut nodes {
            node.compute_load(&self.weights);
        }

        // 4. Update state and broadcast diffs
        {
            let mut state = self.state.cluster.write().await;
            let old = state.clone();

            state.nodes = nodes;
            state.vms   = vms;
            state.recompute_totals();

            // Broadcast per-VM metric updates
            for vm in &state.vms {
                let old_vm = old.vms.iter().find(|v| v.id == vm.id);
                let changed = old_vm.map_or(true, |ov| {
                    (ov.cpu_usage_pct - vm.cpu_usage_pct).abs() > 0.5
                    || ov.net_rx_mbps != vm.net_rx_mbps
                    || ov.status != vm.status
                });

                if changed {
                    let _ = self.event_tx.send(WsEvent::VmMetricsUpdate {
                        id:            vm.id.clone(),
                        cpu_usage_pct: vm.cpu_usage_pct,
                        net_rx_mbps:   vm.net_rx_mbps,
                        net_tx_mbps:   vm.net_tx_mbps,
                        mem_mib:       vm.mem_mib,
                    });
                }

                // Status change event
                if old_vm.map_or(false, |ov| ov.status != vm.status) {
                    let _ = self.event_tx.send(WsEvent::VmStatusChange {
                        id:         vm.id.clone(),
                        status:     vm.status.clone(),
                        migrating:  vm.migrating.clone(),
                    });
                }
            }

            // Broadcast per-node updates
            for node in &state.nodes {
                let _ = self.event_tx.send(WsEvent::NodeMetricsUpdate {
                    id:            node.id.clone(),
                    cpu_usage_pct: node.cpu_usage_pct,
                    mem_used_mib:  node.mem_used_mib,
                    load_score:    node.load_score,
                });
            }
        }

        debug!("CollectorLoop: cycle complete in {:?}", start.elapsed());
        Ok(())
    }
}
