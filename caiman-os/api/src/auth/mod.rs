//! auth/mod.rs — JWT authentication (stub)
pub fn router() -> axum::Router {
    axum::Router::new()
        .route("/auth/login", axum::routing::post(login_handler))
}
async fn login_handler() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({ "status": "TODO" }))
}
