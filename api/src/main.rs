//! caiman-api v1.1.0 -- REST API (production + Railway demo mode)
//!
//! DEMO_MODE=true  -> in-memory simulation, no KVM required (Railway)
//! DEMO_MODE=false -> real VMs via caiman-vmm (bare metal)
//!
//! Auth: JWT via CAIMAN_JWT_SECRET env var
//!   Public:    GET /health, POST /auth/bootstrap
//!   Protected: POST /auth/token  (admin only), all /api/*

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use axum::{
    Router,
    routing::{get, post, delete},
    extract::{Path, Json, State, WebSocketUpgrade},
    http::StatusCode,
    response::IntoResponse,
    middleware,
};
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tracing::info;

mod auth;
mod import;
mod vm;
mod node;
mod demo;

use demo::state::{DemoStore, SharedDemo};
use vm::state::{VmState, VmStatus};
use vm::runner::{CreateVmRequest, spawn_vm, stop_vm, kill_vm, delete_vm};
use node::metrics::NodeMetrics;

fn is_demo() -> bool {
    std::env::var("DEMO_MODE").map(|v| v == "true" || v == "1").unwrap_or(false)
        || !std::path::Path::new("/dev/kvm").exists()
}

fn err(code: StatusCode, msg: impl ToString) -> (StatusCode, Json<Value>) {
    (code, Json(json!({ "error": msg.to_string() })))
}
fn ok(v: impl serde::Serialize) -> Json<Value> {
    Json(serde_json::to_value(v).unwrap_or(json!({})))
}

// - Demo mode handlers -

async fn demo_create_vm(State(store): State<SharedDemo>, Json(req): Json<CreateVmRequest>)
    -> impl IntoResponse
{
    let vm = store.write().unwrap().create_vm(
        req.name.clone(),
        req.cpus.unwrap_or(1),
        req.mem_mib.unwrap_or(256),
    );
    (StatusCode::CREATED, ok(vm)).into_response()
}

async fn demo_list_vms(State(store): State<SharedDemo>) -> Json<Value> {
    store.write().unwrap().transition_booting();
    let vms = store.read().unwrap().list_vms();
    Json(json!(vms))
}

async fn demo_get_vm(State(store): State<SharedDemo>, Path(id): Path<String>)
    -> impl IntoResponse
{
    store.write().unwrap().transition_booting();
    match store.read().unwrap().get_vm(&id) {
        Some(vm) => ok(vm).into_response(),
        None     => err(StatusCode::NOT_FOUND, format!("VM {id} not found")).into_response(),
    }
}

async fn demo_stop_vm(State(store): State<SharedDemo>, Path(id): Path<String>)
    -> impl IntoResponse
{
    store.write().unwrap().stop_vm(&id);
    match store.read().unwrap().get_vm(&id) {
        Some(vm) => ok(vm).into_response(),
        None     => err(StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn demo_start_vm(State(store): State<SharedDemo>, Path(id): Path<String>)
    -> impl IntoResponse
{
    store.write().unwrap().start_vm(&id);
    match store.read().unwrap().get_vm(&id) {
        Some(vm) => ok(vm).into_response(),
        None     => err(StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn demo_delete_vm(State(store): State<SharedDemo>, Path(id): Path<String>)
    -> impl IntoResponse
{
    store.write().unwrap().delete_vm(&id);
    (StatusCode::NO_CONTENT, "").into_response()
}

async fn demo_nodes(State(store): State<SharedDemo>) -> Json<Value> {
    store.write().unwrap().transition_booting();
    let node = store.read().unwrap().node_metrics();
    Json(json!([node]))
}

async fn demo_cluster(State(store): State<SharedDemo>) -> Json<Value> {
    store.write().unwrap().transition_booting();
    let vms  = store.read().unwrap().list_vms();
    let node = store.read().unwrap().node_metrics();
    let sigma = (node.cpu_usage_pct / 100.0 - 0.5).abs() * 0.2;
    Json(json!({
        "nodes": [node], "vms": vms,
        "balanceSigma": sigma, "drsMode": "FullyAutomated",
        "totalCpuPct": node.cpu_usage_pct, "xdpThroughputGbps": 2.4,
        "xdpDropsTotal": 0
    }))
}

// - Real mode handlers -

async fn create_vm(Json(req): Json<CreateVmRequest>) -> impl IntoResponse {
    let hostname = sysinfo::System::host_name().unwrap_or_else(|| "node".into());
    match spawn_vm(req, &hostname).await {
        Ok(s)  => (StatusCode::CREATED, ok(s)).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn list_vms() -> Json<Value> {
    let mut vms = VmState::list_all();
    for vm in &mut vms { vm.reconcile(); }
    Json(json!(vms))
}

async fn get_vm(Path(id): Path<String>) -> impl IntoResponse {
    match VmState::load(&id) {
        Ok(mut s) => { s.reconcile(); ok(s).into_response() }
        Err(_)    => err(StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn stop_vm_h(Path(id): Path<String>) -> impl IntoResponse {
    match VmState::load(&id) {
        Ok(mut s) => match stop_vm(&mut s) {
            Ok(_)  => ok(s).into_response(),
            Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        },
        Err(_) => err(StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn start_vm_h(Path(id): Path<String>) -> impl IntoResponse {
    let Ok(state) = VmState::load(&id) else {
        return err(StatusCode::NOT_FOUND, "not found").into_response();
    };
    let req = CreateVmRequest {
        name: state.name, cpus: Some(state.cpus),
        mem_mib: Some(state.mem_mib), kernel: Some(state.kernel),
        disk: state.disk, uplink: Some(state.uplink),
        cmdline: None, labels: Some(state.labels),
    };
    let hostname = sysinfo::System::host_name().unwrap_or_else(|| "node".into());
    match spawn_vm(req, &hostname).await {
        Ok(s)  => ok(s).into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn force_stop_h(Path(id): Path<String>) -> impl IntoResponse {
    match VmState::load(&id) {
        Ok(mut s) => match kill_vm(&mut s) {
            Ok(_)  => ok(s).into_response(),
            Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
        },
        Err(_) => err(StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

async fn delete_vm_h(Path(id): Path<String>) -> impl IntoResponse {
    if let Ok(mut s) = VmState::load(&id) {
        if s.status == VmStatus::Running { let _ = kill_vm(&mut s); }
    }
    match delete_vm(&id) {
        Ok(_)  => (StatusCode::NO_CONTENT, "").into_response(),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    }
}

async fn vm_logs(Path(id): Path<String>) -> impl IntoResponse {
    let path = format!("/var/run/caiman/{id}.log");
    match std::fs::read_to_string(&path) {
        Ok(c)  => { let lines: Vec<&str> = c.lines().rev().take(200).collect(); ok(lines).into_response() }
        Err(_) => ok(Vec::<String>::new()).into_response(),
    }
}

async fn get_nodes_real() -> Json<Value> {
    let vms = VmState::list_all();
    Json(json!([NodeMetrics::collect(vms.len())]))
}

async fn get_cluster_real() -> Json<Value> {
    let mut vms = VmState::list_all();
    for vm in &mut vms { vm.reconcile(); }
    let m = NodeMetrics::collect(vms.len());
    let sigma = (m.cpu_usage_pct / 100.0 - 0.5).abs() * 0.2;
    Json(json!({
        "nodes": [m], "vms": vms,
        "balanceSigma": sigma, "drsMode": "FullyAutomated",
        "totalCpuPct": m.cpu_usage_pct, "xdpThroughputGbps": 0.0
    }))
}

async fn migrate_vm(Path(id): Path<String>, Json(body): Json<Value>) -> impl IntoResponse {
    let dest = body.get("destination").and_then(|v| v.as_str()).unwrap_or("localhost").to_string();
    let status = tokio::process::Command::new("caiman-livemig")
        .args(["--vm-id", &id, "--destination", &dest])
        .status().await;
    match status {
        Ok(s) if s.success() => ok(json!({"status":"migrated","destination":dest})).into_response(),
        _ => err(StatusCode::INTERNAL_SERVER_ERROR, "migration failed").into_response(),
    }
}

async fn health() -> Json<Value> {
    let demo = is_demo();
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION"), "demo": demo }))
}

async fn drs() -> Json<Value> {
    Json(json!({ "recommendations": [], "balanceSigma": 0.0 }))
}

// - Main -

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let demo_mode = is_demo();
    info!("caiman-api v{} -- demo_mode={}", env!("CARGO_PKG_VERSION"), demo_mode);

    let cors = CorsLayer::permissive();
    let addr: SocketAddr = "0.0.0.0:8765".parse().unwrap();

    // - Public routes (no token needed) -
    let public = Router::new()
        .route("/health",          get(health))
        .route("/auth/bootstrap",  post(auth::bootstrap_token));

    if demo_mode {
        let store: SharedDemo = Arc::new(RwLock::new(DemoStore::new()));

        let protected = Router::new()
            .route("/auth/token",                     post(auth::generate_token))
            .route("/api/vms",                        get(demo_list_vms).post(demo_create_vm))
            .route("/api/vms/:id",                    get(demo_get_vm).delete(demo_delete_vm))
            .route("/api/vms/:id/start",              post(demo_start_vm))
            .route("/api/vms/:id/stop",               post(demo_stop_vm))
            .route("/api/vms/:id/force-stop",         post(demo_stop_vm))
            .route("/api/nodes",                      get(demo_nodes))
            .route("/api/cluster",                    get(demo_cluster))
            .route("/api/drs/recommendations",        get(drs))
            .route("/api/import/discover",                post(import::discover))
            .route("/api/import/vm",                      post(import::import_vm))
            .layer(middleware::from_fn(auth::require_auth))
            .with_state(store);

        let app = Router::new()
            .merge(public)
            .merge(protected)
            .layer(cors);

        info!("DEMO MODE -- listening on {addr}");
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();

    } else {
        let _ = std::fs::create_dir_all("/var/run/caiman");

        let protected = Router::new()
            .route("/auth/token",                     post(auth::generate_token))
            .route("/api/vms",                        get(list_vms).post(create_vm))
            .route("/api/vms/:id",                    get(get_vm).delete(delete_vm_h))
            .route("/api/vms/:id/start",              post(start_vm_h))
            .route("/api/vms/:id/stop",               post(stop_vm_h))
            .route("/api/vms/:id/force-stop",         post(force_stop_h))
            .route("/api/vms/:id/console",            get(vm_logs))
            .route("/api/vms/:id/console/ws",         get(vm_console_ws))
            .route("/api/vms/:id/migrate",            post(migrate_vm))
            .route("/api/nodes",                      get(get_nodes_real))
            .route("/api/cluster",                    get(get_cluster_real))
            .route("/api/drs/recommendations",        get(drs))
            .route("/api/import/discover",                post(import::discover))
            .route("/api/import/vm",                      post(import::import_vm))
            .layer(middleware::from_fn(auth::require_auth));

        let app = Router::new()
            .merge(public)
            .merge(protected)
            .layer(cors);

        info!("PRODUCTION MODE -- listening on {addr}");
        let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    }
}

// - WebSocket console handler -
async fn vm_console_ws(
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_console(socket, id))
}

async fn handle_console(mut socket: axum::extract::ws::WebSocket, id: String) {
    use axum::extract::ws::Message;
    use tokio::io::AsyncBufReadExt;

    let log_path = format!("/var/run/caiman/{id}.log");

    if let Ok(content) = std::fs::read_to_string(&log_path) {
        for line in content.lines() {
            if socket.send(Message::Text(line.to_string().into())).await.is_err() {
                return;
            }
        }
    }

    let Ok(file) = tokio::fs::File::open(&log_path).await else {
        let _ = socket.send(Message::Text("[console] Log not available".into())).await;
        return;
    };

    let mut reader = tokio::io::BufReader::new(file);
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
                if socket.send(Message::Ping(vec![].into())).await.is_err() {
                    break;
                }
            }
            Ok(_) => {
                let msg = line.trim_end_matches('\n').to_string();
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
}
