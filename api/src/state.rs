//! state.rs — shared AppState passed through Axum extractors

use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use moka::future::Cache;
use anyhow::Result;

use crate::collectors::ClusterState;
use crate::ws::WsEvent;

pub struct AppState {
    /// Live cluster state, updated by CollectorLoop every 5s
    pub cluster: Arc<RwLock<ClusterState>>,
    /// Broadcast channel for WebSocket push
    pub event_tx: broadcast::Sender<WsEvent>,
    /// Short-lived response cache (reduces /proc reads)
    pub cache: Cache<String, serde_json::Value>,
    /// SQLite connection pool (audit log, DRS history)
    pub db: sqlx::SqlitePool,
    /// JWT signing secret (from env CAIMAN_JWT_SECRET or /root/caiman-jwt-secret.txt)
    pub jwt_secret: String,
    /// LDAP/AD URL (optional, from env CAIMAN_LDAP_URL)
    pub ldap_url: Option<String>,
    /// LDAP base DN (optional, from env CAIMAN_LDAP_BASE)
    pub ldap_base: Option<String>,
}

impl AppState {
    pub async fn new(event_tx: broadcast::Sender<WsEvent>) -> Result<Self> {
        let db = sqlx::SqlitePool::connect(
            &std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:///var/lib/caiman/caiman.db".into())
        ).await
        .unwrap_or_else(|_| {
            // In-memory fallback for dev
            futures::executor::block_on(
                sqlx::SqlitePool::connect("sqlite::memory:")
            ).unwrap()
        });

        // Run migrations
        sqlx::migrate!("./migrations").run(&db).await.ok();

        Ok(Self {
            cluster:  Arc::new(RwLock::new(ClusterState::default())),
            event_tx,
            cache:    Cache::builder()
                          .max_capacity(1000)
                          .time_to_live(std::time::Duration::from_secs(5))
                          .build(),
            db,
            jwt_secret: std::env::var("CAIMAN_JWT_SECRET")
                .unwrap_or_else(|_| {
                    std::fs::read_to_string("/root/caiman-jwt-secret.txt")
                        .unwrap_or_else(|_| "caiman-default-secret-change-me".to_string())
                        .trim().to_string()
                }),
            ldap_url:  std::env::var("CAIMAN_LDAP_URL").ok(),
            ldap_base: std::env::var("CAIMAN_LDAP_BASE").ok(),
        })
    }
}
