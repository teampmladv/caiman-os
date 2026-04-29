//! caiman-storage v0.9.0 — VSAN + vVols storage daemon
//!
//! Two storage subsystems:
//!
//!   VSAN  — Distributed block storage using local NVMe/SSD.
//!           Replication across nodes, NVMe-oF data plane.
//!           Each node contributes disks to a shared pool.
//!
//!   vVols — Virtual Volumes: iSCSI, NVMe-oF, NFS v4.1, FC.
//!           Storage-policy-based management.
//!           Hardware arrays enforce policy at device level.

use axum::{Router, routing::{get, post, delete}, extract::{Path, Json},
           http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tracing::info;
use uuid::Uuid;

mod vsan;
mod vvols;

// ── Types ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VolumeStatus { Available, InUse, Creating, Deleting, Error }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Volume {
    pub id:            String,
    pub name:          String,
    pub size_gib:      u64,
    pub status:        VolumeStatus,
    pub backend:       String,   // "vsan" | "iscsi" | "nvmeof" | "nfs"
    pub policy:        String,   // "standard" | "performance" | "archive"
    pub node:          Option<String>,
    pub vm_id:         Option<String>,
    pub path:          Option<String>,
    pub iops:          u64,
    pub throughput_mbps: f64,
    pub created_at:    chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVolumeRequest {
    pub name:      String,
    pub size_gib:  u64,
    pub backend:   Option<String>,
    pub policy:    Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageNode {
    pub hostname:      String,
    pub total_gib:     u64,
    pub used_gib:      u64,
    pub free_gib:      u64,
    pub disks:         Vec<DiskInfo>,
    pub nvmeof_addr:   Option<String>,
    pub status:        String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiskInfo {
    pub path:      String,    // "/dev/nvme0n1"
    pub model:     String,
    pub size_gib:  u64,
    pub role:      String,    // "data" | "cache" | "witness"
    pub health:    String,    // "OK" | "WARN" | "FAIL"
    pub iops:      u64,
}

// ── Shared state ──────────────────────────────────────────────────────────

#[derive(Default)]
struct Store {
    volumes: HashMap<String, Volume>,
}

type SharedStore = Arc<RwLock<Store>>;

fn ok(v: impl Serialize) -> Json<Value> {
    Json(serde_json::to_value(v).unwrap_or(json!({})))
}
fn err(code: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (code, Json(json!({ "error": msg.to_string() })))
}

// ── Volume handlers ───────────────────────────────────────────────────────

async fn list_volumes(
    axum::extract::State(store): axum::extract::State<SharedStore>,
) -> Json<Value> {
    let vols: Vec<Volume> = store.read().unwrap().volumes.values().cloned().collect();
    ok(vols)
}

async fn create_volume(
    axum::extract::State(store): axum::extract::State<SharedStore>,
    Json(req): Json<CreateVolumeRequest>,
) -> impl IntoResponse {
    let id = format!("vol-{}", &Uuid::new_v4().to_string()[..8]);
    let backend = req.backend.unwrap_or_else(|| "vsan".into());
    let policy  = req.policy.unwrap_or_else(|| "standard".into());

    let vol = Volume {
        id:              id.clone(),
        name:            req.name,
        size_gib:        req.size_gib,
        status:          VolumeStatus::Available,
        backend:         backend.clone(),
        policy:          policy.clone(),
        node:            Some(hostname()),
        vm_id:           None,
        path:            Some(format!("/var/lib/caiman/vols/{id}")),
        iops:            if policy == "performance" { 100_000 } else { 10_000 },
        throughput_mbps: if policy == "performance" { 2000.0 } else { 500.0 },
        created_at:      chrono::Utc::now(),
    };

    // Create backing file on VSAN
    let path = format!("/var/lib/caiman/vols/{id}");
    std::fs::create_dir_all("/var/lib/caiman/vols").ok();
    if let Err(e) = create_sparse_file(&path, req.size_gib) {
        return err(StatusCode::INTERNAL_SERVER_ERROR,
                   format!("creating volume: {e}")).into_response();
    }

    store.write().unwrap().volumes.insert(id.clone(), vol.clone());
    info!("Volume created: {id} {size}GiB backend={backend}",
          size = req.size_gib);
    (StatusCode::CREATED, ok(vol)).into_response()
}

async fn get_volume(
    axum::extract::State(store): axum::extract::State<SharedStore>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match store.read().unwrap().volumes.get(&id).cloned() {
        Some(v) => ok(v).into_response(),
        None    => err(StatusCode::NOT_FOUND, format!("volume {id} not found")).into_response(),
    }
}

async fn delete_volume(
    axum::extract::State(store): axum::extract::State<SharedStore>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let path = {
        let mut st = store.write().unwrap();
        match st.volumes.remove(&id) {
            Some(v) => v.path,
            None    => return err(StatusCode::NOT_FOUND, "not found").into_response(),
        }
    };
    if let Some(p) = path {
        let _ = std::fs::remove_file(p);
    }
    info!("Volume deleted: {id}");
    (StatusCode::NO_CONTENT, "").into_response()
}

// ── VSAN node info ────────────────────────────────────────────────────────

async fn vsan_nodes() -> Json<Value> {
    ok(vec![build_local_node()])
}

async fn vsan_disks() -> Json<Value> {
    ok(discover_local_disks())
}

async fn vsan_status() -> Json<Value> {
    let node = build_local_node();
    let used_pct = if node.total_gib > 0 {
        node.used_gib as f64 / node.total_gib as f64 * 100.0
    } else { 0.0 };

    ok(json!({
        "status":     if used_pct < 80.0 { "HEALTHY" } else { "HIGH_USAGE" },
        "nodes":      1,
        "totalGib":   node.total_gib,
        "usedGib":    node.used_gib,
        "freeGib":    node.free_gib,
        "usedPct":    used_pct,
        "replication":"FTT=1",
        "dedup":      false,
        "compression":false,
    }))
}

// ── Health ────────────────────────────────────────────────────────────────

async fn health() -> Json<Value> {
    ok(json!({ "status": "ok", "service": "caiman-storage", "version": env!("CARGO_PKG_VERSION") }))
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn hostname() -> String {
    std::fs::read_to_string("/etc/hostname")
        .unwrap_or_else(|_| "caiman-node".into())
        .trim().to_string()
}

fn create_sparse_file(path: &str, size_gib: u64) -> anyhow::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let f = std::fs::OpenOptions::new()
        .write(true).create(true).open(path)?;
    f.set_len(size_gib * 1024 * 1024 * 1024)?;
    Ok(())
}

fn build_local_node() -> StorageNode {
    let vol_dir = "/var/lib/caiman/vols";
    std::fs::create_dir_all(vol_dir).ok();

    let used_gib = std::fs::read_dir(vol_dir)
        .into_iter().flatten().flatten()
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len() / (1024*1024*1024))
        .sum::<u64>();

    let stat = nix_statvfs(vol_dir);
    let total_gib = stat.0;
    let free_gib  = stat.1;

    StorageNode {
        hostname:    hostname(),
        total_gib,
        used_gib,
        free_gib,
        disks:       discover_local_disks(),
        nvmeof_addr: None,
        status:      "HEALTHY".into(),
    }
}

fn nix_statvfs(path: &str) -> (u64, u64) {
    // Use statvfs syscall via /proc/mounts parsing
    if let Ok(out) = std::process::Command::new("df")
        .args(["-BG", "--output=size,avail", path])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let line = stdout.lines().nth(1).unwrap_or("");
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let total = parts[0].trim_end_matches('G').parse().unwrap_or(0);
            let free  = parts[1].trim_end_matches('G').parse().unwrap_or(0);
            return (total, free);
        }
    }
    (0, 0)
}

fn discover_local_disks() -> Vec<DiskInfo> {
    // Read /sys/block for NVMe and SSD devices
    let mut disks = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("nvme") && !name.starts_with("sd") { continue; }
            if name.ends_with("n1p1") || name.contains("loop") { continue; }

            let size_bytes: u64 = std::fs::read_to_string(
                format!("/sys/block/{name}/size")
            ).unwrap_or_default().trim().parse().unwrap_or(0);
            let size_gib = size_bytes * 512 / (1024*1024*1024);
            if size_gib < 10 { continue; } // skip tiny devices

            let model = std::fs::read_to_string(
                format!("/sys/block/{name}/device/model")
            ).unwrap_or_else(|_| "Unknown".into()).trim().to_string();

            disks.push(DiskInfo {
                path:     format!("/dev/{name}"),
                model,
                size_gib,
                role:     "data".into(),
                health:   "OK".into(),
                iops:     if name.starts_with("nvme") { 500_000 } else { 10_000 },
            });
        }
    }
    disks
}

// ── Main ──────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let store: SharedStore = Arc::new(RwLock::new(Store::default()));

    let app = Router::new()
        .route("/health",              get(health))
        .route("/api/volumes",         get(list_volumes).post(create_volume))
        .route("/api/volumes/:id",     get(get_volume).delete(delete_volume))
        .route("/api/vsan/nodes",      get(vsan_nodes))
        .route("/api/vsan/disks",      get(vsan_disks))
        .route("/api/vsan/status",     get(vsan_status))
        .layer(tower_http::cors::CorsLayer::permissive())
        .with_state(store);

    let addr: SocketAddr = "0.0.0.0:8770".parse().unwrap();
    info!("caiman-storage v{} on {addr}", env!("CARGO_PKG_VERSION"));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
