use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, anyhow, bail};

/// A local git working tree. The relay reads facts from it (HEAD, branch,
/// cleanliness, origin) but never clones or pushes — deploy ships the commit
/// from GitHub via the backend.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GitRepo {
    root: PathBuf,
    start_dir: PathBuf,
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

    /// The full HEAD commit SHA.
    pub fn head_commit(&self) -> Result<String> {
        Ok(self.git(["rev-parse", "HEAD"])?.trim().to_string())
    }

    pub fn remote_origin(&self) -> Result<String> {
        Ok(self
            .git(["remote", "get-url", "origin"])?
            .trim()
            .to_string())
    }

    pub(crate) fn git<const N: usize>(&self, args: [&str; N]) -> Result<String> {
        git_output_at(&self.root, args)
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
