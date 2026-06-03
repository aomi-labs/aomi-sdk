//! `.aomi/deployment.json` - local plan artifact carrying the resolved
//! deployment plus three independent state flags. Per ADR 0009.
//!
//! Lifecycle:
//! - `aomi-git dry-run` writes it from `aomi.toml` + git state (all flags
//!   false): the `workspace` + `manifest` stages of the validation pipeline.
//! - `aomi-git dry-run` with a reachable backend fills the `platform` +
//!   `backend` stages with online probes and sets
//!   `platform.resolved_deploy_branch` from `GET /api/control/platforms`.
//! - `aomi-git deploy` reads it, executes push + activate, refreshes the file.
//!
//! The file is **always** rewritten in full on each operation; partial writes
//! must not be observable. Callers should write to a temp file and rename.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::app::App;
use crate::git::Source;

pub const DEPLOYMENT_DIR: &str = ".aomi";
pub const DEPLOYMENT_FILE: &str = "deployment.json";

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DeploymentState {
    pub app: App,
    pub source: Source,
    pub platform: PlatformIntent,
    pub target: TargetSpec,
    /// The app's declared `aomi-sdk` version, parsed from its `Cargo.toml`.
    /// Compared against the backend host's SDK version at preflight to catch a
    /// mismatch before deploy (the bundle would otherwise be rejected at
    /// activation). `None` when it can't be read or parsed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_sdk_version: Option<String>,
    pub state: StateFlags,
    #[serde(default)]
    pub files: Vec<StagedFile>,
    /// The validation pipeline, grouped into ordered stages. Each stage is a
    /// precondition for the next; a failed gate short-circuits the rest
    /// (downstream stages are recorded as `skipped`). See `StageId`.
    #[serde(default)]
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub errors: Vec<String>,
    /// Unix timestamp (seconds).
    pub updated_at: i64,
}

impl DeploymentState {
    /// Build a fresh state from offline inputs. All flags false; stages empty.
    /// `plan::to_state` populates the workspace + manifest stages, and preflight
    /// fills the platform + backend stages and may set
    /// `platform.resolved_deploy_branch`.
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
            app_sdk_version: None,
            state: StateFlags::default(),
            files: Vec::new(),
            stages: Vec::new(),
            errors: Vec::new(),
            updated_at: now_seconds(),
        }
    }

    /// Replace the stage with the same `StageId` (or append it if absent).
    /// Preflight uses this to upgrade the `platform`/`backend` placeholders
    /// that `to_state` seeded as `skipped`.
    pub fn replace_stage(&mut self, stage: Stage) {
        if let Some(slot) = self.stages.iter_mut().find(|s| s.stage == stage.stage) {
            *slot = stage;
        } else {
            self.stages.push(stage);
        }
    }

    /// Compute `state.deployed` from `state.pushed` AND the target branch vs
    /// the platform's resolved deploy branch.
    ///
    /// `deployed` means "the push landed on the contractual deployment branch
    /// and the backend will see it" - it is a strict subset of `pushed`. A
    /// dry-run or `--platform-dir` run that didn't push must leave `deployed`
    /// false even when the branch contract resolves cleanly.
    pub fn recompute_deployed(&mut self) {
        self.state.deployed = self.state.pushed
            && matches!(
                self.platform.resolved_deploy_branch.as_deref(),
                Some(resolved) if resolved == self.target.branch,
            );
    }

    pub fn touch(&mut self) {
        self.updated_at = now_seconds();
    }

    /// Render the validation pipeline as a compact, human-readable summary -
    /// one line per stage, with failed/advisory check details indented beneath.
    pub fn render_preflight(&self) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        out.push_str("Preflight\n");
        for stage in &self.stages {
            let tag = match stage.status {
                StageStatus::Passed => "[ok]",
                StageStatus::Warning => "[warn]",
                StageStatus::Failed => "[fail]",
                StageStatus::Skipped => "[skip]",
            };
            let _ = write!(out, "  {tag:<6} {:<9}", stage.stage.label());

            if stage.checks.is_empty() {
                if let Some(detail) = &stage.detail {
                    let _ = write!(out, " (skipped: {detail})");
                }
            } else {
                let names: Vec<&str> = stage.checks.iter().map(|c| c.name.as_str()).collect();
                let _ = write!(out, " {}", names.join(", "));
            }

            if !stage.resolved.is_empty() {
                let pairs: Vec<String> = stage
                    .resolved
                    .iter()
                    .map(|(k, v)| format!("{k}={}", render_value(v)))
                    .collect();
                let _ = write!(out, "  |  {}", pairs.join(" "));
            }
            out.push('\n');

            // Surface the detail of anything that didn't pass.
            for check in stage.checks.iter().filter(|c| !c.passed) {
                let mark = match check.severity {
                    Severity::Error => "[error]",
                    Severity::Warn => "[warn]",
                };
                if let Some(detail) = &check.detail {
                    let _ = writeln!(out, "         {mark} {}: {detail}", check.name);
                }
            }
        }
        out
    }
}

/// Compact one-line rendering of a resolved JSON value for the preflight
/// summary (`["a","b"]` -> `[a,b]`, strings unquoted).
fn render_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => {
            let rendered: Vec<String> = items.iter().map(render_value).collect();
            format!("[{}]", rendered.join(","))
        }
        other => other.to_string(),
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

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct StagedFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
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

/// How much weight a failing check carries. `Error` checks gate the deploy -
/// a failure fails the whole stage. `Warn` checks are advisory (e.g. a
/// fork-tolerant URL mismatch); a failure downgrades the stage to `Warning`
/// but does not block.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    #[default]
    Error,
    Warn,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct Check {
    pub name: String,
    pub passed: bool,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl Check {
    /// A passing gate check.
    pub fn pass(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: true,
            severity: Severity::Error,
            detail: Some(detail.into()),
        }
    }

    /// A failing gate check - fails the stage it belongs to.
    pub fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed: false,
            severity: Severity::Error,
            detail: Some(detail.into()),
        }
    }

    /// An advisory check. A failure downgrades its stage to `Warning` rather
    /// than `Failed`. Pass `passed` for the actual outcome.
    pub fn warn(name: impl Into<String>, passed: bool, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            passed,
            severity: Severity::Warn,
            detail: Some(detail.into()),
        }
    }
}

/// The ordered stages of the deploy validation pipeline. Each stage is a
/// precondition for the next:
/// 1. `Workspace` - is the local tree shippable (clean git).
/// 2. `Manifest`  - does `aomi.toml` declare what we need.
/// 3. `Platform`  - resolve the declared platform repo + deploy branch.
/// 4. `Backend`   - will the backend actually accept this (server tags, DB).
///
/// Stages 1-2 are offline (filled by `plan::to_state`); 3-4 are online
/// (filled by `preflight::run`).
#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageId {
    Workspace,
    Manifest,
    Platform,
    Backend,
}

impl StageId {
    pub fn label(self) -> &'static str {
        match self {
            StageId::Workspace => "workspace",
            StageId::Manifest => "manifest",
            StageId::Platform => "platform",
            StageId::Backend => "backend",
        }
    }
}

/// Rolled-up outcome of a stage, derived from its checks.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StageStatus {
    /// All checks passed.
    Passed,
    /// At least one `Error`-severity check failed - deploy is blocked here.
    Failed,
    /// Only `Warn`-severity checks failed - advisory, not blocking.
    Warning,
    /// Stage did not run (an upstream gate failed, or its inputs were absent).
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Stage {
    pub stage: StageId,
    pub status: StageStatus,
    /// Why a stage was skipped, or another short note. Omitted when empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<Check>,
    /// Facts this stage established (e.g. the resolved repo + branch, or the
    /// effective server_tags). Distinct from `checks` - these are outputs, not
    /// pass/fail assertions.
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub resolved: Map<String, Value>,
}

impl Stage {
    /// Build a stage from its checks, deriving `status` from their outcomes.
    pub fn new(stage: StageId, checks: Vec<Check>) -> Self {
        let status = Self::derive_status(&checks);
        Self {
            stage,
            status,
            detail: None,
            checks,
            resolved: Map::new(),
        }
    }

    /// A stage that did not run, with a human-readable reason.
    pub fn skipped(stage: StageId, reason: impl Into<String>) -> Self {
        Self {
            stage,
            status: StageStatus::Skipped,
            detail: Some(reason.into()),
            checks: Vec::new(),
            resolved: Map::new(),
        }
    }

    /// Attach a resolved fact (builder style).
    pub fn with_resolved(mut self, key: &str, value: Value) -> Self {
        self.resolved.insert(key.to_string(), value);
        self
    }

    /// Derive a stage's rolled-up status from its checks. Order-independent:
    /// any failed `Error` check means `Failed`; otherwise any failed `Warn`
    /// means `Warning`; otherwise `Passed`.
    fn derive_status(checks: &[Check]) -> StageStatus {
        let mut status = StageStatus::Passed;
        for check in checks {
            if !check.passed {
                match check.severity {
                    Severity::Error => return StageStatus::Failed,
                    Severity::Warn => status = StageStatus::Warning,
                }
            }
        }
        status
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
