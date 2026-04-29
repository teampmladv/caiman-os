//! caiman-bts v0.1.0 — Backup, Templates & Snapshots server
use axum::{Router, routing::get, Json};
use serde_json::{json, Value};
use std::net::SocketAddr;
use tracing::info;

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "caiman-bts", "version": env!("CARGO_PKG_VERSION") }))
}

async fn list_snapshots() -> Json<Value> {
    Json(json!([]))
}

async fn list_backups() -> Json<Value> {
    Json(json!([]))
}

async fn list_templates() -> Json<Value> {
    Json(json!([]))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let app = Router::new()
        .route("/health",              get(health))
        .route("/api/snapshots",       get(list_snapshots))
        .route("/api/backups",         get(list_backups))
        .route("/api/templates",       get(list_templates));

    let addr: SocketAddr = "0.0.0.0:8768".parse().unwrap();
    info!("caiman-bts v{} listening on {addr}", env!("CARGO_PKG_VERSION"));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
