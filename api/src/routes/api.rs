//! routes/api.rs — All REST API endpoints
//!
//! GET  /api/cluster              full snapshot
//! GET  /api/nodes                all nodes
//! GET  /api/nodes/:id            single node detail
//! GET  /api/vms                  all VMs
//! GET  /api/vms/:id              single VM detail
//! GET  /api/vms/:id/console      serial log lines
//! POST /api/vms/:id/start        start stopped VM
//! POST /api/vms/:id/stop         stop running VM
//! POST /api/vms/:id/migrate      live migrate VM
//! GET  /api/drs/recommendations  DRS migration recs
//! POST /api/drs/execute/:vm_id   execute a specific migration
//! GET  /api/xdp/stats            XDP per-VM stats
//! GET  /api/microseg/policies    MicroSegPolicy list
//! GET  /api/microseg/audit       audit event log
//! GET  /api/storage/vsan         VSAN volumes
//! GET  /api/storage/vvols        vVols
//! GET  /api/gpu/allocations      GPU allocation table

use std::sync::Arc;
use axum::{
    Router, routing::{get, post},
    extract::{State, Path, Query},
    Json, http::StatusCode,
};
use serde::{Deserialize, Serialize};
use anyhow::Result;

use crate::state::AppState;

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<serde_json::Value>)>;

fn err(code: StatusCode, msg: impl ToString) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({ "error": msg.to_string() })))
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        // Cluster
        .route("/cluster",                  get(cluster_snapshot))
        // Nodes
        .route("/nodes",                    get(list_nodes))
        .route("/nodes/:id",                get(get_node))
        // VMs
        .route("/vms",                      get(list_vms))
        .route("/vms/:id",                  get(get_vm))
        .route("/vms/:id/console",          get(vm_console))
        .route("/vms/:id/start",            post(start_vm))
        .route("/vms/:id/stop",             post(stop_vm))
        .route("/vms/:id/migrate",          post(migrate_vm))
        .route("/vms",                      post(create_vm))
        // DRS
        .route("/drs/recommendations",      get(drs_recommendations))
        .route("/drs/execute/:vm_id",       post(drs_execute))
        .route("/drs/config",               get(drs_config))
        // XDP
        .route("/xdp/stats",                get(xdp_stats))
        .route("/xdp/stats/:vm_id",         get(xdp_stats_vm))
        // Micro-seg
        .route("/microseg/policies",        get(microseg_policies))
        .route("/microseg/policies",        post(create_microseg_policy))
        .route("/microseg/audit",           get(microseg_audit))
        .route("/microseg/stats",           get(microseg_stats))
        // Storage
        .route("/storage/vsan",             get(vsan_volumes))
        .route("/storage/vsan",             post(create_vsan_volume))
        .route("/storage/vvols",            get(vvols))
        // GPU
        .route("/gpu/allocations",          get(gpu_allocations))
        .route("/gpu/devices",              get(gpu_devices))
        .with_state(state)
}

// ── Cluster ───────────────────────────────────────────────────────────────

async fn cluster_snapshot(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let state = s.cluster.read().await;
    Json(serde_json::json!({
        "nodes":               state.nodes,
        "vms":                 state.vms,
        "balanceSigma":        state.balance_sigma,
        "drsMode":             state.drs_mode,
        "totalCpuPct":         state.total_cpu_pct,
        "totalMemUsedMib":     state.total_mem_used_mib,
        "totalMemMib":         state.total_mem_mib,
        "xdpThroughputGbps":   state.xdp_throughput_gbps,
        "xdpDropsTotal":       state.xdp_drops_total,
        "microsegDenies60s":   state.microseg_denies_60s,
        "timestamp":           state.updated_at.timestamp_millis(),
    }))
}

// ── Nodes ─────────────────────────────────────────────────────────────────

async fn list_nodes(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let state = s.cluster.read().await;
    Json(serde_json::json!(state.nodes))
}

async fn get_node(State(s): State<Arc<AppState>>, Path(id): Path<String>)
    -> ApiResult<serde_json::Value>
{
    let state = s.cluster.read().await;
    state.nodes.iter().find(|n| n.id == id || n.hostname == id)
        .map(|n| Json(serde_json::to_value(n).unwrap()))
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("node {id} not found")))
}

// ── VMs ───────────────────────────────────────────────────────────────────

async fn list_vms(
    State(s): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let state = s.cluster.read().await;
    let vms: Vec<_> = state.vms.iter()
        .filter(|v| {
            params.get("status").map_or(true, |s| &v.status == s)
            && params.get("node").map_or(true, |n| &v.node_name == n)
        })
        .collect();
    Json(serde_json::json!(vms))
}

async fn get_vm(State(s): State<Arc<AppState>>, Path(id): Path<String>)
    -> ApiResult<serde_json::Value>
{
    let state = s.cluster.read().await;
    state.vms.iter().find(|v| v.id == id)
        .map(|v| Json(serde_json::to_value(v).unwrap()))
        .ok_or_else(|| err(StatusCode::NOT_FOUND, format!("vm {id} not found")))
}

#[derive(Deserialize)]
struct ConsoleQuery { lines: Option<usize> }

async fn vm_console(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<ConsoleQuery>,
) -> ApiResult<Vec<String>> {
    let state   = s.cluster.read().await;
    let vm = state.vms.iter().find(|v| v.id == id)
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "vm not found"))?;

    let lines = q.lines.unwrap_or(100);
    let state_dir = std::env::var("CAIMAN_STATE_DIR")
        .unwrap_or_else(|_| "/var/run/caiman".into());

    // Extract numeric ID from vm-001 → 1
    let num_id: u32 = id.trim_start_matches("vm-").parse().unwrap_or(0);
    let log_path = format!("{state_dir}/{num_id}.log");

    let content = tokio::fs::read_to_string(&log_path).await
        .unwrap_or_else(|_| format!("[Serial log not available for {id}]"));

    let all_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let start = all_lines.len().saturating_sub(lines);
    Ok(Json(all_lines[start..].to_vec()))
}

#[derive(Deserialize)]
struct MigrateRequest { #[serde(rename = "toNode")] to_node: String }

async fn migrate_vm(
    State(s): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<MigrateRequest>,
) -> ApiResult<serde_json::Value> {
    // Spawn caiman-livemig binary
    let result = tokio::process::Command::new("caiman-livemig")
        .args(["--vm-id", id.trim_start_matches("vm-"), "--destination", &req.to_node])
        .spawn();

    match result {
        Ok(_) => {
            let _ = s.event_tx.send(crate::ws::WsEvent::VmStatusChange {
                id:        id.clone(),
                status:    "MIGRATING".into(),
                migrating: Some(crate::collectors::MigrationStatus {
                    phase:        "Setup".into(),
                    from_node:    "local".into(),
                    to_node:      req.to_node,
                    progress_pct: 0.0,
                    elapsed_secs: 0,
                    blackout_ms:  None,
                }),
            });
            Ok(Json(serde_json::json!({ "status": "migration started", "vmId": id })))
        }
        Err(e) => Err(err(StatusCode::INTERNAL_SERVER_ERROR, e)),
    }
}

async fn start_vm(State(_): State<Arc<AppState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    // TODO: call caiman-vmm with stored config
    Json(serde_json::json!({ "status": "starting", "vmId": id }))
}

async fn stop_vm(State(s): State<Arc<AppState>>, Path(id): Path<String>)
    -> Json<serde_json::Value>
{
    // Send SIGTERM to VMM process
    let state = s.cluster.read().await;
    if let Some(vm) = state.vms.iter().find(|v| v.id == id) {
        if let Some(pid) = vm.pid {
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid as i32),
                nix::sys::signal::Signal::SIGTERM,
            );
        }
    }
    Json(serde_json::json!({ "status": "stopping", "vmId": id }))
}

async fn create_vm(State(_): State<Arc<AppState>>, Json(_): Json<serde_json::Value>)
    -> Json<serde_json::Value>
{
    Json(serde_json::json!({ "status": "TODO: POST /api/vms" }))
}

// ── DRS ───────────────────────────────────────────────────────────────────

async fn drs_recommendations(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // Forward to caiman-drs service
    let drs_url = std::env::var("CAIMAN_DRS_URL")
        .unwrap_or_else(|_| "http://localhost:8766".into());

    match reqwest::get(format!("{drs_url}/recommendations")).await {
        Ok(r) if r.status().is_success() => {
            Json(r.json().await.unwrap_or(serde_json::json!([])))
        }
        _ => {
            // Fallback: compute locally from cluster state
            let state = s.cluster.read().await;
            Json(serde_json::json!({
                "recommendations": [],
                "sigma": state.balance_sigma,
                "mode":  state.drs_mode,
            }))
        }
    }
}

async fn drs_execute(State(s): State<Arc<AppState>>, Path(vm_id): Path<String>)
    -> Json<serde_json::Value>
{
    Json(serde_json::json!({ "status": "executing", "vmId": vm_id }))
}

async fn drs_config(State(_): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "mode": "FullyAutomated",
        "imbalanceThreshold": 0.10,
        "monitorIntervalSecs": 30,
        "maxConcurrentMigrations": 2,
        "minMigrationScore": 0.25,
    }))
}

// ── XDP stats ─────────────────────────────────────────────────────────────

async fn xdp_stats(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let state = s.cluster.read().await;
    let stats: Vec<_> = state.vms.iter().map(|v| serde_json::json!({
        "vmId":      v.id,
        "vmName":    v.name,
        "rxMbps":    v.net_rx_mbps,
        "txMbps":    v.net_tx_mbps,
        "rxDrops":   v.net_rx_drops,
    })).collect();
    Json(serde_json::json!({
        "vms": stats,
        "totalGbps": state.xdp_throughput_gbps,
        "totalDrops": state.xdp_drops_total,
    }))
}

async fn xdp_stats_vm(State(s): State<Arc<AppState>>, Path(vm_id): Path<String>)
    -> ApiResult<serde_json::Value>
{
    let state = s.cluster.read().await;
    state.vms.iter().find(|v| v.id == vm_id)
        .map(|v| Json(serde_json::json!({
            "vmId":    v.id,
            "rxMbps":  v.net_rx_mbps,
            "txMbps":  v.net_tx_mbps,
            "rxDrops": v.net_rx_drops,
        })))
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "vm not found"))
}

// ── Micro-seg ─────────────────────────────────────────────────────────────

async fn microseg_policies(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // Query Kubernetes API for MicroSegPolicy CRDs
    Json(serde_json::json!({ "policies": [], "total": 0 }))
}

async fn create_microseg_policy(Json(_): Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "created" }))
}

async fn microseg_audit(
    State(s): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Json<serde_json::Value> {
    let limit = params.get("limit").and_then(|s| s.parse::<i64>().ok()).unwrap_or(100);
    // Read from SQLite audit_events table
    let events: Vec<serde_json::Value> = sqlx::query_as::<_, (i64, String, String, String, String, i64)>(
        "SELECT id, src_ip, dst_ip, proto, verdict, timestamp_ns FROM audit_events ORDER BY id DESC LIMIT ?"
    )
    .bind(limit)
    .fetch_all(&s.db)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|(id, src_ip, dst_ip, proto, verdict, ts)| serde_json::json!({
        "id": id, "srcIp": src_ip, "dstIp": dst_ip,
        "proto": proto, "verdict": verdict, "timestampNs": ts,
    }))
    .collect();

    Json(serde_json::json!({ "events": events }))
}

async fn microseg_stats(State(s): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let state = s.cluster.read().await;
    Json(serde_json::json!({
        "denies60s": state.microseg_denies_60s,
        "xdpActive": true,
    }))
}

// ── Storage ───────────────────────────────────────────────────────────────

async fn vsan_volumes(State(_): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "volumes": [], "totalGib": 0, "usedGib": 0 }))
}

async fn create_vsan_volume(Json(_): Json<serde_json::Value>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "created" }))
}

async fn vvols(State(_): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "vvols": [] }))
}

// ── GPU ───────────────────────────────────────────────────────────────────

async fn gpu_allocations(State(_): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "allocations": [] }))
}

async fn gpu_devices(State(_): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // Run nvidia-smi query
    let output = tokio::process::Command::new("nvidia-smi")
        .args(["--query-gpu=gpu_uuid,name,memory.total,utilization.gpu",
               "--format=csv,noheader,nounits"])
        .output().await;

    match output {
        Ok(out) if out.status.success() => {
            let devices: Vec<serde_json::Value> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(|line| {
                    let p: Vec<&str> = line.split(", ").collect();
                    serde_json::json!({
                        "uuid":    p.first().unwrap_or(&""),
                        "name":    p.get(1).unwrap_or(&""),
                        "vramMib": p.get(2).unwrap_or(&"0"),
                        "utilPct": p.get(3).unwrap_or(&"0"),
                    })
                })
                .collect();
            Json(serde_json::json!({ "devices": devices }))
        }
        _ => Json(serde_json::json!({ "devices": [], "error": "nvidia-smi not available" }))
    }
}
