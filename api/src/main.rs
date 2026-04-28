//! caiman-api v0.5.0 — REST API completa
//!
//! VM lifecycle endpoints:
//!   POST   /api/vms                → crear + arrancar VM (spawna caiman-vmm)
//!   GET    /api/vms                → listar VMs desde /var/run/caiman/*.json
//!   GET    /api/vms/:id            → detalle de una VM
//!   POST   /api/vms/:id/start      → arrancar VM parada
//!   POST   /api/vms/:id/stop       → parar VM (SIGTERM)
//!   POST   /api/vms/:id/force-stop → matar VM (SIGKILL)
//!   DELETE /api/vms/:id            → eliminar VM y estado
//!   GET    /api/vms/:id/logs       → últimas líneas del log serial
//!
//! Cluster / infra:
//!   GET    /api/cluster            → overview (nodos + VMs + métricas)
//!   GET    /api/nodes              → métricas reales del nodo (/proc)
//!   GET    /api/drs/recommendations→ recomendaciones DRS (stub)
//!   GET    /health

use std::net::SocketAddr;
use axum::{
    Router,
    routing::{get, post, delete},
    extract::{Path, Json},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tracing::info;

mod vm;
mod node;

use vm::state::{VmState, VmStatus};
use vm::runner::{CreateVmRequest, spawn_vm, stop_vm, kill_vm, delete_vm};
use node::metrics::NodeMetrics;

// ── Error helper ──────────────────────────────────────────────────────────

fn err(code: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (code, Json(json!({ "error": msg.to_string() })))
}

fn ok(v: impl serde::Serialize) -> Json<Value> {
    Json(serde_json::to_value(v).unwrap_or(json!({})))
}

// ── VM handlers ───────────────────────────────────────────────────────────

async fn create_vm(Json(req): Json<CreateVmRequest>)
    -> impl IntoResponse
{
    let hostname = sysinfo::System::host_name().unwrap_or_else(|| "node".into());
    match spawn_vm(req, &hostname).await {
        Ok(state) => (StatusCode::CREATED, ok(state)).into_response(),
        Err(e)    => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn list_vms() -> Json<Value> {
    let mut vms = VmState::list_all();
    // Reconcile: mark as STOPPED any VM whose process died
    for vm in &mut vms { vm.reconcile(); }
    Json(json!(vms))
}

async fn get_vm(Path(id): Path<String>) -> impl IntoResponse {
    match VmState::load(&id) {
        Ok(mut state) => {
            state.reconcile();
            ok(state).into_response()
        }
        Err(_) => err(StatusCode::NOT_FOUND, format!("VM {id} not found")).into_response(),
    }
}

async fn start_vm(Path(id): Path<String>) -> impl IntoResponse {
    let Ok(state) = VmState::load(&id) else {
        return err(StatusCode::NOT_FOUND, format!("VM {id} not found")).into_response();
    };
    if state.status == VmStatus::Running {
        return err(StatusCode::CONFLICT, "VM is already running").into_response();
    }
    let req = CreateVmRequest {
        name:    state.name.clone(),
        cpus:    Some(state.cpus),
        mem_mib: Some(state.mem_mib),
        kernel:  Some(state.kernel.clone()),
        disk:    state.disk.clone(),
        uplink:  Some(state.uplink.clone()),
        cmdline: None,
        labels:  Some(state.labels.clone()),
    };
    let hostname = sysinfo::System::host_name().unwrap_or_else(|| "node".into());
    match spawn_vm(req, &hostname).await {
        Ok(new_state) => ok(new_state).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn stop_vm_handler(Path(id): Path<String>) -> impl IntoResponse {
    let Ok(mut state) = VmState::load(&id) else {
        return err(StatusCode::NOT_FOUND, format!("VM {id} not found")).into_response();
    };
    match stop_vm(&mut state) {
        Ok(_)  => ok(state).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn force_stop_vm(Path(id): Path<String>) -> impl IntoResponse {
    let Ok(mut state) = VmState::load(&id) else {
        return err(StatusCode::NOT_FOUND, format!("VM {id} not found")).into_response();
    };
    match kill_vm(&mut state) {
        Ok(_)  => ok(state).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn delete_vm_handler(Path(id): Path<String>) -> impl IntoResponse {
    // Stop first if running
    if let Ok(mut state) = VmState::load(&id) {
        if state.status == VmStatus::Running {
            let _ = kill_vm(&mut state);
        }
    }
    match delete_vm(&id) {
        Ok(_)  => (StatusCode::NO_CONTENT, "").into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn vm_logs(Path(id): Path<String>) -> impl IntoResponse {
    let log_path = format!("/var/run/caiman/{id}.log");
    match std::fs::read_to_string(&log_path) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().rev().take(200).collect();
            ok(lines).into_response()
        }
        Err(_) => ok(Vec::<String>::new()).into_response(),
    }
}

// ── Cluster / node handlers ───────────────────────────────────────────────

async fn get_nodes() -> Json<Value> {
    let vms = VmState::list_all();
    let metrics = NodeMetrics::collect(vms.len());
    Json(json!([metrics]))
}

async fn get_cluster() -> Json<Value> {
    let mut vms = VmState::list_all();
    for vm in &mut vms { vm.reconcile(); }
    let metrics = NodeMetrics::collect(vms.len());
    let total_cpu = metrics.cpu_usage_pct;
    let mem_pct = if metrics.mem_total_mib > 0 {
        metrics.mem_used_mib as f64 / metrics.mem_total_mib as f64
    } else { 0.0 };
    let sigma = (total_cpu / 100.0 - 0.5).abs() * 0.2;

    Json(json!({
        "nodes": [metrics],
        "vms":   vms,
        "balanceSigma":       sigma,
        "drsMode":            "FullyAutomated",
        "totalCpuPct":        total_cpu,
        "totalMemPct":        mem_pct * 100.0,
        "xdpThroughputGbps":  0.0,
        "xdpDropsTotal":      0
    }))
}

async fn drs_recommendations() -> Json<Value> {
    // Real DRS logic lives in caiman-drs; API just proxies it
    // For now return empty list — DRS service fills this
    Json(json!({ "recommendations": [], "balanceSigma": 0.0 }))
}

async fn health() -> Json<Value> {
    Json(json!({
        "status":  "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "service": "caiman-api"
    }))
}

// ── Main ──────────────────────────────────────────────────────────────────

async fn migrate_vm(
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let dest = body.get("destination")
        .and_then(|v| v.as_str())
        .unwrap_or("localhost")
        .to_string();

    // Spawn caiman-livemig as subprocess
    let status = tokio::process::Command::new("caiman-livemig")
        .args(["--vm-id", &id, "--destination", &dest])
        .status()
        .await;

    match status {
        Ok(s) if s.success() =>
            ok(serde_json::json!({"status": "migrated", "destination": dest})).into_response(),
        Ok(s) =>
            err(StatusCode::INTERNAL_SERVER_ERROR,
                format!("migration failed with exit code {:?}", s.code())).into_response(),
        Err(e) =>
            err(StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to spawn caiman-livemig: {e}")).into_response(),
    }
}


#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Ensure state directory exists
    let _ = std::fs::create_dir_all("/var/run/caiman");

    let app = Router::new()
        // Health
        .route("/health", get(health))
        // VM lifecycle
        .route("/api/vms",                    get(list_vms).post(create_vm))
        .route("/api/vms/:id",                get(get_vm).delete(delete_vm_handler))
        .route("/api/vms/:id/start",          post(start_vm))
        .route("/api/vms/:id/stop",           post(stop_vm_handler))
        .route("/api/vms/:id/force-stop",     post(force_stop_vm))
        .route("/api/vms/:id/migrate",        post(migrate_vm))
        .route("/api/vms/:id/console",        get(vm_logs))
        // Cluster
        .route("/api/cluster",               get(get_cluster))
        .route("/api/nodes",                 get(get_nodes))
        .route("/api/drs/recommendations",   get(drs_recommendations))
        .layer(CorsLayer::permissive());

    let addr: SocketAddr = "0.0.0.0:8765".parse().unwrap();
    info!("caiman-api v0.5.0 listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
