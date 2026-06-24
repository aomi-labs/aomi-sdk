//! The repo-scoped deploy/activate wire contract, in code.
//!
//! Aligned to the live backend (`/api/platforms`): deploy is
//! `POST /api/platforms/:platform/deploy` with `{app_source_id, source_ref,
//! aomi_toml_paths, dry_run?}`; activation is release-tags based,
//! `POST /api/platforms/:platform/apps/activate` with `{target, apps?,
//! target_tags?}` returning `{ok, activation:{…, apps[]}}` with a
//! per-app partial-failure shape. JSON is snake_case to match the backend. Type
//! names intentionally mirror the backend deploy payload where the concept is
//! shared; local-only persistence types keep a `Local*` prefix.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

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

/// Mirrors TypeScript `DeployStatus`; the backend may add more status strings.
pub type DeployStatus = String;

/// Mirrors TypeScript `CiStatus`; the backend may add more status strings.
pub type CiStatus = String;

/// Body of `POST /api/platforms/:platform/deploy`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployInput {
    /// The connected GitHub App install (`app_source`) to deploy from.
    pub app_source_id: i64,
    pub source_ref: SourceRef,
    pub aomi_toml_paths: Vec<String>,
    /// Resolve + validate only; open no PR, write nothing. Omitted when false.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployResult {
    pub ok: bool,
    pub deployment: DeployPayload,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployPayload {
    pub id: String,
    /// `pr_created` | `pr_updated` (and dry-run plan states).
    pub status: DeployStatus,
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
    /// The connected GitHub App install (`app_source`) this deploy ran from.
    /// Absent from the backend deploy response; the CLI records the id it sent
    /// so re-deploys and `activate` can auto-resolve it instead of re-asking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_source_id: Option<i64>,
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
    pub ci_status: Option<CiStatus>,
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
    pub target: Option<String>,
    /// Local `.aomi/deployment.json` overlay; absent from backend deploy result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated: Option<bool>,
}

// ── Activate ───────────────────────────────────────────────────────────────
//
// Activation is release-tags based:
// `POST /api/platforms/:platform/apps/activate`. One call promotes the
// requested release candidates to the platform live branch, then activates each
// requested app, returning per-app results with a partial-failure shape. Mirrors
// the backend `ActivateAppsRequest` / `ActivateAppsResponse`.

#[derive(Debug, Clone, Eq, PartialEq, Deserialize)]
pub struct ReleaseTags {
    pub value: Vec<String>,
}

impl ReleaseTags {
    pub fn new(value: Vec<String>) -> Self {
        Self { value }
    }
}

impl Serialize for ReleaseTags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut target = serializer.serialize_struct("ReleaseTags", 2)?;
        target.serialize_field("kind", "release_tags")?;
        target.serialize_field("value", &self.value)?;
        target.end()
    }
}

/// Body of `POST /api/platforms/:platform/apps/activate`.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct ActivateInput {
    pub target: ReleaseTags,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apps: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivateResult {
    pub ok: bool,
    pub activation: Activation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activation {
    /// `activated` | `partial_failed`.
    pub status: String,
    pub platform: String,
    pub target: ActivationTarget,
    pub apps: Vec<ActivatedApp>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivationTarget {
    pub kind: String,
    /// Array for `release_tags`.
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform_commit_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ci_status: Option<CiStatus>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_commit_hash: Option<String>,
    pub ci_status: CiStatus,
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

// ── Bootstrap: tokens, sources, scaffold ────────────────────────────────────
//
// The platform-token + source-resolution surface that precedes deploy. These
// mirror the backend's `/api/platforms/:platform/*` bodies (token mint, source
// sync/resolve) and `/api/integrations/github-app/.../create-from-template`.

/// Body of `POST /api/platforms/:platform/tokens`.
#[derive(Debug, Clone, Serialize)]
pub struct MintTokenInput {
    /// `platform` | `app`.
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<i64>,
}

/// Response of a token mint — the plaintext `token` is returned exactly once.
#[derive(Debug, Clone, Deserialize)]
pub struct MintTokenResult {
    pub id: i64,
    pub token: String,
    pub scope: String,
}

/// Body of `POST /api/platforms/:platform/sources/sync-installed`.
#[derive(Debug, Clone, Serialize)]
pub struct SyncSourceInput {
    pub repo: String,
}

/// Body of `POST /api/integrations/github-app/platforms/:platform/sources/create-from-template`.
#[derive(Debug, Clone, Serialize)]
pub struct CreateTemplateInput {
    pub installation_id: i64,
    pub template_repo: String,
    pub repo_name: String,
    #[serde(skip_serializing_if = "is_false")]
    pub private: bool,
}

/// A connected GitHub App source row — the `source` payload returned by
/// sync-installed / resolve / create-from-template. Its `id` is the
/// `app_source_id` deploy needs. Fields mirror the backend response; not all
/// are consumed by the CLI today.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct AppSource {
    pub id: i64,
    pub installation_id: i64,
    pub repository_id: i64,
    pub repository_link: String,
    #[serde(default)]
    pub github_account: Option<String>,
    #[serde(default)]
    pub github_user_id: Option<i64>,
    #[serde(default)]
    pub bound_platform_id: Option<i64>,
}

/// Response of `GET /api/integrations/github-app/oauth/start` — the GitHub App
/// install URL the user opens to connect. `state` (the poll handle) is embedded
/// in the URL today; the field is here for when the backend returns it directly.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthStart {
    pub install_url: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub state: Option<String>,
}

/// Response of `GET /api/integrations/github-app/oauth/result?state=…` — the
/// poll the CLI runs after the user installs the App in the browser.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthResult {
    pub status: String,
    #[serde(default)]
    pub installation_id: Option<i64>,
}

/// Envelope around a single source row (`{ ok, source }`).
#[derive(Debug, Clone, Deserialize)]
pub struct SourceResult {
    #[serde(default)]
    #[allow(dead_code)]
    pub ok: bool,
    pub source: AppSource,
}

// ── Local state (.aomi/deployment.json) ─────────────────────────────────────

/// Local `.aomi/deployment.json`: the backend's [`DeployPayload`] (flattened — the
/// single canonical shape, not a re-declared copy) plus a local activation
/// overlay (`state`, and per-app `AppRecord::activated`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalDeployment {
    #[serde(flatten)]
    pub deployment: DeployPayload,
    pub state: LocalDeploymentState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_activation: Option<Activation>,
}

#[derive(Debug, Clone, Eq, PartialEq, Default, Serialize, Deserialize)]
pub struct LocalDeploymentState {
    pub deployed: bool,
    pub ci_passed: bool,
    pub activated: bool,
}

impl LocalDeployment {
    /// Build local state from a fresh deploy response (deployed=true, nothing
    /// activated yet). `app_source_id` is the connected source the CLI deployed
    /// from; the backend omits it from the response, so we record it here for
    /// later re-deploys / `activate` to auto-resolve.
    pub fn from_deploy(resp: DeployResult, app_source_id: i64) -> Self {
        let mut deployment = resp.deployment;
        deployment.source.app_source_id = Some(app_source_id);
        for app in &mut deployment.platform.apps {
            app.activated = Some(false);
        }
        Self {
            deployment,
            state: LocalDeploymentState {
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

    /// The connected source id recorded by the last deploy, if any.
    pub fn app_source_id(&self) -> Option<i64> {
        self.deployment.source.app_source_id
    }

    /// Record the connected source id (used by `source sync` / `scaffold` to
    /// patch an existing deployment record so the next deploy auto-resolves it).
    pub fn set_app_source_id(&mut self, app_source_id: i64) {
        self.deployment.source.app_source_id = Some(app_source_id);
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
