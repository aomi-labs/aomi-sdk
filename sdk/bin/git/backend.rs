use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::Value;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Clone, Debug)]
pub struct BackendClient {
    base_url: String,
    bearer: String,
    http: reqwest::Client,
}

impl BackendClient {
    pub fn new(base_url: String, bearer: String) -> Result<Self> {
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            bail!("backend URL is required");
        }
        if bearer.trim().is_empty() {
            bail!("backend bearer token is required");
        }
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Ok(Self {
            base_url,
            bearer,
            http,
        })
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    pub async fn post_json<T: Serialize>(
        &self,
        path: &str,
        body: &T,
        operation: &str,
    ) -> Result<Value> {
        let endpoint = self.endpoint(path);
        let response = self
            .http
            .post(&endpoint)
            .bearer_auth(&self.bearer)
            .json(body)
            .send()
            .await
            .with_context(|| format!("failed to call {operation} endpoint {endpoint}"))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .with_context(|| format!("failed to read {operation} response body"))?;
        if !matches!(status.as_u16(), 200 | 201) {
            bail!(
                "{operation} endpoint {endpoint} returned {status}: {}",
                body.trim()
            );
        }

        serde_json::from_str(&body)
            .with_context(|| format!("{operation} endpoint {endpoint} returned invalid JSON"))
    }
}
