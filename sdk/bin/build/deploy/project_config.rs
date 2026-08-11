use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::deploy::platform::Platform;

pub const PROJECT_CONFIG_PATH: &str = ".aomi/config.json";

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectConfig {
    version: u8,
    platform: Platform,
    applications: Vec<String>,
}

impl ProjectConfig {
    pub fn load(repo_root: &Path) -> Result<Self> {
        let path = repo_root.join(PROJECT_CONFIG_PATH);
        let bytes =
            fs::read(&path).with_context(|| format!("Project requires {}", path.display()))?;
        let config: Self = serde_json::from_slice(&bytes)
            .with_context(|| format!("invalid {}", path.display()))?;
        config.validate(repo_root)
    }

    pub fn create(repo_root: &Path, platform: &Platform) -> Result<(Self, PathBuf)> {
        let path = repo_root.join(PROJECT_CONFIG_PATH);
        if path.exists() {
            let config = Self::load(repo_root)?;
            if config.platform != *platform {
                bail!(
                    "{} selects platform `{}`, not `{platform}`",
                    path.display(),
                    config.platform
                );
            }
            return Ok((config, path));
        }

        let mut applications = Vec::new();
        discover_manifests(repo_root, repo_root, &mut applications)?;
        applications.sort();
        let config = Self {
            version: 1,
            platform: platform.clone(),
            applications,
        };
        fs::create_dir_all(path.parent().expect("project config has a parent"))?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(&config)?)?;
        fs::rename(&temporary, &path)?;
        Ok((config, path))
    }

    fn validate(mut self, repo_root: &Path) -> Result<Self> {
        if self.version != 1 {
            bail!(
                "unsupported {PROJECT_CONFIG_PATH} version {}; expected 1",
                self.version
            );
        }
        self.platform = self.platform.as_str().parse()?;
        let mut seen = HashSet::new();
        for value in &mut self.applications {
            *value = normalize_application_path(value)?;
            if !seen.insert(value.clone()) {
                bail!("{PROJECT_CONFIG_PATH} contains duplicate application `{value}`");
            }
            if !repo_root.join(&*value).is_file() {
                bail!("{PROJECT_CONFIG_PATH} references missing `{value}`");
            }
        }
        Ok(self)
    }

    pub fn platform(&self) -> &Platform {
        &self.platform
    }

    pub fn applications(&self) -> &[String] {
        &self.applications
    }
}

fn discover_manifests(root: &Path, directory: &Path, manifests: &mut Vec<String>) -> Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name();
        if file_type.is_dir() {
            if matches!(
                name.to_str(),
                Some(".git" | ".aomi" | "target" | "node_modules")
            ) {
                continue;
            }
            discover_manifests(root, &path, manifests)?;
        } else if name == "aomi.toml" {
            manifests.push(
                path.strip_prefix(root)
                    .expect("discovered path is below root")
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
    Ok(())
}

fn normalize_application_path(value: &str) -> Result<String> {
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
