//! The repo-scoped deploy/activate wire contract, in code.
//!
//! Aligned to the live backend (`/api/platforms`): deploy is
//! `POST /api/platforms/:platform/deploy` with `{app_source_id, source_ref,
//! aomi_toml_paths, dry_run?}`; activation is single-app,
//! `POST /api/platforms/:platform/apps/:app/activate` with `{app_release_tag,
//! target_tags?}` returning a flat row. JSON is snake_case to match the backend.
//! This is the single canonical home for the deploy/activate contract and the
//! local `.aomi/deployment.json` state, mirrored by the TypeScript
//! `@aomi-labs/deploy` client.

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
// Activation is single-app: `POST /api/platforms/:platform/apps/:app/activate`
// (platform + app come from the URL path). The body carries the release tag and
// optional narrowing tags. Activate several apps by calling once per app.

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivateRequest {
    pub app_release_tag: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_tags: Vec<String>,
}

/// Flat single-app response from the activate endpoint.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivateResponse {
    pub id: i64,
    pub name: String,
    pub label: String,
    pub platform: String,
    pub is_active: bool,
    pub is_public: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_source_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_release_tag: Option<String>,
    /// `activated`.
    pub status: String,
}

// ── Local state (.aomi/deployment.json) ─────────────────────────────────────

/// Local `.aomi/deployment.json`: the backend's [`Deployment`] (flattened — the
/// single canonical shape, not a re-declared copy) plus a local activation
/// overlay (`state`, and per-app `ManagedApp::activated`).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct LocalRecord {
    #[serde(flatten)]
    pub deployment: DeployPayload,
    pub state: LocalState,
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

    /// Fold one single-app activation response back into local state: set that
    /// app's `activated` flag, then recompute the overall `activated` state.
    pub fn apply_activation(&mut self, response: &ActivateResponse) {
        let apps = &mut self.deployment.platform.apps;
        for app in apps.iter_mut() {
            if app.name == response.name {
                app.activated = Some(response.is_active);
            }
        }
        self.state.activated =
            !apps.is_empty() && apps.iter().all(|a| a.activated.unwrap_or(false));
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}
