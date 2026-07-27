//! Builder-authenticated Aomi Build BFF client.
//!
//! Human CLI deploys use the same ownership-checked surface as the Build UI.
//! Privileged activation tokens remain in `backend.rs` for admin/CI commands.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use reqwest::Url;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::types::{
    ActivateResult, BuildActivateInput, BuildDeployInput, BuildDeployResult, CliExchangeInput,
    CliExchangeResult, CliStatusResult, DeploymentStatusResult,
};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Clone, Debug)]
pub struct BuildClient {
    base_url: String,
    bearer: String,
    http: reqwest::Client,
}

impl BuildClient {
    pub fn new(base_url: impl Into<String>, bearer: impl Into<String>) -> Result<Self> {
        let base_url = base_url.into().trim().trim_end_matches('/').to_string();
        let bearer = bearer.into().trim().to_string();
        if base_url.is_empty() {
            bail!("Aomi Build URL is required");
        }
        if bearer.is_empty() {
            bail!("Aomi Build login is required; run `aomi-build login`");
        }
        Ok(Self {
            base_url,
            bearer,
            http: http_client(),
        })
    }

    pub async fn deploy(
        &self,
        request: &BuildDeployInput,
        preflight: bool,
    ) -> Result<BuildDeployResult> {
        let path = if preflight {
            "/api/bff/deployments/preflight"
        } else {
            "/api/bff/deployments/deploy"
        };
        self.post(
            path,
            request,
            if preflight { "preflight" } else { "deploy" },
        )
        .await
    }

    pub async fn status(
        &self,
        platform: &str,
        deployment_id: &str,
    ) -> Result<DeploymentStatusResult> {
        let mut url = self.url("/api/bff/deployments/status")?;
        url.query_pairs_mut()
            .append_pair("deploymentId", deployment_id)
            .append_pair("platform", platform);
        self.send(self.http.get(url).bearer_auth(&self.bearer), "status")
            .await
    }

    pub async fn activate(&self, request: &BuildActivateInput) -> Result<ActivateResult> {
        self.post("/api/bff/launch/activate", request, "activation")
            .await
    }

    pub async fn whoami(&self) -> Result<CliStatusResult> {
        let url = self.url("/api/bff/cli/status")?;
        self.send(self.http.get(url).bearer_auth(&self.bearer), "login status")
            .await
    }

    fn url(&self, path: &str) -> Result<Url> {
        Url::parse(&format!("{}{}", self.base_url, path))
            .with_context(|| format!("invalid Aomi Build URL `{}`", self.base_url))
    }

    async fn post<Req: Serialize, Resp: DeserializeOwned>(
        &self,
        path: &str,
        body: &Req,
        operation: &str,
    ) -> Result<Resp> {
        let url = self.url(path)?;
        self.send(
            self.http.post(url).bearer_auth(&self.bearer).json(body),
            operation,
        )
        .await
    }

    async fn send<Resp: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        operation: &str,
    ) -> Result<Resp> {
        let response = request
            .send()
            .await
            .with_context(|| format!("failed to call Aomi Build {operation}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .with_context(|| format!("failed to read Aomi Build {operation} response"))?;
        if !matches!(status.as_u16(), 200..=202) {
            bail!("Aomi Build {operation} returned {status}: {}", text.trim());
        }
        serde_json::from_str(&text)
            .with_context(|| format!("Aomi Build {operation} returned invalid JSON"))
    }
}

pub async fn exchange_cli_code(
    build_url: &str,
    code: String,
    code_verifier: String,
) -> Result<CliExchangeResult> {
    let base = build_url.trim().trim_end_matches('/');
    let endpoint = format!("{base}/api/bff/cli/exchange");
    let response = http_client()
        .post(&endpoint)
        .json(&CliExchangeInput {
            code,
            code_verifier,
        })
        .send()
        .await
        .with_context(|| format!("failed to call CLI exchange endpoint {endpoint}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read CLI exchange response")?;
    if !matches!(status.as_u16(), 200 | 201) {
        bail!("CLI exchange returned {status}: {}", text.trim());
    }
    serde_json::from_str(&text).context("CLI exchange returned invalid JSON")
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}
