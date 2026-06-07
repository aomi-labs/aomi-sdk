//! Local git facts the backend relay needs — but that are *contract-independent*
//! and stable regardless of the final deploy/activate wire shape.
//!
//! Two primitives live here, both pure functions over a git working tree so they
//! are unit-testable without the rest of the CLI:
//!
//! 1. [`discover_aomi_tomls`] — every tracked `aomi.toml` under the repo, as
//!    repo-relative POSIX paths. This is the `aomi_toml_paths` payload the
//!    repo-scoped deploy will send (see ADR 0006 / CONTRACTS).
//! 2. [`commit_push_state`] / [`ensure_pushed`] — the "push-first guard"
//!    (backlog 17): deploy ships the commit *on GitHub*, not the local working
//!    tree, so the resolved commit must already exist on a remote before we ask
//!    the backend to fetch it.
//!
//! These are deliberately NOT wired into `cli.rs` yet — the client relay is
//! frozen until the backend repo-scoped endpoints land (ADR 0006). They are
//! built and tested ahead of time because they cannot change with the wire
//! contract.

use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::git::git_output_at;

const MANIFEST_FILE: &str = "aomi.toml";

/// Every tracked `aomi.toml` in the repo at `root`, returned as repo-relative
/// POSIX paths, sorted and de-duplicated.
///
/// Uses `git ls-files`, so only *tracked* files count: untracked/ignored
/// manifests, `target/`, and `node_modules/` are excluded for free, and the set
/// matches what a pushed commit would actually contain — consistent with the
/// push-first guard.
pub fn discover_aomi_tomls(root: &Path) -> Result<Vec<String>> {
    // `-z` gives NUL-separated, quoting-free output (paths with spaces/unicode
    // survive). `--full-name` is implied by ls-files (paths are repo-relative).
    let raw = git_output_at(root, ["ls-files", "-z", "--", "*aomi.toml", "aomi.toml"])
        .with_context(|| format!("failed to list tracked files in {}", root.display()))?;

    let mut paths: Vec<String> = raw
        .split('\0')
        .filter(|entry| !entry.is_empty())
        // ls-files patterns can match substrings like `not-aomi.toml`; keep only
        // entries whose final path segment is exactly `aomi.toml`.
        .filter(|entry| {
            entry
                .rsplit('/')
                .next()
                .map(|name| name == MANIFEST_FILE)
                .unwrap_or(false)
        })
        .map(str::to_string)
        .collect();

    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Whether a commit is present on a remote (and therefore fetchable by the
/// backend), with the remote-tracking refs that contain it.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum PushState {
    /// The commit is contained by at least one remote-tracking ref.
    Pushed { remote_refs: Vec<String> },
    /// The commit exists locally but on no remote-tracking ref.
    NotPushed,
}

/// Determine whether `commit` exists on any remote-tracking branch of the repo
/// at `root`.
///
/// `git branch -r --contains <commit>` lists every remote-tracking ref whose
/// history includes the commit; an empty list means it has not been pushed
/// anywhere the local repo knows about. (This reflects the last `git fetch`; it
/// is a best-effort *local* guard, not a live GitHub query — which is exactly
/// what we want here: no token, no network dependency.)
pub fn commit_push_state(root: &Path, commit: &str) -> Result<PushState> {
    let commit = commit.trim();
    if commit.is_empty() {
        bail!("commit is empty");
    }
    let out = git_output_at(root, ["branch", "-r", "--contains", commit])
        .with_context(|| format!("failed to check remote refs containing `{commit}`"))?;

    let remote_refs: Vec<String> = out
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        // `git branch -r` may print a symref row like `origin/HEAD -> origin/main`;
        // keep the left side only and drop the alias arrow rows.
        .filter(|line| !line.contains("->"))
        .map(str::to_string)
        .collect();

    if remote_refs.is_empty() {
        Ok(PushState::NotPushed)
    } else {
        Ok(PushState::Pushed { remote_refs })
    }
}

/// Guard used before a live deploy: fail with an actionable "push first" message
/// when the resolved commit is not yet on a remote, since the backend fetches
/// the commit from GitHub rather than the local tree.
pub fn ensure_pushed(root: &Path, commit: &str) -> Result<()> {
    match commit_push_state(root, commit)? {
        PushState::Pushed { .. } => Ok(()),
        PushState::NotPushed => bail!(
            "commit `{}` is not on any remote yet — `git push` it first.\n\
             Deploy ships the commit from GitHub (the backend fetches it via the \
             Aomi GitHub App), not your local working tree, so an unpushed commit \
             cannot be deployed.",
            short(commit)
        ),
    }
}

fn short(commit: &str) -> String {
    commit.chars().take(12).collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;

    use super::*;

    fn git<const N: usize>(cwd: &Path, args: [&str; N]) {
        let status = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed");
    }

    fn init(dir: &Path) {
        git(dir, ["init", "-q", "-b", "main"]);
        git(dir, ["config", "user.email", "t@example.test"]);
        git(dir, ["config", "user.name", "Test"]);
    }

    fn write(root: &Path, rel: &str, body: &str) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn discovers_tracked_manifests_repo_relative_sorted() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        init(root);

        write(root, "aomi.toml", "[app]\nname=\"root\"\n");
        write(root, "apps/bot/aomi.toml", "[app]\nname=\"bot\"\n");
        write(root, "apps/bot-2/aomi.toml", "[app]\nname=\"bot2\"\n");
        // a decoy that must NOT match the manifest filename
        write(root, "apps/not-aomi.toml", "x\n");
        git(root, ["add", "."]);
        git(root, ["commit", "-q", "-m", "init"]);

        let found = discover_aomi_tomls(root).unwrap();
        assert_eq!(
            found,
            vec![
                "aomi.toml".to_string(),
                "apps/bot-2/aomi.toml".to_string(),
                "apps/bot/aomi.toml".to_string(),
            ]
        );
    }

    #[test]
    fn untracked_manifest_is_ignored() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        init(root);
        write(root, "aomi.toml", "[app]\nname=\"root\"\n");
        git(root, ["add", "."]);
        git(root, ["commit", "-q", "-m", "init"]);
        // second manifest staged-but-not-committed is still tracked by ls-files
        // once added; an entirely untracked one is not.
        write(root, "apps/ghost/aomi.toml", "[app]\nname=\"ghost\"\n");

        let found = discover_aomi_tomls(root).unwrap();
        assert_eq!(found, vec!["aomi.toml".to_string()]);
    }

    #[test]
    fn push_state_distinguishes_pushed_from_local_only() {
        let remote_dir = TempDir::new().unwrap();
        // bare repo acts as the "remote" / GitHub.
        git(remote_dir.path(), ["init", "-q", "--bare", "-b", "main"]);

        let work = TempDir::new().unwrap();
        let root = work.path();
        init(root);
        git(
            root,
            [
                "remote",
                "add",
                "origin",
                remote_dir.path().to_str().unwrap(),
            ],
        );

        write(root, "aomi.toml", "[app]\nname=\"root\"\n");
        git(root, ["add", "."]);
        git(root, ["commit", "-q", "-m", "c1"]);

        let local_only = head(root);
        assert_eq!(
            commit_push_state(root, &local_only).unwrap(),
            PushState::NotPushed
        );
        assert!(ensure_pushed(root, &local_only).is_err());

        git(root, ["push", "-q", "origin", "main"]);
        // refresh remote-tracking refs the push created
        let state = commit_push_state(root, &local_only).unwrap();
        assert!(
            matches!(state, PushState::Pushed { .. }),
            "expected pushed, got {state:?}"
        );
        assert!(ensure_pushed(root, &local_only).is_ok());

        // a new local commit on top is again not pushed
        write(root, "b.txt", "b\n");
        git(root, ["add", "."]);
        git(root, ["commit", "-q", "-m", "c2"]);
        assert_eq!(
            commit_push_state(root, &head(root)).unwrap(),
            PushState::NotPushed
        );
    }

    fn head(root: &Path) -> String {
        String::from_utf8(
            Command::new("git")
                .current_dir(root)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string()
    }
}
