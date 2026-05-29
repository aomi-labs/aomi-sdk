//! Managed transit clone for platform publish repos.
//!
//! Pre-0.2 contributors had to clone the platform repo themselves and pass
//! `--platform-repo-dir <PATH>` to `aomi-git deploy`. That leaked pipeline
//! plumbing into contributor headspace (where to put the clone, how to keep it
//! fresh, why the tool needs two repos at all).
//!
//! Post-0.2 we manage the clone ourselves under `$AOMI_HOME/transit/<owner>-<repo>/`
//! (defaulting to `~/.aomi/transit/…`). The cache is content-addressed by
//! normalized GitHub `owner/repo`, so changing platforms in `aomi.toml`
//! transparently uses a different clone instead of silently pushing to the
//! wrong place.
//!
//! `TransitCache::resolve` is idempotent: it clones on first call and runs
//! `fetch` + force-reset on subsequent calls. The escape hatch
//! (`--platform-dir <DIR>`) bypasses this module entirely; users who want to
//! hand-manage their clone still can.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};

use crate::platform::normalize_github_repo;

const TRANSIT_DIR: &str = "transit";

pub(crate) struct TransitCache {
    root: PathBuf,
}

impl TransitCache {
    /// `$AOMI_HOME/transit/`, defaulting to `$HOME/.aomi/transit/`.
    ///
    /// Tests can override by setting `AOMI_HOME`.
    pub(crate) fn load() -> Result<Self> {
        let aomi_home = match std::env::var_os("AOMI_HOME") {
            Some(v) if !v.is_empty() => PathBuf::from(v),
            _ => {
                let home = std::env::var_os("HOME")
                    .filter(|v| !v.is_empty())
                    .ok_or_else(|| {
                        anyhow!("neither AOMI_HOME nor HOME is set — cannot locate transit cache")
                    })?;
                PathBuf::from(home).join(".aomi")
            }
        };
        Ok(Self {
            root: aomi_home.join(TRANSIT_DIR),
        })
    }

    /// Resolve and lazily refresh a managed clone of `platform_git` at `branch`.
    /// Returns the clone path ready for the existing stage code to write into.
    pub(crate) fn resolve(&self, platform_git: &str, branch: &str) -> Result<PathBuf> {
        let owner_repo = normalize_github_repo(platform_git).with_context(|| {
            format!(
                "platform git URL `{platform_git}` does not look like a GitHub owner/repo \
                 (expected `owner/name`, `https://github.com/owner/name`, or `git@github.com:owner/name.git`)"
            )
        })?;
        let clone_path = self.root.join(owner_repo.replace('/', "-"));

        if clone_path.join(".git").is_dir() {
            self.refresh(&clone_path, branch).with_context(|| {
                format!(
                    "failed to refresh transit clone at {} — delete it and retry to re-clone",
                    clone_path.display()
                )
            })?;
        } else {
            if clone_path.exists() {
                bail!(
                    "{} exists but is not a git clone — refusing to touch it. Delete it and retry.",
                    clone_path.display()
                );
            }
            std::fs::create_dir_all(&self.root)
                .with_context(|| format!("failed to create {}", self.root.display()))?;
            self.clone_repo(
                &format!("https://github.com/{owner_repo}.git"),
                branch,
                &clone_path,
            )?;
        }

        Ok(clone_path)
    }

    fn clone_repo(&self, url: &str, branch: &str, dest: &Path) -> Result<()> {
        let status = Command::new("git")
            .args([
                "clone",
                "--quiet",
                "--branch",
                branch,
                "--single-branch",
                url,
            ])
            .arg(dest)
            .status()
            .context("failed to invoke `git clone` — is git installed and on PATH?")?;
        if !status.success() {
            bail!(
                "`git clone {url}` exited {} — check your auth (try `gh auth login`) and that the repo + branch exist",
                status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".to_string())
            );
        }
        Ok(())
    }

    fn refresh(&self, clone_path: &Path, branch: &str) -> Result<()> {
        let status = Command::new("git")
            .current_dir(clone_path)
            .args(["fetch", "--quiet", "--prune", "origin"])
            .status()
            .context("failed to invoke `git fetch`")?;
        if !status.success() {
            bail!(
                "`git fetch` in transit clone exited {}",
                status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".to_string())
            );
        }

        let status = Command::new("git")
            .current_dir(clone_path)
            .args([
                "checkout",
                "--quiet",
                "-B",
                branch,
                &format!("origin/{branch}"),
            ])
            .status()
            .context("failed to invoke `git checkout`")?;
        if !status.success() {
            bail!(
                "`git checkout -B {branch} origin/{branch}` exited {} — does that branch exist upstream?",
                status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".to_string())
            );
        }

        let status = Command::new("git")
            .current_dir(clone_path)
            .args(["clean", "--quiet", "-fd"])
            .status()
            .context("failed to invoke `git clean`")?;
        if !status.success() {
            bail!(
                "`git clean -fd` exited {}",
                status
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "?".to_string())
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_rejects_input_without_slash() {
        let cache = TransitCache {
            root: PathBuf::from("/tmp/aomi-transit-test"),
        };
        let err = cache
            .resolve("not-a-repo", "publish")
            .expect_err("input without a / must be rejected");
        assert!(err.to_string().contains("owner/repo"), "{err}");
    }
}
