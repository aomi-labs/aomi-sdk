//! `.aomi/deployment.json` — local plan artifact carrying the resolved
//! deployment plus three independent state flags. Per ADR 0009.
//!
//! Lifecycle:
//! - `aomi-git dry-run` writes it from `aomi.toml` + git state (all flags false).
//! - `aomi-git dry-run --preflight` extends `checks[]` with online probes
//!   and fills `platform_resolved.resolved_deploy_branch` from
//!   `GET /api/control/platforms`.
//! - `aomi-git deploy` reads it, executes push + activate, refreshes the file.
//!
//! The file is **always** rewritten in full on each operation; partial writes
//! must not be observable. Callers should write to a temp file and rename.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::git::Source;

pub const DEPLOYMENT_DIR: &str = ".aomi";
pub const DEPLOYMENT_FILE: &str = "deployment.json";

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct DeploymentState {
    pub app: App,
    pub source: Source,
    pub platform: PlatformIntent,
    pub target: TargetSpec,
    pub state: StateFlags,
    #[serde(default)]
    pub checks: Vec<Check>,
    #[serde(default)]
    pub errors: Vec<String>,
    /// Unix timestamp (seconds).
    pub updated_at: i64,
}

impl DeploymentState {
    /// Build a fresh state from offline inputs. All flags false; checks empty.
    /// Preflight extends `checks[]` and may set `platform.resolved_deploy_branch`.
    pub fn new(app: App, source: Source, target: TargetSpec) -> Self {
        let platform = PlatformIntent {
            name: app.platform.clone(),
            github_repo: app.git.clone(),
            resolved_deploy_branch: None,
        };
        Self {
            app,
            source,
            platform,
            target,
            state: StateFlags::default(),
            checks: Vec::new(),
            errors: Vec::new(),
            updated_at: now_seconds(),
        }
    }

    /// Compute `state.deployed` from the target branch vs the platform's
    /// resolved deploy branch. Only meaningful after preflight has filled
    /// `resolved_deploy_branch`.
    pub fn recompute_deployed(&mut self) {
        self.state.deployed = match self.platform.resolved_deploy_branch.as_deref() {
            Some(resolved) => resolved == self.target.branch,
            None => false,
        };
    }

    pub fn touch(&mut self) {
        self.updated_at = now_seconds();
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Default, Deserialize, Serialize)]
pub struct PlatformIntent {
    /// User-declared platform name from aomi.toml. Optional during early
    /// preflight (a user without aomi.toml's `platform` field can't deploy).
    pub name: Option<String>,
    /// User-declared GitHub URL from aomi.toml.
    pub github_repo: Option<String>,
    /// Echo of the platform's `deployment_branch` from
    /// `GET /api/control/platforms`. None until preflight runs.
    pub resolved_deploy_branch: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct TargetSpec {
    /// The branch `aomi-git deploy` will push to. From `aomi.toml`'s
    /// `[app].branch`, defaulting to `publish` for backward compatibility.
    pub branch: String,
    /// Relative path inside the platform repo where the source lands.
    pub app_path: String,
    /// The release tag this deploy will create (`apps-{name}-{shortcommit}`).
    pub release_tag: String,
    /// Required backend server tags for activation/load targeting.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server_tags: Vec<String>,
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct StateFlags {
    /// `git push` to the platform repo succeeded.
    pub pushed: bool,
    /// Push landed on the platform's contractual `deployment_branch`. The
    /// backend will see it.
    pub deployed: bool,
    /// `applications` row written with `is_active = true`.
    pub activated: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct Check {
    pub name: String,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Check {
    pub fn pass(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            detail: Some(detail.into()),
        }
    }

    pub fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            detail: Some(detail.into()),
        }
    }
}

pub fn deployment_path(source_repo_root: &Path) -> PathBuf {
    source_repo_root.join(DEPLOYMENT_DIR).join(DEPLOYMENT_FILE)
}

pub fn write(source_repo_root: &Path, state: &DeploymentState) -> Result<PathBuf> {
    let path = deployment_path(source_repo_root);
    let parent = path
        .parent()
        .expect("deployment path always has a parent dir");
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    let json = serde_json::to_string_pretty(state)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", tmp.display()))?;
    fs::rename(&tmp, &path)
        .with_context(|| format!("failed to rename {} -> {}", tmp.display(), path.display()))?;
    Ok(path)
}

pub fn read(source_repo_root: &Path) -> Result<Option<DeploymentState>> {
    let path = deployment_path(source_repo_root);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let state: DeploymentState = serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(state))
}

fn now_seconds() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
