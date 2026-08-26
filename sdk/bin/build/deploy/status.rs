//! `aomi-build status` — local deploy state plus the backend's app/runtime
//! view, per app. GitHub credentials and CI/release verification live on the
//! backend; the CLI never calls the GitHub API here.

use super::backend::BackendClient;
use super::platform::Platform;
use super::state::LocalDeployment;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct StatusResult {
    pub platform: String,
    pub deployment_id: String,
    pub pr_url: String,
    pub deploy_branch: String,
    pub deployed: bool,
    pub activated: bool,
    pub project_url: Option<String>,
    pub backend: Option<String>,
    pub deployment: DeploymentBackendStatus,
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
    Found {
        is_active: bool,
        artifact_ready: bool,
        loaded: bool,
    },
    Unknown {
        detail: String,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "deployment_state")]
pub enum DeploymentBackendStatus {
    NotChecked,
    Found {
        state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ci_url: Option<String>,
    },
    Unknown {
        detail: String,
    },
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
        let platform_tag = Platform::new(&platform);
        let client = match (&backend_url, &activation_token) {
            (Some(url), Some(token)) => BackendClient::new(url.clone(), token.clone()).ok(),
            _ => None,
        };
        let deployment = match &client {
            None => DeploymentBackendStatus::NotChecked,
            Some(client) => {
                fetch_deployment_status(client, &platform_tag, &state.deployment.id).await
            }
        };

        let mut apps = Vec::with_capacity(state.deployment.platform.apps.len());
        for app in &state.deployment.platform.apps {
            let backend = match &client {
                None => BackendAppStatus::NotChecked,
                Some(client) => fetch_app(client, &platform_tag, &app.name, &app.release_tag).await,
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
            deployment_id: state.deployment.id.clone(),
            pr_url: state.deployment.platform.pr_url.clone().unwrap_or_default(),
            deploy_branch: state.deployment.platform.deploy_branch.clone(),
            deployed: state.state.deployed,
            activated: state.state.activated,
            project_url: state.project_url.clone(),
            backend: backend_url,
            deployment,
            apps,
        }
    }

    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "Deployment status");
        let _ = writeln!(out, "  platform      : {}", self.platform);
        let _ = writeln!(out, "  deployment_id : {}", self.deployment_id);
        let _ = writeln!(out, "  pr            : {}", self.pr_url);
        let _ = writeln!(out, "  deploy_branch : {}", self.deploy_branch);
        if let Some(url) = &self.project_url {
            let _ = writeln!(out, "  project       : {url}");
        }
        let _ = writeln!(
            out,
            "  local state   : deployed={} activated={}",
            self.deployed, self.activated
        );
        let _ = writeln!(
            out,
            "  backend       : {}",
            self.backend.as_deref().unwrap_or("not checked")
        );
        match &self.deployment {
            DeploymentBackendStatus::NotChecked => {}
            DeploymentBackendStatus::Found {
                state,
                message,
                ci_url,
            } => {
                let detail = message.as_deref().unwrap_or("");
                if detail.is_empty() {
                    let _ = writeln!(out, "  deploy state  : {state}");
                } else {
                    let _ = writeln!(out, "  deploy state  : {state} ({detail})");
                }
                if let Some(url) = ci_url {
                    let _ = writeln!(out, "  build logs    : {url}");
                }
            }
            DeploymentBackendStatus::Unknown { detail } => {
                let _ = writeln!(out, "  deploy state  : unknown ({detail})");
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
                BackendAppStatus::Found {
                    is_active,
                    artifact_ready,
                    loaded,
                } => {
                    let health = if *loaded { "loaded" } else { "not loaded" };
                    let _ = writeln!(
                        out,
                        "      backend   : active={is_active} artifact_ready={artifact_ready} {health}"
                    );
                }
            }
        }
        out
    }
}

async fn fetch_deployment_status(
    client: &BackendClient,
    platform: &Platform,
    deployment_id: &str,
) -> DeploymentBackendStatus {
    match client.deployment_status(platform, deployment_id).await {
        Ok(status) => DeploymentBackendStatus::Found {
            state: status.state,
            message: status.message,
            ci_url: status.ci.and_then(|ci| ci.url),
        },
        Err(err) => DeploymentBackendStatus::Unknown {
            detail: err.to_string(),
        },
    }
}

async fn fetch_app(
    client: &BackendClient,
    platform: &Platform,
    name: &str,
    release_tag: &str,
) -> BackendAppStatus {
    match client.get_app(platform, name, release_tag).await {
        Ok(live) => BackendAppStatus::Found {
            is_active: live.app.is_active,
            artifact_ready: live.app.artifact_ready,
            loaded: live.app.loaded,
        },
        Err(err) if err.to_string().contains("returned 404") => BackendAppStatus::NotRegistered,
        Err(err) => BackendAppStatus::Unknown {
            detail: err.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_prints_build_logs_url() {
        let report = StatusResult {
            platform: "community".into(),
            deployment_id: "dep_1".into(),
            pr_url: String::new(),
            deploy_branch: "publish".into(),
            deployed: true,
            activated: false,
            project_url: None,
            backend: None,
            deployment: DeploymentBackendStatus::Found {
                state: "failed".into(),
                message: Some("release build failed".into()),
                ci_url: Some("https://github.com/aomi-labs/community-apps/actions/runs/1".into()),
            },
            apps: vec![],
        };
        assert!(report.render().contains(
            "build logs    : https://github.com/aomi-labs/community-apps/actions/runs/1"
        ));
    }
}
