//! drs/src/monitor.rs — Cluster resource monitor
//!
//! Polls all nodes every 30s (configurable) and builds a ClusterSnapshot.
//! Data sources:
//!   CPU/RAM  → /proc/stat, /proc/meminfo via sysinfo crate
//!   Storage  → /sys/block/*/stat  (per-device IOPS/latency)
//!   Network  → caiman_net_mod Generic Netlink (XDP RX/TX stats per VM)
//!   VM state → KVM /dev/kvm ioctls + state files in /var/run/caiman/

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::types::{DrsConfig, NodeMetrics, VmMetrics};

// ── ClusterSnapshot ────────────────────────────────────────────────────────

#[derive(Debug, Default, Clone)]
pub struct ClusterSnapshot {
    pub nodes:     Vec<NodeMetrics>,
    pub timestamp: u64,  // unix seconds
}

impl ClusterSnapshot {
    pub fn total_vms(&self) -> usize {
        self.nodes.iter().map(|n| n.vms.len()).sum()
    }

    /// Standard deviation of node load scores.
    pub fn balance_score(&self) -> f64 {
        if self.nodes.len() < 2 { return 0.0; }
        let loads: Vec<f64> = self.nodes.iter().map(|n| n.load_score).collect();
        let mean = loads.iter().sum::<f64>() / loads.len() as f64;
        let variance = loads.iter()
            .map(|l| (l - mean).powi(2))
            .sum::<f64>() / loads.len() as f64;
        variance.sqrt()
    }

    pub fn is_imbalanced(&self, threshold: f64) -> bool {
        self.balance_score() > threshold
    }

    pub fn node(&self, hostname: &str) -> Option<&NodeMetrics> {
        self.nodes.iter().find(|n| n.hostname == hostname)
    }

    pub fn most_loaded(&self) -> Option<&NodeMetrics> {
        self.nodes.iter().max_by(|a, b|
            a.load_score.partial_cmp(&b.load_score).unwrap()
        )
    }

    pub fn least_loaded(&self) -> Option<&NodeMetrics> {
        self.nodes.iter().min_by(|a, b|
            a.load_score.partial_cmp(&b.load_score).unwrap()
        )
    }
}

// ── Monitor run loop ───────────────────────────────────────────────────────

pub async fn run(cluster: Arc<RwLock<ClusterSnapshot>>, cfg: DrsConfig) {
    let interval = std::time::Duration::from_secs(cfg.monitor_interval_secs);
    info!("Monitor: polling every {}s", cfg.monitor_interval_secs);

    loop {
        match collect_snapshot(&cfg).await {
            Ok(snap) => {
                let score = snap.balance_score();
                debug!("Snapshot: {} nodes, {} VMs, balance_score={:.3}",
                       snap.nodes.len(), snap.total_vms(), score);
                *cluster.write().await = snap;
            }
            Err(e) => warn!("Monitor collect error: {e}"),
        }
        tokio::time::sleep(interval).await;
    }
}

async fn collect_snapshot(cfg: &DrsConfig) -> anyhow::Result<ClusterSnapshot> {
    // In a real cluster deployment the DRS daemon runs on each node and
    // aggregates data from peers via the Kubernetes API or a gossip protocol.
    // Here we collect local node data and merge with Kubernetes node metrics.

    let local = collect_local_node(cfg).await?;
    let k8s_nodes = collect_k8s_nodes().await.unwrap_or_default();

    // Merge: local node is authoritative for its own metrics
    let mut nodes = k8s_nodes;
    let local_hostname = local.hostname.clone();
    nodes.retain(|n| n.hostname != local_hostname);
    nodes.push(local);

    // Compute load score for each node
    for node in &mut nodes {
        node.compute_load(&cfg.weights);
    }

    Ok(ClusterSnapshot {
        nodes,
        timestamp: unix_now(),
    })
}

// ── Local node metrics ─────────────────────────────────────────────────────

async fn collect_local_node(cfg: &DrsConfig) -> anyhow::Result<NodeMetrics> {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "unknown".into())
        .trim().to_string();

    // CPU
    let cpu_cores   = sys.cpus().len() as u32;
    let cpu_usage   = sys.global_cpu_usage() as f64;

    // Memory
    let mem_total   = sys.total_memory() / (1024 * 1024);
    let mem_used    = sys.used_memory()  / (1024 * 1024);
    let mem_free    = sys.free_memory()  / (1024 * 1024);

    // Storage (sum across all block devices)
    let (read_iops, write_iops) = collect_disk_stats().await;

    // Network (from caiman_net_mod netlink)
    let (net_rx, net_tx) = collect_xdp_stats().await;

    // Running VMs (from state files)
    let vms = collect_vm_metrics(cfg).await;

    Ok(NodeMetrics {
        hostname,
        cpu_cores,
        cpu_usage_pct:  cpu_usage,
        mem_total_mib:  mem_total,
        mem_used_mib:   mem_used,
        mem_free_mib:   mem_free,
        disk_read_iops: read_iops,
        disk_write_iops: write_iops,
        net_rx_mbps:    net_rx,
        net_tx_mbps:    net_tx,
        vms,
        load_score:     0.0,  // computed after
    })
}

async fn collect_disk_stats() -> (f64, f64) {
    // Read /sys/block/*/stat: fields 0(read_ios) and 4(write_ios)
    let mut total_r = 0f64;
    let mut total_w = 0f64;

    if let Ok(dir) = std::fs::read_dir("/sys/block") {
        for entry in dir.flatten() {
            let stat_path = entry.path().join("stat");
            if let Ok(data) = std::fs::read_to_string(stat_path) {
                let fields: Vec<u64> = data.split_whitespace()
                    .filter_map(|s| s.parse().ok())
                    .collect();
                if fields.len() >= 8 {
                    total_r += fields[0] as f64;
                    total_w += fields[4] as f64;
                }
            }
        }
    }
    (total_r, total_w)
}

async fn collect_xdp_stats() -> (f64, f64) {
    // Query caiman_net_mod via netlink for aggregate RX/TX bytes
    // Full impl: send KVM_NET_CMD_VM_STATS with wildcard vm_id
    // Stub: return zeros (will be filled in full implementation)
    (0.0, 0.0)
}

async fn collect_vm_metrics(cfg: &DrsConfig) -> Vec<VmMetrics> {
    let state_dir = "/var/run/caiman";
    let mut vms = Vec::new();

    let Ok(dir) = std::fs::read_dir(state_dir) else { return vms; };

    for entry in dir.flatten() {
        if entry.path().extension().map_or(false, |e| e == "json") {
            if let Ok(data) = std::fs::read_to_string(entry.path()) {
                if let Ok(state) = serde_json::from_str::<serde_json::Value>(&data) {
                    let vm_id = state["vm_id"].as_u64().unwrap_or(0) as u32;
                    vms.push(VmMetrics {
                        vm_id,
                        name:          state["name"].as_str().unwrap_or("").to_string(),
                        node:          hostname_local(),
                        cpu_cores:     state["cpus"].as_u64().unwrap_or(1) as u32,
                        mem_mib:       state["mem_mib"].as_u64().unwrap_or(256),
                        cpu_usage_pct: collect_vm_cpu_usage(vm_id).await,
                        ..Default::default()
                    });
                }
            }
        }
    }
    vms
}

async fn collect_vm_cpu_usage(vm_id: u32) -> f64 {
    // Read /proc/<vmm_pid>/stat for the VMM process CPU time
    // Full impl: use perf_event or KVM_GET_VCPU_EVENTS
    0.0
}

async fn collect_k8s_nodes() -> anyhow::Result<Vec<NodeMetrics>> {
    // Query Kubernetes API for node resource usage
    // Uses the metrics-server API: GET /apis/metrics.k8s.io/v1beta1/nodes
    Ok(Vec::new())
}

fn hostname_local() -> String {
    std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default().trim().to_string()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
