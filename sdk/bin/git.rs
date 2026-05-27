use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::plan::short_hash;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GitRepo {
    root: PathBuf,
    start_dir: PathBuf,
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct Source {
    pub git_root: PathBuf,
    pub source_path: PathBuf,
    pub branch: String,
    pub commit: String,
    pub tree: String,
    pub digest: String,
    pub dirty: bool,
}

impl GitRepo {
    pub fn discover(start: impl AsRef<Path>) -> Result<Self> {
        let start_dir = normalize_start_dir(start.as_ref())?;
        let root = git_output_at(&start_dir, ["rev-parse", "--show-toplevel"])
            .with_context(|| format!("failed to find git root from {}", start_dir.display()))?;
        let root = PathBuf::from(root.trim());
        let root = root.canonicalize().unwrap_or(root);

        Ok(Self { root, start_dir })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn start_dir(&self) -> &Path {
        &self.start_dir
    }

    pub fn snapshot(&self, source_path: &Path, allow_dirty: bool) -> Result<Source> {
        let dirty = !self
            .git(["status", "--porcelain=v1", "--untracked-files=all"])?
            .trim()
            .is_empty();
        if dirty && !allow_dirty {
            bail!("git tree is dirty; commit or stash changes, or pass --allow-dirty");
        }

        let commit = self.git(["rev-parse", "HEAD"])?.trim().to_string();
        let branch = self.branch_name(&commit)?;
        let tree = self.tree_for(source_path)?;
        let digest = self.archive_digest(source_path)?;

        Ok(Source {
            git_root: self.root.clone(),
            source_path: source_path.to_path_buf(),
            branch,
            commit,
            tree,
            digest,
            dirty,
        })
    }

    fn branch_name(&self, commit: &str) -> Result<String> {
        match self.git(["symbolic-ref", "--quiet", "--short", "HEAD"]) {
            Ok(branch) => Ok(branch.trim().to_string()),
            Err(_) => Ok(format!("detached@{}", short_hash(commit))),
        }
    }

    fn tree_for(&self, source_path: &Path) -> Result<String> {
        let tree_ref = if source_path.as_os_str().is_empty() || source_path == Path::new(".") {
            "HEAD^{tree}".to_string()
        } else {
            format!("HEAD:{}", source_path.display())
        };
        Ok(self
            .git(["rev-parse", tree_ref.as_str()])?
            .trim()
            .to_string())
    }

    fn archive_digest(&self, source_path: &Path) -> Result<String> {
        let mut args = vec!["archive", "--format=tar", "HEAD"];
        if !(source_path.as_os_str().is_empty() || source_path == Path::new(".")) {
            args.push("--");
            args.push(source_path.to_str().ok_or_else(|| {
                anyhow!("source path is not valid UTF-8: {}", source_path.display())
            })?);
        }

        let archive = self.git_bytes(args)?;
        Ok(format!("sha256:{:x}", Sha256::digest(&archive)))
    }

    pub(crate) fn tracked_files(&self, source_path: &Path) -> Result<Vec<PathBuf>> {
        let mut args = vec!["ls-tree", "-r", "-z", "--name-only", "HEAD"];
        if !(source_path.as_os_str().is_empty() || source_path == Path::new(".")) {
            args.push("--");
            args.push(source_path.to_str().ok_or_else(|| {
                anyhow!("source path is not valid UTF-8: {}", source_path.display())
            })?);
        }

        let output = self.git_bytes(args)?;
        let mut files: Vec<PathBuf> = output
            .split(|byte| *byte == 0)
            .filter(|raw| !raw.is_empty())
            .map(|raw| {
                String::from_utf8(raw.to_vec())
                    .map(PathBuf::from)
                    .context("git file path was not valid UTF-8")
            })
            .collect::<Result<_>>()?;
        files.sort();
        Ok(files)
    }

    pub(crate) fn file_at_head(&self, path: &Path) -> Result<Vec<u8>> {
        let path = path
            .to_str()
            .ok_or_else(|| anyhow!("source path is not valid UTF-8: {}", path.display()))?;
        self.git_bytes(["show".to_string(), format!("HEAD:{path}")])
    }

    pub(crate) fn remote_origin(&self) -> Result<String> {
        Ok(self.git(["remote", "get-url", "origin"])?.trim().to_string())
    }

    pub(crate) fn dirty_paths(&self) -> Result<Vec<PathBuf>> {
        let status = self.git(["status", "--porcelain=v1", "--untracked-files=all"])?;
        let mut paths = Vec::new();
        for line in status.lines() {
            if line.len() < 4 {
                continue;
            }
            let path = &line[3..];
            if let Some((old, new)) = path.split_once(" -> ") {
                paths.push(PathBuf::from(old));
                paths.push(PathBuf::from(new));
            } else {
                paths.push(PathBuf::from(path));
            }
        }
        Ok(paths)
    }

    pub(crate) fn checkout_or_create_branch(&self, branch: &str) -> Result<()> {
        if self.ref_exists(&format!("refs/heads/{branch}"))? {
            self.git(["checkout", branch])?;
        } else if self.ref_exists(&format!("refs/remotes/origin/{branch}"))? {
            self.git(["checkout", "-b", branch, &format!("origin/{branch}")])?;
        } else {
            self.git(["checkout", "-b", branch])?;
        }
        Ok(())
    }

    pub(crate) fn add_path(&self, path: &str) -> Result<()> {
        self.git(["add", "-A", "--", path])?;
        Ok(())
    }

    pub(crate) fn has_staged_changes_under(&self, path: &str) -> Result<bool> {
        Ok(!self
            .exit_status(["diff", "--cached", "--quiet", "--", path])?
            .success())
    }

    pub(crate) fn commit_with_message(&self, subject: &str, body: &str) -> Result<String> {
        let output = ProcessCommand::new("git")
            .arg("-C")
            .arg(&self.root)
            .arg("commit")
            .arg("-m")
            .arg(subject)
            .arg("-m")
            .arg(body)
            .output()
            .with_context(|| format!("failed to run git commit in {}", self.root.display()))?;
        if !output.status.success() {
            bail!(
                "git commit failed in {}: {}",
                self.root.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        self.head_commit()
    }

    pub(crate) fn push_branch(&self, branch: &str) -> Result<()> {
        self.git([
            "push",
            "origin",
            &format!("refs/heads/{branch}:refs/heads/{branch}"),
        ])?;
        Ok(())
    }

    pub(crate) fn head_commit(&self) -> Result<String> {
        Ok(self.git(["rev-parse", "HEAD"])?.trim().to_string())
    }

    pub(crate) fn current_branch(&self) -> Result<String> {
        Ok(self
            .git(["symbolic-ref", "--short", "HEAD"])?
            .trim()
            .to_string())
    }

    fn ref_exists(&self, reference: &str) -> Result<bool> {
        Ok(self
            .exit_status(["rev-parse", "--verify", "--quiet", reference])?
            .success())
    }

    pub(crate) fn git<const N: usize>(&self, args: [&str; N]) -> Result<String> {
        git_output_at(&self.root, args)
    }

    pub(crate) fn exit_status<const N: usize>(
        &self,
        args: [&str; N],
    ) -> Result<std::process::ExitStatus> {
        ProcessCommand::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .status()
            .with_context(|| format!("failed to run git in {}", self.root.display()))
    }

    fn git_bytes<I, S>(&self, args: I) -> Result<Vec<u8>>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        let output = ProcessCommand::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .with_context(|| format!("failed to run git in {}", self.root.display()))?;
        if !output.status.success() {
            bail!(
                "git command failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(output.stdout)
    }
}

fn normalize_start_dir(path: &Path) -> Result<PathBuf> {
    let path = if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    };
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    Ok(if path.is_file() {
        path.parent()
            .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?
            .to_path_buf()
    } else {
        path
    })
}

pub(crate) fn git_output_at<const N: usize>(dir: &Path, args: [&str; N]) -> Result<String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", dir.display()))?;
    if !output.status.success() {
        bail!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("git output was not valid UTF-8")
}
