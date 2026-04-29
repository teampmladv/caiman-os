//! api/mod.rs — HTTP client wrapper for caiman-api

use anyhow::{bail, Context, Result};
use reqwest::{Client as Http, Response};
use serde::{de::DeserializeOwned, Serialize};
use tracing::debug;

pub struct Client {
    http:    Http,
    base:    String,
    token:   Option<String>,
    verbose: bool,
}

impl Client {
    pub fn new(base: String, token: Option<String>, verbose: bool) -> Self {
        let http = Http::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("building HTTP client");
        Self { http, base, token, verbose }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base.trim_end_matches('/'), path)
    }

    fn auth(&self, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(tok) = &self.token {
            req.bearer_auth(tok)
        } else {
            req
        }
    }

    pub async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let url = self.url(path);
        debug!("GET {url}");
        let res = self.auth(self.http.get(&url)).send().await
            .with_context(|| format!("GET {url}"))?;
        self.parse(res).await
    }

    pub async fn post<B: Serialize>(&self, path: &str, body: &B) -> Result<serde_json::Value> {
        let url = self.url(path);
        debug!("POST {url}");
        let res = self.auth(self.http.post(&url).json(body)).send().await
            .with_context(|| format!("POST {url}"))?;
        self.parse(res).await
    }

    pub async fn post_empty(&self, path: &str) -> Result<serde_json::Value> {
        self.post(path, &serde_json::json!({})).await
    }

    pub async fn patch<B: Serialize>(&self, path: &str, body: &B) -> Result<serde_json::Value> {
        let url = self.url(path);
        let res = self.auth(self.http.patch(&url).json(body)).send().await?;
        self.parse(res).await
    }

    pub async fn delete(&self, path: &str) -> Result<serde_json::Value> {
        let url = self.url(path);
        let res = self.auth(self.http.delete(&url)).send().await?;
        self.parse(res).await
    }

    pub async fn get_typed<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let val = self.get(path).await?;
        serde_json::from_value(val).context("parsing response")
    }

    async fn parse(&self, res: Response) -> Result<serde_json::Value> {
        let status = res.status();
        let body   = res.text().await.unwrap_or_default();

        if self.verbose {
            eprintln!("← {} {}", status.as_u16(), &body[..body.len().min(500)]);
        }

        if !status.is_success() {
            let msg = serde_json::from_str::<serde_json::Value>(&body)
                .ok()
                .and_then(|v| v["error"].as_str().map(|s| s.to_string()))
                .unwrap_or(body);
            bail!("API error {}: {}", status.as_u16(), msg);
        }

        if body.is_empty() {
            return Ok(serde_json::json!({}));
        }
        serde_json::from_str(&body).context("parsing JSON response")
    }

    /// Base URL for WS connection
    pub fn ws_url(&self, path: &str) -> String {
        self.base.replacen("http", "ws", 1) + path
    }
}
