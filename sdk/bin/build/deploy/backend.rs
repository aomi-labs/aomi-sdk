use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::platform::Platform;
use super::types::{
    ActivateInput, ActivateResult, DeploymentStatusResult, MintTokenInput, MintTokenResult,
    OAuthStart, PlatformAppResult, SourceResult, SyncSourceInput,
};

/// GET the aomi-build GitHub App install URL for `platform`. Query params mirror
/// the portal's bootstrap path: `repo` scopes the install to one repo, and
/// `mode=authorize` re-consents an already-installed App (vs a fresh `install`).
/// Like the portal this call carries no activation bearer — install association
/// is an account-session concern the CLI doesn't own — so it doesn't go through
/// [`BackendClient`].
pub async fn oauth_start(
    base_url: &str,
    platform: &str,
    repo: Option<&str>,
    mode: &str,
) -> Result<OAuthStart> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        bail!("backend URL is required via --backend or AOMI_BACKEND_URL");
    }
    let endpoint = format!("{base}/api/integrations/github-app/oauth/start");
    let http = http_client();
    let mut query = vec![("platform", platform.to_string())];
    if let Some(repo) = repo {
        query.push(("repo", repo.to_string()));
    }
    // The portal only sends `mode` for re-authorization; `install` is the default.
    if mode == "authorize" {
        query.push(("mode", "authorize".to_string()));
    }
    let response = http
        .get(&endpoint)
        .query(&query)
        .send()
        .await
        .with_context(|| format!("failed to call oauth start endpoint {endpoint}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read oauth start response body")?;
    if !matches!(status.as_u16(), 200 | 201) {
        bail!(
            "oauth start endpoint {endpoint} returned {status}: {}",
            text.trim()
        );
    }
    serde_json::from_str(&text)
        .with_context(|| format!("oauth start endpoint {endpoint} returned invalid JSON"))
}

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

pub(crate) fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Outcome of probing an activation token against a platform. Separates an
/// auth rejection from a transport failure so callers don't report "bad token"
/// when the backend was merely unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenCheck {
    Valid,
    Invalid,
    Unreachable,
}

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
        Ok(Self {
            base_url,
            bearer,
            http: http_client(),
        })
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
            &format!(
                "/api/platforms/{}/sources/sync-installed",
                platform.as_str()
            ),
            request,
            "source sync",
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

    pub async fn get_app(
        &self,
        platform: &Platform,
        app: &str,
        release_tag: &str,
    ) -> Result<PlatformAppResult> {
        self.get_query(
            &format!("/api/platforms/{}/apps/{}", platform.as_str(), app),
            &[("release_tag", release_tag)],
            "app status",
        )
        .await
    }

    /// Build status of a deployment:
    /// `GET /api/platforms/:platform/deployments/:id/status`. Matches the
    /// portal's poll source — used to gate activation on the release build.
    pub async fn deployment_status(
        &self,
        platform: &Platform,
        deployment_id: &str,
    ) -> Result<DeploymentStatusResult> {
        self.get(
            &format!(
                "/api/platforms/{}/deployments/{}/status",
                platform.as_str(),
                deployment_id
            ),
            "deployment status",
        )
        .await
    }

    /// Probe whether the bearer is a valid activation token for `platform`. A
    /// successful `apps` read means valid; a 401/403 means the token was
    /// rejected; anything else (transport error, 5xx, etc.) is `Unreachable` so
    /// a network blip isn't misreported as a bad token.
    pub async fn check_token(&self, platform: &Platform) -> TokenCheck {
        let endpoint = self.endpoint(&format!("/api/platforms/{}/apps", platform.as_str()));
        match self
            .http
            .get(&endpoint)
            .bearer_auth(&self.bearer)
            .send()
            .await
        {
            Err(_) => TokenCheck::Unreachable,
            Ok(resp) => match resp.status().as_u16() {
                200 | 201 => TokenCheck::Valid,
                401 | 403 => TokenCheck::Invalid,
                _ => TokenCheck::Unreachable,
            },
        }
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
        decode_response(response, operation, &endpoint).await
    }

    async fn get<Resp: DeserializeOwned>(&self, path: &str, operation: &str) -> Result<Resp> {
        self.send(reqwest::Method::GET, path, operation).await
    }

    async fn get_query<Resp: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, &str)],
        operation: &str,
    ) -> Result<Resp> {
        let endpoint = self.endpoint(path);
        let response = self
            .http
            .get(&endpoint)
            .bearer_auth(&self.bearer)
            .query(query)
            .send()
            .await
            .with_context(|| format!("failed to call {operation} endpoint {endpoint}"))?;
        decode_response(response, operation, &endpoint).await
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
        decode_response(response, operation, &endpoint).await
    }
}

async fn decode_response<Resp: DeserializeOwned>(
    response: reqwest::Response,
    operation: &str,
    endpoint: &str,
) -> Result<Resp> {
    let status = response.status();
    let text = response
        .text()
        .await
        .with_context(|| format!("failed to read {operation} response body"))?;
    if !status.is_success() {
        bail!(
            "{operation} endpoint {endpoint} returned {status}: {}",
            text.trim()
        );
    }
    serde_json::from_str(&text)
        .with_context(|| format!("{operation} endpoint {endpoint} returned invalid JSON"))
}
