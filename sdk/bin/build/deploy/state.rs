//! `.aomi/deployment.json` — what the last deploy produced, plus the local
//! activation overlay the CLI keeps on top of it.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEPLOYMENT_DIR: &str = ".aomi";
const DEPLOYMENT_FILE: &str = "deployment.json";

use super::types::{ActivateResult, Activation, BuildDeployResult, DeployPayload, DeployResult};

// ── Local state (.aomi/deployment.json) ─────────────────────────────────────

/// Local `.aomi/deployment.json`: the backend's [`DeployPayload`] (flattened — the
/// single canonical shape, not a re-declared copy) plus a local activation
/// overlay (`state`, and per-app `AppRecord::activated`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalDeployment {
    pub project_id: i64,
    #[serde(flatten)]
    pub deployment: DeployPayload,
    pub state: LocalDeploymentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activation: Option<Activation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_url: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct LocalDeploymentState {
    pub deployed: bool,
    pub ci_passed: bool,
    pub activated: bool,
}

impl LocalDeployment {
    /// Build local state from a fresh deploy response (deployed=true, nothing
    /// activated yet). The Project remains stable across deployments and owns
    /// the repository/platform binding.
    pub fn from_deploy(resp: DeployResult, project_id: i64) -> Self {
        let mut deployment = resp.deployment;
        for app in &mut deployment.platform.apps {
            app.activated = Some(false);
        }
        Self {
            project_id,
            deployment,
            state: LocalDeploymentState {
                deployed: true,
                ci_passed: false,
                activated: false,
            },
            last_activation: None,
            project_url: None,
        }
    }

    pub fn from_build_deploy(resp: BuildDeployResult) -> Self {
        let project_url = resp.project_url;
        let mut state = Self::from_deploy(
            DeployResult {
                ok: resp.ok,
                deployment: resp.deployment,
            },
            resp.project_id,
        );
        state.project_url = Some(project_url);
        state
    }

    fn path(repo_root: &Path) -> PathBuf {
        repo_root.join(DEPLOYMENT_DIR).join(DEPLOYMENT_FILE)
    }

    /// Read `.aomi/deployment.json` from a repo root, or `None` when absent.
    pub fn read(repo_root: &Path) -> Result<Option<Self>> {
        let path = Self::path(repo_root);
        if !path.exists() {
            return Ok(None);
        }
        let bytes =
            fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .with_context(|| format!("failed to parse {}", path.display()))
    }

    /// Atomically write `.aomi/deployment.json` (temp file + rename so a partial
    /// write is never observable). Returns the file path.
    pub fn write(&self, repo_root: &Path) -> Result<PathBuf> {
        let path = Self::path(repo_root);
        let parent = path.parent().expect("deployment path always has a parent");
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
        let json = serde_json::to_string_pretty(self)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, format!("{json}\n"))
            .with_context(|| format!("failed to write {}", tmp.display()))?;
        fs::rename(&tmp, &path)
            .with_context(|| format!("failed to rename {} -> {}", tmp.display(), path.display()))?;
        Ok(path)
    }

    /// Every app name recorded by the last deploy.
    pub fn app_names(&self) -> Vec<String> {
        self.deployment
            .platform
            .apps
            .iter()
            .map(|a| a.name.clone())
            .collect()
    }

    /// The recorded release tag for an app from the last deploy.
    pub fn release_tag_for(&self, app: &str) -> Option<&str> {
        self.deployment
            .platform
            .apps
            .iter()
            .find(|a| a.name == app)
            .map(|a| a.release_tag.as_str())
    }

    /// Fold a target-based multi-app activation response back into local state:
    /// set each returned app's usable activation flag, mirror the target's CI
    /// outcome into `ci_passed`, then recompute the overall `activated` state.
    pub fn apply_target_activation(&mut self, response: &ActivateResult) {
        for activated in &response.activation.apps {
            for app in self.deployment.platform.apps.iter_mut() {
                if app.name == activated.name {
                    app.activated = Some(
                        activated.is_active
                            && activated.artifact_ready
                            && activated.loaded
                            && activated.error.is_none(),
                    );
                }
            }
        }
        let target_ci_passed = response.activation.target.ci_status.as_deref() == Some("passed");
        let promoted_ci_passed = !response.activation.target.promoted.is_empty()
            && response
                .activation
                .target
                .promoted
                .iter()
                .all(|promotion| promotion.ci_status == "passed");
        // The activation response may contain only the request target. A fully usable app set proves the
        // release artifacts passed the activation gate.
        let all_apps_activated = !response.activation.apps.is_empty()
            && response.activation.apps.iter().all(|app| {
                app.is_active && app.artifact_ready && app.loaded && app.error.is_none()
            });
        if target_ci_passed || promoted_ci_passed || all_apps_activated {
            self.state.ci_passed = true;
        }
        let apps = &self.deployment.platform.apps;
        self.state.activated =
            !apps.is_empty() && apps.iter().all(|a| a.activated.unwrap_or(false));
        self.last_activation = Some(response.activation.clone());
    }
}
