//! The repo-scoped deploy/activate wire contract, in code.
//!
//! Aligned to the live backend (`/api/platforms`): deploy is
//! `POST /api/platforms/:platform/deploy` with `{app_source_id, source_ref,
//! aomi_toml_paths, dry_run?}`; activation is target-based and multi-app,
//! `POST /api/platforms/:platform/apps/activate` with `{target, apps,
//! release_tags?, target_tags?}` returning `{ok, activation:{…, apps[]}}` with a
//! per-app partial-failure shape. JSON is snake_case to match the backend. This
//! is the single canonical home for the deploy/activate contract and the local
//! `.aomi/deployment.json` state, mirrored by the TypeScript `@aomi-labs/deploy`
//! client.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEPLOYMENT_DIR: &str = ".aomi";
const DEPLOYMENT_FILE: &str = "deployment.json";

// ── Source ref ─────────────────────────────────────────────────────────────

/// `{ "kind": "branch", "value": "main" }` or `{ "kind": "commit", "value": "<sha>" }`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceRef {
    Branch { value: String },
    Commit { value: String },
}

impl SourceRef {
    pub fn branch(value: impl Into<String>) -> Self {
        SourceRef::Branch {
            value: value.into(),
        }
    }
    pub fn commit(value: impl Into<String>) -> Self {
        SourceRef::Commit {
            value: value.into(),
        }
    }
}

// ── Deploy ─────────────────────────────────────────────────────────────────

/// Body of `POST /api/platforms/:platform/deploy`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployRequest {
    /// The connected GitHub App install (`app_source`) to deploy from.
    pub app_source_id: i64,
    pub source_ref: SourceRef,
    pub aomi_toml_paths: Vec<String>,
    /// Resolve + validate only; open no PR, write nothing. Omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployResponse {
    pub ok: bool,
    pub deployment: DeployPayload,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployPayload {
    pub id: String,
    /// `pr_created` | `pr_updated` (and dry-run plan states).
    pub status: String,
    pub source: Source,
    pub platform: Platform,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub installation_id: i64,
    pub repository_id: i64,
    pub repository_link: String,
    /// Normalized `owner/name` slug from `repository_link`.
    #[serde(default)]
    pub owner_repo_name: String,
    #[serde(rename = "ref")]
    pub source_ref: SourceRef,
    pub commit_hash: String,
    pub aomi_toml_paths: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Platform {
    pub platform: String,
    pub repository: String,
    pub source_branch: String,
    pub deploy_branch: String,
    // Null until the backend's write/PR path lands (it commits + opens the PR).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_url: Option<String>,
    pub apps: Vec<AppRecord>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppRecord {
    pub name: String,
    pub path: String,
    pub aomi_toml_path: String,
    pub release_tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated: Option<bool>,
}

// ── Activate ───────────────────────────────────────────────────────────────
//
// Activation is target-based and multi-app:
// `POST /api/platforms/:platform/apps/activate`. One call resolves a platform
// PR / branch / commit (or explicit release tags), verifies Aomi CI, and
// activates every requested app, returning per-app results with a
// partial-failure shape. Mirrors the backend `ActivateAppsRequest` /
// `ActivateAppsResponse` and the TypeScript `@aomi-labs/deploy` client.

/// `{ "kind": "platform_pr", "value": "<pr url>" }` and friends. `release_tags`
/// carries an array value; the others carry a single string.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TargetRef {
    PlatformPr { value: String },
    PlatformBranch { value: String },
    PlatformCommit { value: String },
    ReleaseTags { value: Vec<String> },
}

/// Body of `POST /api/platforms/:platform/apps/activate`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivateRequest {
    pub target: TargetRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<String>,
    /// Explicit release tags — required for `platform_commit` targets, ignored
    /// for `platform_pr` / `platform_branch` (the backend derives those).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub release_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivateResponse {
    pub ok: bool,
    pub activation: ActivationPayload,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivationPayload {
    /// `activated` | `partial_failed`.
    pub status: String,
    pub platform: String,
    pub target: ActivationTarget,
    pub apps: Vec<ActivatedApp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivationTarget {
    pub kind: String,
    /// String for PR/branch/commit, array for release tags.
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_commit_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promoted: Vec<ActivationPromotion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivationPromotion {
    pub name: String,
    pub release_tag: String,
    pub source_branch: String,
    pub platform_commit_hash: String,
    pub ci_status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub release_assets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivatedApp {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_tag: Option<String>,
    pub is_active: bool,
    pub loaded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ── Local state (.aomi/deployment.json) ─────────────────────────────────────

/// Local `.aomi/deployment.json`: the backend's [`Deployment`] (flattened — the
/// single canonical shape, not a re-declared copy) plus a local activation
/// overlay (`state`, and per-app `AppRecord::activated`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalRecord {
    #[serde(flatten)]
    pub deployment: DeployPayload,
    pub state: LocalState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activation: Option<ActivationPayload>,
}

#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct LocalState {
    pub deployed: bool,
    pub ci_passed: bool,
    pub activated: bool,
}

impl LocalRecord {
    /// Build local state from a fresh deploy response (deployed=true, nothing
    /// activated yet).
    pub fn from_deploy(resp: DeployResponse) -> Self {
        let mut deployment = resp.deployment;
        for app in &mut deployment.platform.apps {
            app.activated = Some(false);
        }
        Self {
            deployment,
            state: LocalState {
                deployed: true,
                ci_passed: false,
                activated: false,
            },
            last_activation: None,
        }
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
    pub fn apply_target_activation(&mut self, response: &ActivateResponse) {
        for activated in &response.activation.apps {
            for app in self.deployment.platform.apps.iter_mut() {
                if app.name == activated.name {
                    app.activated =
                        Some(activated.is_active && activated.loaded && activated.error.is_none());
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
        if target_ci_passed || promoted_ci_passed {
            self.state.ci_passed = true;
        }
        let apps = &self.deployment.platform.apps;
        self.state.activated =
            !apps.is_empty() && apps.iter().all(|a| a.activated.unwrap_or(false));
        self.last_activation = Some(response.activation.clone());
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}
