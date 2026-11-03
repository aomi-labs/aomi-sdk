//! Public app catalog on the Aomi Chat host. It needs no credential, so the
//! CLI reads application ids from it when the Builder routes cannot serve them.

use anyhow::{Context, Result};

use super::backend::http_client;
use super::types::PlatformAppStatus;

#[derive(Clone, Debug)]
pub struct ChatCatalog {
    base_url: String,
    http: reqwest::Client,
}

impl ChatCatalog {
    /// The Chat host paired with a known staging/production backend, matching
    /// how the Build URL is inferred. An unknown backend has no catalog.
    pub fn for_backend(backend_url: &str) -> Option<Self> {
        let base_url = match backend_url.trim().trim_end_matches('/') {
            "https://api-staging.aomi.dev" => "https://chat-staging.aomi.dev",
            "https://api.aomi.dev" => "https://chat.aomi.dev",
            _ => return None,
        };
        Some(Self {
            base_url: base_url.to_string(),
            http: http_client(),
        })
    }

    /// Public apps on `platform`: `GET /api/public/catalog/apps?platform=…`.
    /// Private apps are absent.
    pub async fn apps(&self, platform: &str) -> Result<Vec<PlatformAppStatus>> {
        let endpoint = format!("{}/api/public/catalog/apps", self.base_url);
        self.http
            .get(&endpoint)
            .query(&[("platform", platform)])
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .with_context(|| format!("failed to call app catalog {endpoint}"))?
            .json()
            .await
            .with_context(|| format!("app catalog {endpoint} returned invalid JSON"))
    }
}
