//! `aomi-build status` — local deploy state plus the backend's registry/runtime
//! view, per app. GitHub credentials and CI/release verification live on the
//! backend; the CLI never calls the GitHub API here.

use std::time::Duration;

use serde::Serialize;
use serde_json::Value;

use super::types::LocalDeployment;

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
    /// live view (a single `GET /api/control/apps/status`).
    pub async fn collect(state: &LocalDeployment, backend_url: Option<String>) -> Self {
        let rows = match &backend_url {
            Some(url) => fetch_apps(url).await,
            None => None,
        };

        let apps = state
            .deployment
            .platform
            .apps
            .iter()
            .map(|app| AppStatus {
                name: app.name.clone(),
                release_tag: app.release_tag.clone(),
                activated_locally: app.activated.unwrap_or(false),
                backend: match &rows {
                    None => BackendAppStatus::NotChecked,
                    Some(Err(detail)) => BackendAppStatus::Unknown {
                        detail: detail.clone(),
                    },
                    Some(Ok(rows)) => backend_row(rows, &app.name),
                },
            })
            .collect();

        Self {
            platform: state.deployment.platform.platform.clone(),
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

fn backend_row(rows: &[Value], name: &str) -> BackendAppStatus {
    let needle = name.trim().to_ascii_lowercase();
    let Some(row) = rows.iter().find(|row| {
        row.get("name")
            .and_then(|n| n.as_str())
            .map(|n| n.eq_ignore_ascii_case(&needle))
            .unwrap_or(false)
    }) else {
        return BackendAppStatus::NotRegistered;
    };
    BackendAppStatus::Found {
        is_active: row
            .get("is_active")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        loaded: row.get("loaded").and_then(Value::as_bool).unwrap_or(false),
    }
}

/// `Some(Ok(rows))` on success, `Some(Err(detail))` on failure. Returns the
/// `apps` array from `GET /api/control/apps/status`.
async fn fetch_apps(backend_url: &str) -> Option<Result<Vec<Value>, String>> {
    let base = backend_url.trim().trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .user_agent(UA)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let result = async {
        let resp = client
            .get(format!("{base}/api/control/apps/status"))
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("backend returned {}", resp.status()));
        }
        let value: Value = resp.json().await.map_err(|e| e.to_string())?;
        Ok(value
            .get("apps")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }
    .await;
    Some(result)
}
