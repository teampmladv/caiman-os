//! auth/mod.rs -- JWT authentication for caiman-api
//! Roles: read-only | operator | admin
//! Tokens format: caim_<jwt>

use axum::{
    extract::Extension,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Json, Response},
    body::Body,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn signing_key() -> String {
    std::env::var("CAIMAN_JWT_SECRET")
        .unwrap_or_else(|_| "caiman-dev-secret-change-in-production".to_string())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role { ReadOnly, Operator, Admin }

impl Role {
    pub fn can_operate(&self) -> bool { matches!(self, Role::Operator | Role::Admin) }
    pub fn can_admin(&self)   -> bool { matches!(self, Role::Admin) }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, pub jti: String, pub role: Role,
    pub cluster: String, pub iat: i64, pub exp: i64,
}

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub name: String, pub role: Role, pub expires: String,
    #[serde(default = "default_cluster")]
    pub cluster: String,
}
fn default_cluster() -> String { "default".to_string() }

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String, pub expires_at: Option<String>,
    pub role: Role, pub cluster: String, pub name: String,
}

fn parse_exp(s: &str) -> Result<i64, &'static str> {
    let now = Utc::now();
    match s {
        "never" => Ok((now + Duration::days(3650)).timestamp()),
        s if s.ends_with('h') => {
            let h: i64 = s.trim_end_matches('h').parse().map_err(|_| "bad hours")?;
            Ok((now + Duration::hours(h)).timestamp())
        }
        s if s.ends_with('d') => {
            let d: i64 = s.trim_end_matches('d').parse().map_err(|_| "bad days")?;
            Ok((now + Duration::days(d)).timestamp())
        }
        s if s.ends_with('y') => {
            let y: i64 = s.trim_end_matches('y').parse().map_err(|_| "bad years")?;
            Ok((now + Duration::days(y * 365)).timestamp())
        }
        _ => Err("use: 1h | 7d | 30d | 1y | never"),
    }
}

fn mint(name: &str, role: Role, cluster: &str, expires: &str) -> Result<TokenResponse, String> {
    let exp = parse_exp(expires).map_err(|e| e.to_string())?;
    let claims = Claims {
        sub: name.to_string(), jti: Uuid::new_v4().to_string(),
        role: role.clone(), cluster: cluster.to_string(),
        iat: Utc::now().timestamp(), exp,
    };
    let jwt = encode(&Header::default(), &claims,
        &EncodingKey::from_secret(signing_key().as_bytes()))
        .map_err(|e| e.to_string())?;
    let expires_at = if expires == "never" { None } else {
        Some(chrono::DateTime::from_timestamp(exp, 0).unwrap()
            .format("%Y-%m-%dT%H:%M:%SZ").to_string())
    };
    Ok(TokenResponse {
        token: format!("caim_{jwt}"), expires_at,
        role, cluster: cluster.to_string(), name: name.to_string(),
    })
}

pub async fn generate_token(
    Extension(caller): Extension<Claims>,
    Json(req): Json<TokenRequest>,
) -> impl IntoResponse {
    if !caller.role.can_admin() {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "only admin tokens can generate new tokens"
        }))).into_response();
    }
    match mint(&req.name, req.role, &req.cluster, &req.expires) {
        Ok(r)  => (StatusCode::CREATED, Json(r)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

pub async fn bootstrap_token(Json(req): Json<TokenRequest>) -> impl IntoResponse {
    if std::env::var("CAIMAN_BOOTSTRAP_ALLOWED").unwrap_or_default() != "1" {
        return (StatusCode::FORBIDDEN, Json(serde_json::json!({
            "error": "set CAIMAN_BOOTSTRAP_ALLOWED=1 to enable bootstrap"
        }))).into_response();
    }
    match mint(&req.name, Role::Admin, &req.cluster, &req.expires) {
        Ok(r)  => (StatusCode::CREATED, Json(r)).into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))).into_response(),
    }
}

pub async fn require_auth(req: Request<Body>, next: Next) -> Response {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| t.trim_start_matches("caim_").to_string());

    let token = match token {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "error": "missing Authorization: Bearer caim_<token>"
        }))).into_response(),
    };

    let mut validation = Validation::default();
    validation.leeway = 10;
    match decode::<Claims>(&token,
        &DecodingKey::from_secret(signing_key().as_bytes()), &validation)
    {
        Ok(data) => {
            let (mut parts, body) = req.into_parts();
            parts.extensions.insert(data.claims);
            next.run(Request::from_parts(parts, body)).await
        }
        Err(e) => (StatusCode::UNAUTHORIZED, Json(serde_json::json!({
            "error": format!("invalid or expired token: {e}")
        }))).into_response(),
    }
}
