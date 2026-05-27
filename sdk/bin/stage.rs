use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::git::GitRepo;
use crate::plan::Deployment;

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct StagedFile {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

pub(crate) fn manifest_path_in(app_dir: &Path) -> PathBuf {
    app_dir.join(".aomi-publish").join("manifest.json")
}

pub(crate) fn write_source_tree(
    repo: &GitRepo,
    source_path: &Path,
    app_dir: &Path,
) -> Result<Vec<StagedFile>> {
    if app_dir.exists() {
        fs::remove_dir_all(app_dir)
            .with_context(|| format!("failed to clear {}", app_dir.display()))?;
    }
    fs::create_dir_all(app_dir)
        .with_context(|| format!("failed to create {}", app_dir.display()))?;

    let source_files = repo.tracked_files(source_path)?;
    let mut staged = Vec::with_capacity(source_files.len());
    for repo_path in source_files {
        let relative_path = relative_source_file_path(&repo_path, source_path)?;
        let bytes = repo.file_at_head(&repo_path)?;
        let dest = app_dir.join(&relative_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&dest, &bytes)
            .with_context(|| format!("failed to write {}", dest.display()))?;
        staged.push(StagedFile {
            path: path_to_slash(&relative_path)?,
            sha256: format!("sha256:{:x}", Sha256::digest(&bytes)),
            bytes: bytes.len() as u64,
        });
    }
    staged.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(staged)
}

pub(crate) fn write_manifest(manifest_path: &Path, deployment: &Deployment) -> Result<()> {
    let parent = manifest_path
        .parent()
        .ok_or_else(|| anyhow!("manifest path has no parent: {}", manifest_path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {}", parent.display()))?;
    let json = serde_json::to_string_pretty(deployment)?;
    fs::write(manifest_path, format!("{json}\n"))
        .with_context(|| format!("failed to write {}", manifest_path.display()))
}

fn relative_source_file_path(repo_path: &Path, source_path: &Path) -> Result<PathBuf> {
    if source_path.as_os_str().is_empty() || source_path == Path::new(".") {
        return Ok(repo_path.to_path_buf());
    }
    repo_path
        .strip_prefix(source_path)
        .map(Path::to_path_buf)
        .with_context(|| {
            format!(
                "git file {} is not under source path {}",
                repo_path.display(),
                source_path.display()
            )
        })
}

fn path_to_slash(path: &Path) -> Result<String> {
    let value = path
        .to_str()
        .ok_or_else(|| anyhow!("path is not valid UTF-8: {}", path.display()))?;
    Ok(value.replace(std::path::MAIN_SEPARATOR, "/"))
}
