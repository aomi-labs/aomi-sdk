//! How a deploy step reads its inputs from the working tree.
//!
//! Everything here answers "what exactly are we deploying?" from git and
//! `aomi.toml` — the source commit, the manifest set, the destination platform,
//! and the connected source id — with no network involved. Split from the
//! lifecycle in `deploy.rs`, which consumes the answers.

use std::path::Path;

use anyhow::{Result, bail};

use super::DeployStepArgs;
use super::shared::{APP_SOURCE_ID_ENV, env_value, head_commit, remote_origin, tracked_aomi_tomls};
use crate::deploy::app::AomiAppFiles;
use crate::deploy::config::AomiConfig;
use crate::deploy::platform::{Platform, normalize_github_repo};
use crate::deploy::state::LocalDeployment;
use crate::deploy::types::BuildDeployInput;

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
        Ok(BuildDeployInput {
            platform: platform.to_string(),
            source_ref: self.source_ref(git_root)?,
            aomi_toml_paths: self.aomi_toml_paths(git_root)?,
            app_source_id: self.resolve_app_source_id(git_root, &repo),
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

    /// Platform declared by the manifests this deploy actually ships — the
    /// `--aomi-toml` set, or every tracked `aomi.toml` when the flag is absent.
    /// `None` when no manifest in the set declares one; an error when the set
    /// disagrees, since one deploy targets exactly one platform.
    pub(crate) fn manifest_platform(&self, git_root: &Path) -> Result<Option<Platform>> {
        let Ok(paths) = self.aomi_toml_paths(git_root) else {
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
                 A deploy targets one platform; align `[app].platform`, scope the deploy with --aomi-toml, or pass --platform."
            );
        }
        Ok(declared.pop().map(|(_, platform)| platform))
    }

    pub(crate) fn source_ref(&self, git_root: &Path) -> Result<String> {
        if self
            .branch
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .is_some()
        {
            bail!(
                "--branch is not supported by the current backend deploy contract; checkout the branch locally or pass --commit with a resolved SHA"
            );
        }
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

    pub(crate) fn aomi_toml_paths(&self, git_root: &Path) -> Result<Vec<String>> {
        if !self.aomi_toml.is_empty() {
            let mut paths: Vec<String> = self
                .aomi_toml
                .iter()
                .map(|p| normalize_rel_path(p))
                .collect::<Result<_>>()?;
            paths.sort();
            paths.dedup();
            return Ok(paths);
        }
        let found = tracked_aomi_tomls(git_root)?;
        if found.is_empty() {
            bail!(
                "no tracked aomi.toml found under {} — add and commit one, or pass --aomi-toml",
                git_root.display()
            );
        }
        Ok(found)
    }

    /// Resolution order: flag → env → the id recorded by a prior deploy /
    /// `source sync` in `.aomi/deployment.json`. The last step is what lets a
    /// re-deploy run with no `--app-source-id` once the source is known.
    ///
    /// `repo` is the source the caller actually asked to deploy. The backend
    /// resolves the repo *from* `app_source_id`, so a recorded id belonging to a
    /// different repo would silently win over that request — making the wizard's
    /// "Source repo (owner/name)" answer a no-op. When they disagree, the
    /// recorded id is dropped so preflight re-resolves the source from `repo`.
    pub(crate) fn resolve_app_source_id(&self, git_root: &Path, repo: &str) -> Option<i64> {
        if let Some(id) = self.app_source_id.filter(|id| *id > 0) {
            return Some(id);
        }
        if let Some(id) = env_value(APP_SOURCE_ID_ENV)
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|id| *id > 0)
        {
            return Some(id);
        }
        let state = LocalDeployment::read(git_root).ok().flatten()?;
        let id = state.app_source_id().filter(|id| *id > 0)?;
        let recorded_repo = state
            .source_repo_hint()
            .and_then(|hint| normalize_github_repo(hint).ok());
        match recorded_repo {
            // Recorded against a different repo — re-resolve rather than deploy
            // the wrong source under the user's nose.
            Some(recorded) if !recorded.eq_ignore_ascii_case(repo) => {
                eprintln!(
                    "  note: .aomi/deployment.json records app_source_id {id} for `{recorded}`, \
                     but this deploy targets `{repo}` — re-resolving the source."
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
fn normalize_rel_path(value: &str) -> Result<String> {
    let path = value.trim().replace('\\', "/");
    let path = path.strip_prefix("./").unwrap_or(&path);
    let path = path.trim_matches('/');
    if path.is_empty() {
        bail!("empty --aomi-toml path");
    }
    if path.split('/').any(|seg| seg == "..") {
        bail!("--aomi-toml path may not contain '..': `{value}`");
    }
    Ok(path.to_string())
}
