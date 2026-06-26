//! `aomi-build status` — local deploy state plus the backend's app/runtime
//! view, per app. GitHub credentials and CI/release verification live on the
//! backend; the CLI never calls the GitHub API here.

use std::time::Duration;

use super::types::LocalDeployment;
use serde::Serialize;

const UA: &str = concat!("aomi-build/", env!("CARGO_PKG_VERSION"));
const PROBE_TIMEOUT: Duration = Duration::from_secs(12);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Debug, Serialize)]
pub struct StatusResult {
    pub platform: String,
    pub pr_url: String,
    pub deploy_branch: String,
    pub deployed: bool,
    pub activated: bool,
    pub backend: Option<String>,
    pub apps: Vec<AppStatus>,
}

#[derive(Debug, Serialize)]
pub struct AppStatus {
    pub name: String,
    pub release_tag: String,
    /// What the local `.aomi/deployment.json` last recorded.
    pub activated_locally: bool,
    /// What the backend reports, when reachable.
    #[serde(flatten)]
    pub backend: BackendAppStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "backend_state")]
pub enum BackendAppStatus {
    NotChecked,
    NotRegistered,
    Found { is_active: bool, loaded: bool },
    Unknown { detail: String },
}

impl StatusResult {
    /// Collect a report from local state, optionally enriched with the backend's
    /// live view for each deployed release.
    pub async fn collect(
        state: &LocalDeployment,
        backend_url: Option<String>,
        activation_token: Option<String>,
    ) -> Self {
        let platform = state.deployment.platform.platform.clone();
        let client = match (&backend_url, &activation_token) {
            (Some(url), Some(token)) => probe_client(url, token).ok(),
            _ => None,
        };

        let mut apps = Vec::with_capacity(state.deployment.platform.apps.len());
        for app in &state.deployment.platform.apps {
            let backend = match &client {
                None => BackendAppStatus::NotChecked,
                Some(client) => {
                    match fetch_app(client, &platform, &app.name, &app.release_tag).await {
                        Ok(status) => status,
                        Err(detail) => BackendAppStatus::Unknown { detail },
                    }
                }
            };
            apps.push(AppStatus {
                name: app.name.clone(),
                release_tag: app.release_tag.clone(),
                activated_locally: app.activated.unwrap_or(false),
                backend,
            });
        }

        Self {
            platform,
            pr_url: state.deployment.platform.pr_url.clone().unwrap_or_default(),
            deploy_branch: state.deployment.platform.deploy_branch.clone(),
            deployed: state.state.deployed,
            activated: state.state.activated,
            backend: backend_url,
            apps,
        }
    }

    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "Deployment status");
        let _ = writeln!(out, "  platform      : {}", self.platform);
        let _ = writeln!(out, "  pr            : {}", self.pr_url);
        let _ = writeln!(out, "  deploy_branch : {}", self.deploy_branch);
        let _ = writeln!(
            out,
            "  local state   : deployed={} activated={}",
            self.deployed, self.activated
        );
        match &self.backend {
            Some(url) => {
                let _ = writeln!(out, "  backend       : {url}");
            }
            None => {
                let _ = writeln!(out, "  backend       : not checked");
            }
        }
        for app in &self.apps {
            let _ = writeln!(out, "  - {} ({})", app.name, app.release_tag);
            let _ = writeln!(out, "      local     : activated={}", app.activated_locally);
            match &app.backend {
                BackendAppStatus::NotChecked => {}
                BackendAppStatus::NotRegistered => {
                    let _ = writeln!(out, "      backend   : not activated yet");
                }
                BackendAppStatus::Unknown { detail } => {
                    let _ = writeln!(out, "      backend   : unknown ({detail})");
                }
                BackendAppStatus::Found { is_active, loaded } => {
                    let health = if *loaded { "loaded" } else { "not loaded" };
                    let _ = writeln!(out, "      backend   : active={is_active} {health}");
                }
            }
        }
        out
    }
}

struct ProbeClient {
    base_url: String,
    bearer: String,
    http: reqwest::Client,
}

fn probe_client(backend_url: &str, bearer: &str) -> Result<ProbeClient, String> {
    let base_url = backend_url.trim().trim_end_matches('/').to_string();
    if base_url.is_empty() || bearer.trim().is_empty() {
        return Err("missing backend URL or activation token".to_string());
    }
    let client = reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .user_agent(UA)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    Ok(ProbeClient {
        base_url,
        bearer: bearer.to_string(),
        http: client,
    })
}

async fn fetch_app(
    client: &ProbeClient,
    platform: &str,
    name: &str,
    release_tag: &str,
) -> Result<BackendAppStatus, String> {
    let mut url = reqwest::Url::parse(&format!("{}/", client.base_url))
        .map_err(|e| format!("invalid backend URL: {e}"))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "backend URL cannot be a base URL".to_string())?;
        segments
            .push("api")
            .push("platforms")
            .push(platform)
            .push("apps")
            .push(name);
    }
    url.query_pairs_mut()
        .append_pair("release_tag", release_tag);

    let resp = client
        .http
        .get(url)
        .bearer_auth(&client.bearer)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if resp.status().as_u16() == 404 {
        return Ok(BackendAppStatus::NotRegistered);
    }
    if !resp.status().is_success() {
        return Err(format!("backend returned {}", resp.status()));
    }
    let value: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    let Some(app) = value.get("app") else {
        return Ok(BackendAppStatus::NotRegistered);
    };
    Ok(BackendAppStatus::Found {
        is_active: app
            .get("is_active")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        loaded: app
            .get("loaded")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}
