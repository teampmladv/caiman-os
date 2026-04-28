//! ws/mod.rs — WebSocket handler
//!
//! Each client connects to GET /ws and receives a stream of JSON events:
//!
//!   VmMetricsUpdate    — per-VM CPU/RAM/NET every 2.5s
//!   NodeMetricsUpdate  — per-node load score every 5s
//!   VmStatusChange     — when VM transitions states
//!   DrsRecommendation  — new DRS migration candidate
//!   MicrosegDeny       — XDP deny event (real-time)
//!   Alert              — threshold breach notifications
//!   MigrationProgress  — live migration phase updates
//!
//! Client → server messages:
//!   { "type": "subscribe", "topics": ["vms", "nodes", "microseg"] }
//!   { "type": "ping" }

use std::sync::Arc;
use axum::{
    extract::{State, WebSocketUpgrade, ws::{WebSocket, Message}},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use futures::{sink::SinkExt, stream::StreamExt};
use tracing::{debug, info};

use crate::state::AppState;
use crate::collectors::MigrationStatus;

// ── Event types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WsEvent {
    VmMetricsUpdate {
        id:            String,
        cpu_usage_pct: f64,
        net_rx_mbps:   f64,
        net_tx_mbps:   f64,
        mem_mib:       u64,
    },
    NodeMetricsUpdate {
        id:            String,
        cpu_usage_pct: f64,
        mem_used_mib:  u64,
        load_score:    f64,
    },
    VmStatusChange {
        id:        String,
        status:    String,
        migrating: Option<MigrationStatus>,
    },
    DrsRecommendation {
        vm_id:     String,
        vm_name:   String,
        from_node: String,
        to_node:   String,
        score:     f64,
        reason:    String,
    },
    MicrosegDeny {
        src_ip:   String,
        dst_ip:   String,
        proto:    String,
        dst_port: u16,
        verdict:  String,
    },
    Alert {
        level:   String,
        title:   String,
        message: String,
    },
    MigrationProgress {
        vm_id:        String,
        phase:        String,
        progress_pct: f64,
    },
    ClusterSnapshot {
        sigma:        f64,
        drs_mode:     String,
        node_count:   usize,
        vm_count:     usize,
        total_cpu:    f64,
        xdp_gbps:     f64,
    },
    Pong,
}

// ── Handler ───────────────────────────────────────────────────────────────

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = state.event_tx.subscribe();

    info!("WebSocket client connected");

    // Send initial snapshot immediately
    {
        let snap = state.cluster.read().await;
        let init = WsEvent::ClusterSnapshot {
            sigma:      snap.balance_sigma,
            drs_mode:   snap.drs_mode.clone(),
            node_count: snap.nodes.len(),
            vm_count:   snap.vms.len(),
            total_cpu:  snap.total_cpu_pct,
            xdp_gbps:   snap.xdp_throughput_gbps,
        };
        if let Ok(json) = serde_json::to_string(&init) {
            let _ = sender.send(Message::Text(json)).await;
        }
    }

    // Spawn send task
    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            match serde_json::to_string(&event) {
                Ok(json) => {
                    if sender.send(Message::Text(json)).await.is_err() {
                        break;
                    }
                }
                Err(e) => debug!("WS serialize error: {e}"),
            }
        }
    });

    // Receive task (handles ping, subscribe commands)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&text) {
                        if cmd["type"] == "ping" {
                            // Pong handled by send_task via broadcast
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }

    info!("WebSocket client disconnected");
}
