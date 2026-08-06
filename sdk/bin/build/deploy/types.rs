//! Repo-scoped wire types accepting backend snake_case and Builder camelCase.
//!
//! The local `.aomi/deployment.json` record built from these lives in `state`.

use std::collections::BTreeMap;

use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

// ── Deploy ─────────────────────────────────────────────────────────────────

/// Mirrors TypeScript `DeployStatus`; the backend may add more status strings.
pub type DeployStatus = String;

/// Mirrors TypeScript `CiStatus`; the backend may add more status strings.
pub type CiStatus = String;

/// Body of `POST /api/projects/:project_id/deploy`.
#[cfg(test)]
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployInput {
    /// Resolved immutable source commit SHA. Branches are resolved before this request.
    pub source_ref: String,
    /// Preview the deployment plan; may materialize backend source metadata but opens no PR.
    #[serde(default, skip_serializing_if = "is_false")]
    pub preflight: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployResult {
    pub ok: bool,
    pub deployment: DeployPayload,
}

/// Aomi Build BFF request. Browser and CLI deployments share this
/// Builder-authenticated surface; the BFF derives ownership from the session.
/// Manager keys deploys by `projectId`; preflight resolves it from `repo`
/// when absent.
#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildDeployInput {
    pub repo: String,
    pub source_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<i64>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildDeployResult {
    pub ok: bool,
    pub project_id: i64,
    pub deployment: DeployPayload,
    pub project_url: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct DeployPayload {
    pub id: String,
    /// `preflight` | `pr_created` | `pr_updated` | `unchanged`.
    pub status: DeployStatus,
    #[serde(default, alias = "sdkVersion", skip_serializing_if = "Option::is_none")]
    pub sdk_version: Option<String>,
    pub source: Source,
    pub platform: Platform,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Source {
    #[serde(alias = "installationId")]
    pub installation_id: i64,
    #[serde(alias = "repositoryId")]
    pub repository_id: i64,
    #[serde(alias = "repositoryLink")]
    pub repository_link: String,
    /// Normalized `owner/name` slug from `repository_link`.
    #[serde(default, alias = "ownerRepoName")]
    pub owner_repo_name: String,
    #[serde(rename = "ref")]
    pub source_ref: String,
    #[serde(alias = "commitHash")]
    pub commit_hash: String,
    #[serde(default, alias = "aomiTomlPaths")]
    pub aomi_toml_paths: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Platform {
    pub platform: String,
    pub repository: String,
    #[serde(alias = "platformBranch")]
    pub platform_branch: String,
    #[serde(alias = "deployBranch")]
    pub deploy_branch: String,
    // Null until the backend's write/PR path lands (it commits + opens the PR).
    #[serde(default, alias = "commitHash", skip_serializing_if = "Option::is_none")]
    pub commit_hash: Option<String>,
    #[serde(default, alias = "prNumber", skip_serializing_if = "Option::is_none")]
    pub pr_number: Option<i64>,
    #[serde(default, alias = "prUrl", skip_serializing_if = "Option::is_none")]
    pub pr_url: Option<String>,
    #[serde(default, alias = "ciStatus", skip_serializing_if = "Option::is_none")]
    pub ci_status: Option<CiStatus>,
    #[serde(default, alias = "ciUrl", skip_serializing_if = "Option::is_none")]
    pub ci_url: Option<String>,
    pub apps: Vec<AppRecord>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppRecord {
    pub name: String,
    pub path: String,
    #[serde(alias = "aomiTomlPath")]
    pub aomi_toml_path: String,
    #[serde(alias = "releaseTag")]
    pub release_tag: String,
    #[serde(default, alias = "sdkVersion", skip_serializing_if = "Option::is_none")]
    pub sdk_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Local `.aomi/deployment.json` overlay; absent from backend deploy result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated: Option<bool>,
}

// Release-tag activation request and response.

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

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildActivateInput {
    pub platform: String,
    pub project_id: i64,
    pub release_tags: Vec<String>,
    pub apps: Vec<String>,
    /// Backend server tags from `--target-tag`. Omitted entirely when unused so
    /// the common request body is byte-for-byte what it has always been; when
    /// the user does pass the flag it reaches the BFF instead of being dropped.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub target_tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Activation {
    /// `activating` | `partial_failed`.
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
    #[serde(alias = "platformRepo", skip_serializing_if = "Option::is_none")]
    pub platform_repo: Option<String>,
    #[serde(alias = "platformBranch", skip_serializing_if = "Option::is_none")]
    pub platform_branch: Option<String>,
    #[serde(alias = "platformCommitHash", skip_serializing_if = "Option::is_none")]
    pub platform_commit_hash: Option<String>,
    #[serde(alias = "ciStatus", skip_serializing_if = "Option::is_none")]
    pub ci_status: Option<CiStatus>,
    #[serde(alias = "ciUrl", skip_serializing_if = "Option::is_none")]
    pub ci_url: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub promoted: Vec<ActivationPromotion>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivationPromotion {
    pub name: String,
    #[serde(alias = "releaseTag")]
    pub release_tag: String,
    #[serde(default, alias = "platformBranch")]
    pub platform_branch: String,
    #[serde(alias = "platformCommitHash", skip_serializing_if = "Option::is_none")]
    pub platform_commit_hash: Option<String>,
    #[serde(alias = "activatedCommitHash", skip_serializing_if = "Option::is_none")]
    pub activated_commit_hash: Option<String>,
    #[serde(alias = "liveCommitHash", skip_serializing_if = "Option::is_none")]
    pub live_commit_hash: Option<String>,
    #[serde(alias = "ciStatus")]
    pub ci_status: CiStatus,
    #[serde(alias = "ciUrl", skip_serializing_if = "Option::is_none")]
    pub ci_url: Option<String>,
    #[serde(alias = "releaseAssets")]
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub release_assets: Vec<String>,
    #[serde(alias = "releaseAssetDigests")]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub release_asset_digests: BTreeMap<String, String>,
    #[serde(alias = "activationStatus", skip_serializing_if = "Option::is_none")]
    pub activation_status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivatedApp {
    #[serde(alias = "applicationId", skip_serializing_if = "Option::is_none")]
    pub application_id: Option<i64>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(alias = "releaseTag", skip_serializing_if = "Option::is_none")]
    pub release_tag: Option<String>,
    #[serde(alias = "isActive")]
    pub is_active: bool,
    // No `default`: a missing field used to deserialize to `false`, which
    // `print_activation` reports as a failed activation. Absent and "not ready"
    // are different problems, so an omitted field is now a parse error like its
    // `is_active` / `loaded` siblings.
    #[serde(default, alias = "artifactReady")]
    pub artifact_ready: bool,
    pub loaded: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(alias = "platformBranch", skip_serializing_if = "Option::is_none")]
    pub platform_branch: Option<String>,
    #[serde(alias = "liveCommitHash", skip_serializing_if = "Option::is_none")]
    pub live_commit_hash: Option<String>,
    #[serde(alias = "activationStatus", skip_serializing_if = "Option::is_none")]
    pub activation_status: Option<String>,
}

// Platform-token and source bootstrap types.

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

/// Body of `POST /api/platforms/:platform/projects`.
#[derive(Debug, Clone, Serialize)]
pub struct CreateProjectInput {
    pub repo: String,
    /// Verified Builder identity established by `aomi-build login`. The
    /// backend verifies installation ownership before recording the project.
    pub github_user_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct Project {
    pub id: i64,
    pub installation_id: i64,
    pub repository_id: i64,
    pub repository_link: String,
    #[serde(default)]
    pub github_account: Option<String>,
    #[serde(default)]
    pub platform_id: Option<i64>,
}

/// Response of `GET /api/integrations/github-app/oauth/start` — the GitHub App
/// install URL the user opens to connect. Mirrors the portal's response shape
/// (`{ ok, install_url }`); the CLI only needs `install_url`. GitHub returns the
/// resolved `installation_id` to the App's configured redirect, not to the CLI,
/// so there is no result/poll endpoint — the user reads it from that redirect.
#[derive(Debug, Clone, Deserialize)]
pub struct OAuthStart {
    pub install_url: String,
}

/// Minimal projection of `GET /api/platforms/:platform/deployments/:id/status`
/// — enough to gate activation on the release build, matching the portal's
/// poll. `state` is one of `no_ci` | `pending` | `building` | `releasing` |
/// `ready` | `failed`.
#[derive(Debug, Clone, Deserialize)]
pub struct DeploymentStatusResult {
    pub state: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub ci: Option<DeploymentCiStatus>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DeploymentCiStatus {
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CliExchangeInput {
    pub code: String,
    pub code_verifier: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliExchangeResult {
    #[serde(alias = "access_token")]
    pub access_token: String,
    #[serde(alias = "token_type")]
    pub token_type: String,
    #[serde(alias = "expires_in")]
    pub expires_in: i64,
    #[serde(alias = "github_login")]
    pub github_login: String,
    #[serde(alias = "github_user_id")]
    pub github_user_id: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CliStatusResult {
    #[serde(alias = "signed_in")]
    pub signed_in: bool,
    #[serde(alias = "github_login")]
    pub github_login: String,
    #[serde(alias = "github_user_id")]
    pub github_user_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectResult {
    #[serde(default)]
    #[allow(dead_code)]
    pub ok: bool,
    pub project: Project,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformAppResult {
    pub app: PlatformAppStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlatformAppStatus {
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub artifact_ready: bool,
    #[serde(default)]
    pub loaded: bool,
    #[serde(default)]
    pub app_release_tag: Option<String>,
}

/// `skip_serializing_if` for the test-only preflight flag.
#[cfg(test)]
fn is_false(b: &bool) -> bool {
    !*b
}
