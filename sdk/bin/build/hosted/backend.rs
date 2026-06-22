use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::platform::Platform;
use super::types::{ActivateInput, ActivateResult, DeployInput, DeployResult};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

/// Authenticated relay to the Aomi backend's repo-scoped deploy/activate
/// endpoints. The CLI never talks to GitHub; every privileged call goes through
/// here with the caller's activation bearer.
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
            bail!("backend URL is required via --backend or AOMI_BACKEND_URL");
        }
        if bearer.trim().is_empty() {
            bail!(
                "activation token is required via --activation-token or AOMI_APP_ACTIVATION_TOKEN"
            );
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

    /// Repo-scoped deploy: `POST /api/platforms/:platform/deploy`.
    pub async fn deploy(&self, platform: &Platform, request: &DeployInput) -> Result<DeployResult> {
        self.post(
            &format!("/api/platforms/{}/deploy", platform.as_str()),
            request,
            "deploy",
        )
        .await
    }

    /// Release-tags activation:
    /// `POST /api/platforms/:platform/apps/activate`.
    pub async fn activate(
        &self,
        platform: &Platform,
        request: &ActivateInput,
    ) -> Result<ActivateResult> {
        self.post(
            &format!("/api/platforms/{}/apps/activate", platform.as_str()),
            request,
            "activation",
        )
        .await
    }

    pub fn endpoint(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    async fn post<Req: Serialize, Resp: DeserializeOwned>(
        &self,
        path: &str,
        body: &Req,
        operation: &str,
    ) -> Result<Resp> {
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
        let text = response
            .text()
            .await
            .with_context(|| format!("failed to read {operation} response body"))?;
        if !matches!(status.as_u16(), 200 | 201) {
            bail!(
                "{operation} endpoint {endpoint} returned {status}: {}",
                text.trim()
            );
        }
        serde_json::from_str(&text)
            .with_context(|| format!("{operation} endpoint {endpoint} returned invalid JSON"))
    }
}
