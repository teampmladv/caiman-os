//! caiman-mcp — Model Context Protocol server
//! Exposes cluster state and actions via the MCP protocol for AI assistants.
use axum::{Router, routing::get, Json};
use serde_json::{json, Value};

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "caiman-mcp" }))
}

async fn tools() -> Json<Value> {
    Json(json!({
        "tools": [
            { "name": "list_vms",    "description": "List all VMs in the cluster" },
            { "name": "vm_status",   "description": "Get status of a specific VM" },
            { "name": "cluster_status", "description": "Get overall cluster health" },
            { "name": "drs_recommendations", "description": "Get DRS migration recommendations" }
        ]
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let app = Router::new()
        .route("/health", get(health))
        .route("/tools",  get(tools));
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8767").await.unwrap();
    tracing::info!("caiman-mcp listening on :8767");
    axum::serve(listener, app).await.unwrap();
}
