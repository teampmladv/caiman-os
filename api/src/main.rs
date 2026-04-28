//! caiman-api — REST API + WebSocket + Prometheus
use axum::{Router, routing::get, Json};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tracing::info;

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

async fn cluster() -> Json<Value> {
    Json(json!({
        "nodes": [],
        "vms": [],
        "balanceSigma": 0.0,
        "drsMode": "FullyAutomated",
        "totalCpuPct": 0.0,
        "xdpThroughputGbps": 0.0,
        "xdpDropsTotal": 0
    }))
}

async fn nodes() -> Json<Value>  { Json(json!([])) }
async fn vms()   -> Json<Value>  { Json(json!([])) }
async fn drs()   -> Json<Value>  { Json(json!({ "recommendations": [] })) }

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let app = Router::new()
        .route("/health",      get(health))
        .route("/api/cluster", get(cluster))
        .route("/api/nodes",   get(nodes))
        .route("/api/vms",     get(vms))
        .route("/api/drs/recommendations", get(drs));

    let addr: SocketAddr = "0.0.0.0:8765".parse().unwrap();
    info!("caiman-api listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
