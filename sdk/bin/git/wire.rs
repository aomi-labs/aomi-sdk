//! The repo-scoped deploy/activate wire contract, in code.
//!
//! These types mirror `docs/platform-ralph/CONTRACTS.md` (product-mono) field
//! for field — they are the canonical Rust side of the contract the TypeScript
//! client also implements. JSON is snake_case to match the backend exactly.
//!
//! Status: the backend endpoints these target (`POST /platforms/:platform/deploy`
//! and the target-based `/apps/activate`) are not reshaped on the server yet, so
//! this is bound to CONTRACTS.md, not to live backend code. When codex lands the
//! repo-scoped endpoints (backlog 4/6) any field-name drift is reconciled here.
//! This is the single canonical home for the deploy/activate contract and the
//! local `.aomi/deployment.json` state.

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

/// Body of `POST /api/admin/platforms/:platform/deploy`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployRequest {
    pub source_ref: SourceRef,
    pub aomi_toml_paths: Vec<String>,
    /// Resolve + validate only; open no PR, write nothing. Omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployResponse {
    pub ok: bool,
    pub deployment: Deployment,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Deployment {
    pub id: String,
    /// `pr_created` | `pr_updated` (and dry-run plan states).
    pub status: String,
    pub source: DeploymentSource,
    pub managed: Managed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeploymentSource {
    pub installation_id: i64,
    pub repository_id: i64,
    pub repository_link: String,
    #[serde(rename = "ref")]
    pub source_ref: SourceRef,
    pub commit_hash: String,
    pub aomi_toml_paths: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Managed {
    pub platform: String,
    pub repository: String,
    pub base_branch: String,
    pub deploy_branch: String,
    pub commit_sha: String,
    pub pr_number: i64,
    pub pr_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_url: Option<String>,
    pub apps: Vec<ManagedApp>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ManagedApp {
    pub name: String,
    pub path: String,
    pub aomi_toml_path: String,
    pub release_tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated: Option<bool>,
}

// ── Activate ───────────────────────────────────────────────────────────────

/// Body of `POST /api/admin/apps/activate` (target-based).
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivateRequest {
    pub platform: String,
    pub target: ActivateTarget,
    pub apps: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_tags: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivateTarget {
    /// `managed_pr` | `managed_branch` | `managed_commit` | `release_tags`.
    pub kind: String,
    pub value: TargetValue,
}

impl ActivateTarget {
    pub fn managed_pr(value: impl Into<String>) -> Self {
        Self {
            kind: "managed_pr".to_string(),
            value: TargetValue::One(value.into()),
        }
    }

    pub fn managed_branch(value: impl Into<String>) -> Self {
        Self {
            kind: "managed_branch".to_string(),
            value: TargetValue::One(value.into()),
        }
    }
}

/// `value` is a single string for PR/branch/commit targets, or a list of
/// release tags for `release_tags`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TargetValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivateResponse {
    pub ok: bool,
    pub activation: Activation,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Activation {
    /// `activated` | `partial_failed` | ...
    pub status: String,
    pub platform: String,
    pub target: ActivationTarget,
    pub apps: Vec<ActivatedApp>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivationTarget {
    pub kind: String,
    pub value: TargetValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_commit_sha: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_url: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivatedApp {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub release_tag: String,
    pub is_active: bool,
    pub loaded: bool,
    /// Present only on per-app failure (partial_failed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ── Local state (.aomi/deployment.json) ─────────────────────────────────────

/// Repo-scoped, multi-app local state written after a deploy and updated by
/// activate/status. Mirrors the deploy response plus per-app/overall activation
/// overlays. Replaces the old single-app `DeploymentState`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeploymentRecord {
    pub source: DeploymentSource,
    pub managed: Managed,
    pub state: LocalState,
}

#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct LocalState {
    pub deployed: bool,
    pub ci_passed: bool,
    pub activated: bool,
}

impl DeploymentRecord {
    /// Build local state from a fresh deploy response (deployed=true, nothing
    /// activated yet).
    pub fn from_deploy(resp: DeployResponse) -> Self {
        let mut d = resp.deployment;
        for app in &mut d.managed.apps {
            app.activated = Some(false);
        }
        Self {
            source: d.source,
            managed: d.managed,
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
        self.managed.apps.iter().map(|a| a.name.clone()).collect()
    }

    /// The default activation target: the managed PR opened by the last deploy.
    pub fn default_target(&self) -> ActivateTarget {
        ActivateTarget::managed_pr(self.managed.pr_url.clone())
    }

    /// Fold an activation response back into local state: flip per-app
    /// `activated` flags and the overall `activated` state.
    pub fn apply_activation(&mut self, activation: &Activation) {
        for app in &mut self.managed.apps {
            if let Some(result) = activation.apps.iter().find(|r| r.name == app.name) {
                app.activated = Some(result.is_active);
            }
        }
        self.state.activated = self
            .managed
            .apps
            .iter()
            .all(|a| a.activated.unwrap_or(false))
            && !self.managed.apps.is_empty();
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}
