//! How a deploy step reads its inputs from the working tree.
//!
//! Everything here answers "what exactly are we deploying?" from git and
//! `aomi.toml` — the source commit, the manifest set, the destination platform,
//! and the platform-bound Project id — with no network involved. Split from the
//! lifecycle in `deploy.rs`, which consumes the answers.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use super::DeployStepArgs;
use super::shared::{PROJECT_ID_ENV, env_value, head_commit, remote_origin};
use crate::deploy::app::AomiAppFiles;
use crate::deploy::config::AomiConfig;
use crate::deploy::platform::{Platform, normalize_github_repo};
use crate::deploy::state::LocalDeployment;
use crate::deploy::types::BuildDeployInput;

const PROJECT_CONFIG_PATH: &str = ".aomi/config.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectConfig {
    version: u8,
    applications: Vec<String>,
}

/// Where a deploy's destination platform came from, for the summary card.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum PlatformOrigin {
    Flag,
    Manifest,
    SavedConfig,
    Default,
}

impl PlatformOrigin {
    pub(crate) fn describe(self) -> &'static str {
        match self {
            Self::Flag => "requested",
            Self::Manifest => "from aomi.toml",
            Self::SavedConfig => "from saved config",
            Self::Default => "default",
        }
    }
}

impl DeployStepArgs {
    pub(super) fn build_request(
        &self,
        git_root: &Path,
        platform: &Platform,
    ) -> Result<BuildDeployInput> {
        let repo = match self.repo.as_deref() {
            Some(repo) => normalize_github_repo(repo)?,
            None => normalize_github_repo(&remote_origin(git_root)?)?,
        };
        self.project_applications(git_root)?;
        Ok(BuildDeployInput {
            platform: platform.to_string(),
            source_ref: self.source_ref(git_root)?,
            project_id: self.resolve_project_id(git_root, &repo),
            repo,
        })
    }

    /// Documented precedence: `--platform` flag, then the platform declared by
    /// the deployed `aomi.toml` manifests, then saved config, then `community`.
    #[cfg(test)]
    pub(crate) fn platform(&self, git_root: &Path, start_dir: &Path) -> Result<Platform> {
        self.resolve_platform(git_root, start_dir, AomiConfig::load().platform)
    }

    /// `platform` plus where the answer came from, for the deploy summary.
    pub(crate) fn platform_with_origin(
        &self,
        git_root: &Path,
        start_dir: &Path,
    ) -> Result<(Platform, PlatformOrigin)> {
        self.resolve_platform_with_origin(git_root, start_dir, AomiConfig::load().platform)
    }

    /// `platform` with the saved-config value injected so tests don't depend
    /// on the machine's `~/.config/aomi/config.toml`.
    #[cfg(test)]
    pub(crate) fn resolve_platform(
        &self,
        git_root: &Path,
        start_dir: &Path,
        saved_platform: Option<String>,
    ) -> Result<Platform> {
        Ok(self
            .resolve_platform_with_origin(git_root, start_dir, saved_platform)?
            .0)
    }

    pub(crate) fn resolve_platform_with_origin(
        &self,
        git_root: &Path,
        start_dir: &Path,
        saved_platform: Option<String>,
    ) -> Result<(Platform, PlatformOrigin)> {
        if let Some(p) = &self.platform {
            return Ok((p.clone(), PlatformOrigin::Flag));
        }
        if let Some(platform) = self.manifest_platform(git_root)? {
            return Ok((platform, PlatformOrigin::Manifest));
        }
        if let Some(platform) = AomiAppFiles::discover(start_dir, git_root)
            .ok()
            .and_then(|a| a.platform)
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
        {
            return Ok((Platform::new(platform), PlatformOrigin::Manifest));
        }
        Ok(match saved_platform {
            Some(saved) => (Platform::new(saved), PlatformOrigin::SavedConfig),
            None => (Platform::community(), PlatformOrigin::Default),
        })
    }

    /// Platform declared by the manifests in the root Project configuration.
    /// `None` when no manifest in the set declares one; an error when the set
    /// disagrees, since one deploy targets exactly one platform.
    pub(crate) fn manifest_platform(&self, git_root: &Path) -> Result<Option<Platform>> {
        let Ok(paths) = self.project_applications(git_root) else {
            // No deployable manifest set (e.g. nothing tracked yet); the deploy
            // steps surface that error where it matters.
            return Ok(None);
        };
        let mut declared: Vec<(String, Platform)> = Vec::new();
        for path in paths {
            let Some(platform) = AomiAppFiles::from_aomi_toml(&git_root.join(&path), git_root)
                .ok()
                .and_then(|app| app.platform)
                .map(Platform::new)
            else {
                continue;
            };
            if !declared.iter().any(|(_, seen)| *seen == platform) {
                declared.push((path, platform));
            }
        }
        if declared.len() > 1 {
            let listing = declared
                .iter()
                .map(|(path, platform)| format!("  {path} -> {platform}"))
                .collect::<Vec<_>>()
                .join("\n");
            bail!(
                "the aomi.toml manifests in this deploy declare conflicting platforms:\n{listing}\n\
                 A Project targets one platform; align `[app].platform` or pass --platform."
            );
        }
        Ok(declared.pop().map(|(_, platform)| platform))
    }

    pub(crate) fn source_ref(&self, git_root: &Path) -> Result<String> {
        if let Some(commit) = self
            .commit
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            return validate_source_commit(commit);
        }
        validate_source_commit(&head_commit(git_root)?)
    }

    pub(crate) fn project_applications(&self, git_root: &Path) -> Result<Vec<String>> {
        let path = git_root.join(PROJECT_CONFIG_PATH);
        let bytes =
            fs::read(&path).with_context(|| format!("Project requires {}", path.display()))?;
        let config: ProjectConfig = serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid {}", path.display()))?;
        if config.version != 1 {
            bail!(
                "unsupported {PROJECT_CONFIG_PATH} version {}; expected 1",
                config.version
            );
        }
        let mut seen = HashSet::new();
        let mut applications = Vec::with_capacity(config.applications.len());
        for value in config.applications {
            let path = normalize_project_path(&value)?;
            if !seen.insert(path.clone()) {
                bail!("{PROJECT_CONFIG_PATH} contains duplicate application `{path}`");
            }
            if !git_root.join(&path).is_file() {
                bail!("{PROJECT_CONFIG_PATH} references missing `{path}`");
            }
            applications.push(path);
        }
        Ok(applications)
    }

    /// Resolution order: flag → env → the id recorded by a prior deploy or
    /// `project create` in `.aomi/deployment.json`.
    ///
    /// `repo` is the source the caller actually asked to deploy. The backend
    /// resolves the repo from the Project, so a recorded id belonging to a
    /// different repo would silently win over that request — making the wizard's
    /// "Source repo (owner/name)" answer a no-op. When they disagree, the
    /// recorded id is dropped so preflight resolves the Project for `repo`.
    pub(crate) fn resolve_project_id(&self, git_root: &Path, repo: &str) -> Option<i64> {
        if let Some(id) = self.project_id.filter(|id| *id > 0) {
            return Some(id);
        }
        if let Some(id) = env_value(PROJECT_ID_ENV)
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|id| *id > 0)
        {
            return Some(id);
        }
        let state = LocalDeployment::read(git_root).ok().flatten()?;
        let id = (state.project_id > 0).then_some(state.project_id)?;
        let recorded_repo = state
            .source_repo_hint()
            .and_then(|hint| normalize_github_repo(hint).ok());
        match recorded_repo {
            // Recorded against a different repo — re-resolve rather than deploy
            // the wrong source under the user's nose.
            Some(recorded) if !recorded.eq_ignore_ascii_case(repo) => {
                eprintln!(
                    "  note: .aomi/deployment.json records project_id {id} for `{recorded}`, \
                     but this deploy targets `{repo}` — resolving its Project instead."
                );
                None
            }
            _ => Some(id),
        }
    }
}

fn validate_source_commit(value: &str) -> Result<String> {
    let commit = value.trim().to_ascii_lowercase();
    if (7..=40).contains(&commit.len()) && commit.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(commit)
    } else {
        bail!("source commit must be a git commit SHA (7-40 hex chars), got `{value}`")
    }
}

/// Normalize a user-supplied path to a clean repo-relative POSIX path.
fn normalize_project_path(value: &str) -> Result<String> {
    let path = value.trim();
    if path.is_empty()
        || path.starts_with('/')
        || path.contains('\\')
        || (!path.ends_with("/aomi.toml") && path != "aomi.toml")
        || path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        bail!("invalid application path `{value}` in {PROJECT_CONFIG_PATH}");
    }
    Ok(path.to_string())
}
