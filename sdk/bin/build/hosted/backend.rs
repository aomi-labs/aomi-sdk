use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::platform::Platform;
use super::types::{
    ActivateInput, ActivateResult, CreateTemplateInput, DeployInput, DeployResult, MintTokenInput,
    MintTokenResult, SourceResult, SyncSourceInput,
};

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

    /// Mint a platform/app activation token:
    /// `POST /api/platforms/:platform/tokens`. The bearer here is the caller's
    /// *privileged* admin/service AomiBearer (signed from `AOMI_ADMIN_KEY`),
    /// not an activation token — that's what this call bootstraps.
    pub async fn mint_token(
        &self,
        platform: &Platform,
        request: &MintTokenInput,
    ) -> Result<MintTokenResult> {
        self.post(
            &format!("/api/platforms/{}/tokens", platform.as_str()),
            request,
            "token mint",
        )
        .await
    }

    /// List a platform's activation tokens: `GET /api/platforms/:platform/tokens`.
    pub async fn list_tokens(&self, platform: &Platform) -> Result<serde_json::Value> {
        self.get(
            &format!("/api/platforms/{}/tokens", platform.as_str()),
            "token list",
        )
        .await
    }

    /// Revoke a token: `DELETE /api/platforms/:platform/tokens/:id`.
    pub async fn revoke_token(&self, platform: &Platform, id: i64) -> Result<serde_json::Value> {
        self.delete(
            &format!("/api/platforms/{}/tokens/{id}", platform.as_str()),
            "token revoke",
        )
        .await
    }

    /// Resolve-or-bind an installed source repo:
    /// `POST /api/platforms/:platform/sources/sync-installed`. Returns the
    /// `app_source` row whose `id` deploy needs.
    pub async fn sync_installed(
        &self,
        platform: &Platform,
        request: &SyncSourceInput,
    ) -> Result<SourceResult> {
        self.post(
            &format!("/api/platforms/{}/sources/sync-installed", platform.as_str()),
            request,
            "source sync",
        )
        .await
    }

    /// Create a source repo from a template (one-shot scaffold):
    /// `POST /api/integrations/github-app/platforms/:platform/sources/create-from-template`.
    pub async fn create_from_template(
        &self,
        platform: &Platform,
        request: &CreateTemplateInput,
    ) -> Result<SourceResult> {
        self.post(
            &format!(
                "/api/integrations/github-app/platforms/{}/sources/create-from-template",
                platform.as_str()
            ),
            request,
            "scaffold",
        )
        .await
    }

    /// List a platform's apps: `GET /api/platforms/:platform/apps`.
    pub async fn list_apps(&self, platform: &Platform) -> Result<serde_json::Value> {
        self.get(
            &format!("/api/platforms/{}/apps", platform.as_str()),
            "apps list",
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

    async fn get<Resp: DeserializeOwned>(&self, path: &str, operation: &str) -> Result<Resp> {
        self.send(reqwest::Method::GET, path, operation).await
    }

    async fn delete<Resp: DeserializeOwned>(&self, path: &str, operation: &str) -> Result<Resp> {
        self.send(reqwest::Method::DELETE, path, operation).await
    }

    /// Bodyless request (GET/DELETE) sharing `post`'s status + JSON handling.
    async fn send<Resp: DeserializeOwned>(
        &self,
        method: reqwest::Method,
        path: &str,
        operation: &str,
    ) -> Result<Resp> {
        let endpoint = self.endpoint(path);
        let response = self
            .http
            .request(method, &endpoint)
            .bearer_auth(&self.bearer)
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
