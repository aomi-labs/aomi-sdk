//! Builder-authenticated Aomi Build BFF client.
//!
//! Human CLI deploys use the same ownership-checked surface as the Build UI.
//! Privileged activation tokens remain in `backend.rs` for admin/CI commands.

use anyhow::{Context, Result, anyhow, bail};
use reqwest::Url;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::backend::http_client;
use super::types::{
    ActivateResult, BuildActivateInput, BuildDeployInput, BuildDeployResult, CliExchangeInput,
    CliExchangeResult, CliStatusResult, DeploymentStatusResult,
};

/// Result of checking a stored credential against Aomi Build.
pub enum AuthProbe {
    SignedIn(CliStatusResult),
    /// Build answered and refused the credential — a browser login is the fix.
    Rejected,
    /// Build could not be reached or answered something unusable. The saved
    /// credential may be perfectly good, so this must not trigger a re-login.
    Unreachable(anyhow::Error),
}

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

    /// Probe the saved credential, separating "Build says this login is no
    /// longer valid" from "Build could not be reached".
    ///
    /// Collapsing the two is what made an offline laptop or a down backend look
    /// identical to an expired session and pop a browser window at the user.
    pub async fn probe_identity(&self) -> AuthProbe {
        let url = match self.url("/api/bff/cli/status") {
            Ok(url) => url,
            Err(error) => return AuthProbe::Unreachable(error),
        };
        let response = match self.http.get(url).bearer_auth(&self.bearer).send().await {
            Ok(response) => response,
            Err(error) => {
                return AuthProbe::Unreachable(
                    anyhow::Error::new(error).context("failed to reach Aomi Build"),
                );
            }
        };
        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return AuthProbe::Rejected;
        }
        let text = match response.text().await {
            Ok(text) => text,
            Err(error) => {
                return AuthProbe::Unreachable(
                    anyhow::Error::new(error).context("failed to read Aomi Build login status"),
                );
            }
        };
        if !status.is_success() {
            return AuthProbe::Unreachable(anyhow!(
                "Aomi Build login status returned {status}: {}",
                text.trim()
            ));
        }
        match serde_json::from_str::<CliStatusResult>(&text) {
            // A 200 that reports `signedIn: false` is Build declining the
            // credential, not a transport problem.
            Ok(status) if status.signed_in => AuthProbe::SignedIn(status),
            Ok(_) => AuthProbe::Rejected,
            Err(error) => AuthProbe::Unreachable(
                anyhow::Error::new(error).context("Aomi Build login status returned invalid JSON"),
            ),
        }
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
        decode_response(response, operation, 200..=202).await
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
    decode_response(response, "CLI exchange", 200..=201).await
}

async fn decode_response<Resp: DeserializeOwned>(
    response: reqwest::Response,
    operation: &str,
    success: std::ops::RangeInclusive<u16>,
) -> Result<Resp> {
    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("failed to read Aomi Build {operation} response"))?;
    if !success.contains(&status.as_u16()) {
        bail!("Aomi Build {operation} returned {status}: {}", text.trim());
    }
    serde_json::from_str(&text)
        .with_context(|| format!("Aomi Build {operation} returned invalid JSON"))
}
