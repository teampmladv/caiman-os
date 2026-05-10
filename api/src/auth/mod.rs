//! auth/mod.rs -- JWT authentication

use axum::{
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use chrono::Utc;

const JWT_EXPIRY_HOURS: i64 = 24;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub:      String,
    pub role:     String,
    pub provider: String,
    pub exp:      usize,
    pub iat:      usize,
}

#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token:    String,
    pub username: String,
    pub role:     String,
    pub expires:  String,
}

#[derive(Debug, Serialize)]
pub struct ErrResp { error: String }

pub fn get_secret() -> String {
    std::env::var("CAIMAN_JWT_SECRET").unwrap_or_else(|_|
        std::fs::read_to_string("/root/caiman-jwt-secret.txt")
            .unwrap_or_else(|_| "caiman-dev-secret".to_string())
            .trim().to_string()
    )
}

fn make_token(username: &str, role: &str, provider: &str)
    -> Result<Json<TokenResponse>, (StatusCode, Json<ErrResp>)>
{
    let secret = get_secret();
    let now = Utc::now();
    let exp = now + chrono::Duration::hours(JWT_EXPIRY_HOURS);
    let claims = Claims {
        sub:      username.to_string(),
        role:     role.to_string(),
        provider: provider.to_string(),
        iat:      now.timestamp() as usize,
        exp:      exp.timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ).map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrResp { error: "Token generation failed".into() }),
    ))?;
    Ok(Json(TokenResponse {
        token,
        username: username.to_string(),
        role: role.to_string(),
        expires: exp.to_rfc3339(),
    }))
}

// POST /auth/bootstrap -- no auth required
pub async fn bootstrap_token() -> Result<Json<TokenResponse>, (StatusCode, Json<ErrResp>)> {
    make_token("admin", "admin", "local")
}

// POST /auth/token -- login with credentials
pub async fn generate_token(
    Json(req): Json<TokenRequest>,
) -> Result<Json<TokenResponse>, (StatusCode, Json<ErrResp>)> {
    if req.password.len() < 4 {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrResp { error: "Invalid credentials".into() }),
        ));
    }
    let role = if req.username == "root" || req.username == "admin" {
        "admin"
    } else {
        "operator"
    };
    make_token(&req.username, role, "local")
}

pub fn verify_token(token: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let secret = get_secret();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ).map(|d| d.claims)
}

// Axum middleware
pub async fn require_auth(
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, (StatusCode, Json<ErrResp>)> {
    let err = || (
        StatusCode::UNAUTHORIZED,
        Json(ErrResp { error: "Missing or invalid token".into() }),
    );
    let auth = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(err)?;

    verify_token(auth).map_err(|_| err())?;
    Ok(next.run(req).await)
}
