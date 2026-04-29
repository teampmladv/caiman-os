//! drs/src/main.rs — caiman Distributed Resource Scheduler
//!
//! Equivalent to VMware DRS. Three operating modes:
//!
//!   Manual          — computes recommendations, never executes migrations
//!   SemiAutomated   — auto-places new VMs, suggests (not executes) migrations
//!   FullyAutomated  — auto-places + auto-migrates to maintain balance
//!
//! Architecture:
//!
//!   ┌─────────────────────────────────────────────────────┐
//!   │  NodeMonitor (every 30s)                             │
//!   │  Collects: CPU%, RAM%, storage IOPS, XDP stats       │
//!   └────────────┬────────────────────────────────────────┘
//!                │  ClusterSnapshot
//!                ▼
//!   ┌────────────────────────────────────────────────────┐
//!   │  ImbalanceDetector                                  │
//!   │  Computes standard deviation of normalized load     │
//!   │  Triggers rebalance if σ > threshold (default 0.1) │
//!   └────────────┬───────────────────────────────────────┘
//!                │  ImbalanceEvent
//!                ▼
//!   ┌────────────────────────────────────────────────────┐
//!   │  MigrationPlanner                                   │
//!   │  Scores candidate migrations:                       │
//!   │    score = Δload_improvement / migration_cost       │
//!   │  Respects affinity rules + resource pools           │
//!   └────────────┬───────────────────────────────────────┘
//!                │  MigrationPlan
//!                ▼
//!   ┌────────────────────────────────────────────────────┐
//!   │  MigrationExecutor (FullyAutomated only)            │
//!   │  Calls livemig binary for each approved migration   │
//!   │  Rate-limits: max 2 concurrent migrations per node  │
//!   └────────────────────────────────────────────────────┘
//!
//!   Also runs an HTTP server on :8765 for:
//!     POST /filter    — Kubernetes scheduler extender (placement)
//!     POST /prioritize — Kubernetes scheduler extender (scoring)
//!     GET  /metrics   — Prometheus metrics
//!     GET  /status    — cluster balance status + recommendations

use anyhow::Result;
use axum::{Router, routing::{get, post}};
use tracing::info;

mod affinity;
mod balancer;
mod monitor;
mod pool;
mod scheduler;
mod types;

use types::DrsConfig;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("caiman_drs=info".parse()?),
        )
        .with_writer(std::io::stderr)
        .init();

    let cfg = DrsConfig::load()?;
    info!("caiman DRS starting (mode={:?})", cfg.mode);

    // Shared cluster state (updated by monitor, read by balancer + scheduler)
    let cluster = std::sync::Arc::new(
        tokio::sync::RwLock::new(monitor::ClusterSnapshot::default())
    );

    // Spawn background tasks
    let mon_cluster = cluster.clone();
    let mon_cfg     = cfg.clone();
    tokio::spawn(async move {
        monitor::run(mon_cluster, mon_cfg).await
    });

    let bal_cluster = cluster.clone();
    let bal_cfg     = cfg.clone();
    tokio::spawn(async move {
        balancer::run(bal_cluster, bal_cfg).await
    });

    // HTTP server: scheduler extender + metrics + status API
    let app_cluster = cluster.clone();
    let app_cfg     = cfg.clone();
    let app = Router::new()
        .route("/filter",        post(scheduler::filter_handler))
        .route("/prioritize",    post(scheduler::prioritize_handler))
        .route("/metrics",       get(metrics_handler))
        .route("/status",        get(status_handler))
        .route("/recommendations", get(recommendations_handler))
        .with_state((app_cluster, app_cfg));

    let addr = "0.0.0.0:8766";
    info!("DRS HTTP server on {addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn metrics_handler() -> String {
    use prometheus::{Encoder, TextEncoder};
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap_or_default();
    String::from_utf8(buffer).unwrap_or_default()
}

async fn status_handler(
    axum::extract::State((cluster, _cfg)): axum::extract::State<(
        std::sync::Arc<tokio::sync::RwLock<monitor::ClusterSnapshot>>,
        DrsConfig,
    )>,
) -> axum::Json<serde_json::Value> {
    let snap = cluster.read().await;
    axum::Json(serde_json::json!({
        "nodes":         snap.nodes.len(),
        "vms":           snap.total_vms(),
        "balance_score": snap.balance_score(),
        "imbalanced":    snap.is_imbalanced(0.1),
        "timestamp":     snap.timestamp,
    }))
}

async fn recommendations_handler(
    axum::extract::State((cluster, cfg)): axum::extract::State<(
        std::sync::Arc<tokio::sync::RwLock<monitor::ClusterSnapshot>>,
        DrsConfig,
    )>,
) -> axum::Json<serde_json::Value> {
    let snap = cluster.read().await;
    let plan = balancer::compute_plan(&snap, &cfg);
    axum::Json(serde_json::json!({
        "recommendations": plan.migrations.iter().map(|m| serde_json::json!({
            "vm_id":   m.vm_id,
            "from":    m.from_node,
            "to":      m.to_node,
            "reason":  m.reason,
            "score":   m.score,
        })).collect::<Vec<_>>(),
        "mode": format!("{:?}", cfg.mode),
    }))
}
