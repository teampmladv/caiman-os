//! caiman-api — REST API + WebSocket + Prometheus exporter
//!
//! Ports:
//!   :8765  — REST API + WebSocket (UI → caiman-api)
//!   :9090  — Prometheus metrics scrape endpoint
//!
//! Architecture:
//!
//!   ┌─────────────────────────────────────────────────────┐
//!   │                  caiman-api                          │
//!   │                                                      │
//!   │  CollectorLoop (every 5s)                            │
//!   │    ├─ NodeCollector    /proc + sysinfo               │
//!   │    ├─ VmCollector      /var/run/caiman/*.json        │
//!   │    ├─ XdpCollector     caiman_net netlink            │
//!   │    ├─ MicrosegCollector BPF ring buffer              │
//!   │    └─ KubeCollector    k8s API (nodes, pods)         │
//!   │           │                                          │
//!   │     StateStore (Arc<RwLock<ClusterState>>)           │
//!   │           │                                          │
//!   │    ┌──────┴───────┬──────────────────────┐           │
//!   │  REST API       WebSocket bus          Prometheus    │
//!   │  /api/vms       /ws (JSON events)      /metrics      │
//!   │  /api/nodes     live push every 2s     gauges/ctr    │
//!   │  /api/drs       on-change push         histograms    │
//!   └─────────────────────────────────────────────────────┘

use std::sync::Arc;
use axum::{Router, middleware};
use tokio::sync::broadcast;
use tower_http::{cors::CorsLayer, trace::TraceLayer, compression::CompressionLayer};
use tracing::info;

mod auth;
mod collectors;
mod metrics;
mod middleware as mw;
mod routes;
mod state;
mod ws;

use state::AppState;
use collectors::CollectorLoop;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("caiman_api=info".parse()?)
                .add_directive("tower_http=warn".parse()?)
        )
        .json()
        .init();

    info!("Caimán API starting");

    // Config
    let cfg = config::Config::builder()
        .add_source(config::File::with_name("caiman-api").required(false))
        .add_source(config::Environment::with_prefix("CAIMAN_API").separator("__"))
        .build()?;

    // Shared state
    let (event_tx, _) = broadcast::channel::<ws::WsEvent>(512);
    let state = Arc::new(AppState::new(event_tx.clone()).await?);

    // Start collector loop (polls all data sources every 5s)
    let collector = CollectorLoop::new(state.clone(), event_tx.clone());
    tokio::spawn(collector.run());

    // Start Prometheus metrics server on :9090
    let metrics_state = state.clone();
    tokio::spawn(metrics::serve_metrics(metrics_state, 9090));

    // Build main Axum router
    let app = Router::new()
        // ── Auth ───────────────────────────────────────────────────────
        .nest("/auth",     routes::auth::router())
        // ── REST API ───────────────────────────────────────────────────
        .nest("/api",      routes::api::router(state.clone()))
        // ── WebSocket ──────────────────────────────────────────────────
        .route("/ws",      axum::routing::get(ws::ws_handler))
        // ── Health ─────────────────────────────────────────────────────
        .route("/health",  axum::routing::get(health_handler))
        .route("/ready",   axum::routing::get(ready_handler))
        // ── Middleware ─────────────────────────────────────────────────
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any),
        )
        .with_state(state);

    let addr = "0.0.0.0:8765";
    info!("Listening on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_handler() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({ "status": "ok", "service": "caiman-api" }))
}

async fn ready_handler(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::Json<serde_json::Value> {
    let snap = state.cluster.read().await;
    axum::Json(serde_json::json!({
        "ready":   snap.nodes.len() > 0,
        "nodes":   snap.nodes.len(),
        "vms":     snap.vms.len(),
    }))
}
