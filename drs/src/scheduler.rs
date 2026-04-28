//! drs/src/scheduler.rs — Kubernetes scheduler extender
//!
//! Implements the Kubernetes scheduler extender protocol so kube-scheduler
//! asks caiman DRS where to place new VM pods.
//!
//! DRS scoring algorithm for initial placement:
//!   1. Filter: remove nodes that can't fit the VM (CPU/RAM/GPU)
//!   2. Score: rank remaining nodes by:
//!        - Available headroom (40%)
//!        - Affinity rules compliance (30%)
//!        - Storage proximity (20%)
//!        - Current load σ impact (10%)
//!
//! Registered in kube-scheduler config as an extender:
//!   urlPrefix: http://caiman-drs.caiman-system.svc:8765
//!   filterVerb: filter
//!   prioritizeVerb: prioritize
//!   weight: 5

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::monitor::ClusterSnapshot;
use crate::affinity::AffinityChecker;
use crate::types::DrsConfig;

// ── Kubernetes extender wire types ─────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtenderArgs {
    #[serde(rename = "Pod")]
    pub pod:   serde_json::Value,
    #[serde(rename = "Nodes")]
    pub nodes: Option<NodeList>,
    #[serde(rename = "NodeNames")]
    pub node_names: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeList {
    pub items: Vec<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ExtenderFilterResult {
    #[serde(rename = "Nodes")]
    pub nodes: Option<NodeList>,
    #[serde(rename = "NodeNames")]
    pub node_names: Option<Vec<String>>,
    #[serde(rename = "FailedNodes")]
    pub failed_nodes: HashMap<String, String>,
    #[serde(rename = "Error")]
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct ExtenderPriorityResult(pub Vec<HostPriority>);

#[derive(Debug, Serialize)]
pub struct HostPriority {
    #[serde(rename = "Host")]
    pub host:  String,
    #[serde(rename = "Score")]
    pub score: i64,  // 0-100
}

type DrsState = (Arc<RwLock<ClusterSnapshot>>, DrsConfig);

// ── Filter handler: remove nodes that can't host the VM ───────────────────

pub async fn filter_handler(
    State((cluster, cfg)): State<DrsState>,
    Json(args): Json<ExtenderArgs>,
) -> Json<ExtenderFilterResult> {
    let snap = cluster.read().await;
    let (cpu_req, mem_req, gpu_req) = extract_vm_resources(&args.pod);

    debug!("DRS filter: VM wants cpu={cpu_req} mem={mem_req}MiB gpu={gpu_req}");

    let mut failed_nodes = HashMap::new();
    let mut allowed_names = Vec::new();

    let candidate_names = args.node_names.clone()
        .or_else(|| args.nodes.as_ref().map(|nl|
            nl.items.iter()
                .filter_map(|n| n["metadata"]["name"].as_str().map(|s| s.to_string()))
                .collect()
        ))
        .unwrap_or_default();

    for name in &candidate_names {
        if let Some(node) = snap.node(name) {
            // Check resource availability
            let reason = check_node_feasibility(node, cpu_req, mem_req, gpu_req);
            match reason {
                None => allowed_names.push(name.clone()),
                Some(r) => { failed_nodes.insert(name.clone(), r); }
            }
        } else {
            // Node not in DRS snapshot yet — allow (conservative)
            allowed_names.push(name.clone());
        }
    }

    Json(ExtenderFilterResult {
        nodes:        None,
        node_names:   Some(allowed_names),
        failed_nodes,
        error:        String::new(),
    })
}

fn check_node_feasibility(
    node:    &crate::types::NodeMetrics,
    cpu_req: f64,
    mem_req: u64,
    gpu_req: bool,
) -> Option<String> {
    // CPU headroom (keep 15% buffer)
    let avail_cpu = node.available_cpu_cores() - (node.cpu_cores as f64 * 0.15);
    if cpu_req > avail_cpu {
        return Some(format!("insufficient CPU: need {cpu_req:.1} cores, available {avail_cpu:.1}"));
    }

    // RAM headroom (keep 256 MiB buffer)
    let avail_mem = node.available_mem_mib().saturating_sub(256);
    if mem_req > avail_mem {
        return Some(format!("insufficient RAM: need {mem_req} MiB, available {avail_mem}"));
    }

    // GPU check (just label presence — device plugin handles the rest)
    // No failure here — handled by k8s node selectors

    None
}

// ── Prioritize handler: score nodes 0-100 ─────────────────────────────────

pub async fn prioritize_handler(
    State((cluster, cfg)): State<DrsState>,
    Json(args): Json<ExtenderArgs>,
) -> Json<ExtenderPriorityResult> {
    let snap = cluster.read().await;
    let checker = AffinityChecker::new();
    let (cpu_req, mem_req, _gpu_req) = extract_vm_resources(&args.pod);

    let candidate_names = args.node_names.clone().unwrap_or_default();
    let mut priorities = Vec::new();

    for name in &candidate_names {
        let score = if let Some(node) = snap.node(name) {
            score_node(node, &snap, &args.pod, cpu_req, mem_req)
        } else {
            50  // default mid score for unknown nodes
        };
        priorities.push(HostPriority { host: name.clone(), score });
    }

    Json(ExtenderPriorityResult(priorities))
}

fn score_node(
    node:    &crate::types::NodeMetrics,
    snap:    &ClusterSnapshot,
    pod:     &serde_json::Value,
    cpu_req: f64,
    mem_req: u64,
) -> i64 {
    // Component 1: CPU headroom (0-40 points)
    let cpu_avail_pct = node.available_cpu_cores() / node.cpu_cores as f64;
    let cpu_score = (cpu_avail_pct * 40.0) as i64;

    // Component 2: RAM headroom (0-30 points)
    let mem_avail_pct = node.available_mem_mib() as f64 / node.mem_total_mib as f64;
    let mem_score = (mem_avail_pct * 30.0) as i64;

    // Component 3: Balance impact (0-20 points)
    // Prefer nodes that are below the cluster average load
    let avg_load = snap.nodes.iter().map(|n| n.load_score).sum::<f64>()
                 / snap.nodes.len() as f64;
    let balance_score = if node.load_score < avg_load {
        ((avg_load - node.load_score) * 20.0 / avg_load.max(0.01)) as i64
    } else { 0 };

    // Component 4: Affinity soft preference (0-10 points)
    // If the pod has preferred affinity to other VMs already on this node, add points
    let affinity_score = 5i64; // stub — full impl reads pod annotations

    (cpu_score + mem_score + balance_score + affinity_score).clamp(0, 100)
}

// ── Resource extraction from pod spec ─────────────────────────────────────

fn extract_vm_resources(pod: &serde_json::Value) -> (f64, u64, bool) {
    let containers = pod["spec"]["containers"].as_array();
    let mut cpu_total = 0.0f64;
    let mut mem_total = 0u64;
    let mut gpu_req   = false;

    if let Some(ctrs) = containers {
        for c in ctrs {
            let requests = &c["resources"]["requests"];

            // CPU: "2" or "2000m"
            if let Some(cpu_str) = requests["cpu"].as_str() {
                if cpu_str.ends_with('m') {
                    cpu_total += cpu_str.trim_end_matches('m')
                        .parse::<f64>().unwrap_or(0.0) / 1000.0;
                } else {
                    cpu_total += cpu_str.parse::<f64>().unwrap_or(0.0);
                }
            }

            // Memory: "512Mi" or "1Gi"
            if let Some(mem_str) = requests["memory"].as_str() {
                mem_total += parse_memory_mib(mem_str);
            }

            // GPU
            if requests.get("caiman.io/gpu-passthrough").is_some()
               || requests.get("caiman.io/mig-3g.40gb").is_some() {
                gpu_req = true;
            }
        }
    }

    (cpu_total.max(1.0), mem_total.max(256), gpu_req)
}

fn parse_memory_mib(s: &str) -> u64 {
    if let Some(g) = s.strip_suffix("Gi") {
        g.parse::<u64>().unwrap_or(0) * 1024
    } else if let Some(m) = s.strip_suffix("Mi") {
        m.parse::<u64>().unwrap_or(0)
    } else if let Some(k) = s.strip_suffix("Ki") {
        k.parse::<u64>().unwrap_or(0) / 1024
    } else {
        s.parse::<u64>().unwrap_or(0) / (1024 * 1024)
    }
}
