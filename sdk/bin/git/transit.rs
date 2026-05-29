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
//! `resolve_transit_clone` is idempotent: it clones on first call and runs
//! `fetch` + force-reset on subsequent calls. The escape hatch
//! (`--platform-dir <DIR>`) bypasses this module entirely; users who want to
//! hand-manage their clone still can.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};

use crate::preflight::normalize_github_url;

const TRANSIT_DIR: &str = "transit";

/// Resolve (and lazily refresh) a managed clone of `platform_git` at
/// `target_branch`. Returns the path to the clone — ready for the existing
/// stage code to write into.
///
/// Accepts any input `normalize_github_url` accepts: full HTTPS URL,
/// SSH URL, or bare `owner/repo`.
pub(crate) fn resolve_transit_clone(platform_git: &str, target_branch: &str) -> Result<PathBuf> {
    let owner_repo = normalize_github_url(platform_git);
    if owner_repo.is_empty() || !owner_repo.contains('/') {
        bail!(
            "platform git URL `{platform_git}` does not look like a GitHub owner/repo \
             (expected `owner/name`, `https://github.com/owner/name`, or `git@github.com:owner/name.git`)"
        );
    }

    let cache_root = transit_cache_root()?;
    let key = owner_repo.replace('/', "-");
    let clone_path = cache_root.join(&key);

    if clone_path.join(".git").is_dir() {
        refresh_existing(&clone_path, target_branch).with_context(|| {
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
        std::fs::create_dir_all(&cache_root)
            .with_context(|| format!("failed to create {}", cache_root.display()))?;
        let url = https_url(&owner_repo);
        fresh_clone(&url, target_branch, &clone_path)?;
    }

    Ok(clone_path)
}

/// `$AOMI_HOME/transit/`, defaulting to `$HOME/.aomi/transit/`.
///
/// Tests can override by setting `AOMI_HOME`.
fn transit_cache_root() -> Result<PathBuf> {
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
    Ok(aomi_home.join(TRANSIT_DIR))
}

fn https_url(owner_repo: &str) -> String {
    format!("https://github.com/{owner_repo}.git")
}

fn fresh_clone(url: &str, branch: &str, dest: &Path) -> Result<()> {
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
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".to_string())
        );
    }
    Ok(())
}

fn refresh_existing(clone_path: &Path, branch: &str) -> Result<()> {
    // `git fetch` — pick up new commits + delete stale refs.
    let status = Command::new("git")
        .current_dir(clone_path)
        .args(["fetch", "--quiet", "--prune", "origin"])
        .status()
        .context("failed to invoke `git fetch`")?;
    if !status.success() {
        bail!(
            "`git fetch` in transit clone exited {}",
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".to_string())
        );
    }

    // Force-flip local branch to the upstream tip. -B both creates and resets,
    // so this works regardless of whether the branch exists locally or what
    // commit it's currently at.
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
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".to_string())
        );
    }

    // Wipe any untracked debris from prior runs. Don't pass -x: that would
    // nuke .git/config and credential caches.
    let status = Command::new("git")
        .current_dir(clone_path)
        .args(["clean", "--quiet", "-fd"])
        .status()
        .context("failed to invoke `git clean`")?;
    if !status.success() {
        bail!(
            "`git clean -fd` exited {}",
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".to_string())
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn https_url_round_trip() {
        assert_eq!(
            https_url("aomi-labs/community-apps"),
            "https://github.com/aomi-labs/community-apps.git"
        );
    }

    #[test]
    fn resolve_rejects_input_without_slash() {
        let err = resolve_transit_clone("not-a-repo", "publish")
            .expect_err("input without a / must be rejected");
        assert!(err.to_string().contains("owner/repo"), "{err}");
    }
}
