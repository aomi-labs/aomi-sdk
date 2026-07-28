//! Global CLI identity persisted at `~/.config/aomi/config.toml`.
//!
//! This is account-level state established by `aomi-build login` or `connect`:
//! which backend and Build UI to use, the verified GitHub Builder identity, and
//! any CLI/admin credentials. Per-project state stays in `.aomi/deployment.json`.
//!
//! The file holds a secret (the activation token), so it is written `0600`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AomiConfig {
    /// Backend base URL (`AOMI_BACKEND_URL`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_url: Option<String>,
    /// Aomi Build base URL (`AOMI_BUILD_URL`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_url: Option<String>,
    /// Platform the connection is scoped to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// Connected GitHub App installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<i64>,
    /// Verified GitHub user attached to this Builder login.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_user_id: Option<String>,
    /// Verified GitHub login for friendly CLI output.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_login: Option<String>,
    /// Builder-authenticated CLI bearer issued by Aomi Build.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cli_access_token: Option<String>,
    /// Activation token (issued by the Aomi admin). Secret — file is `0600`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_token: Option<String>,
}

impl AomiConfig {
    fn dir() -> PathBuf {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            let xdg = xdg.trim();
            if !xdg.is_empty() {
                return PathBuf::from(xdg).join("aomi");
            }
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".config").join("aomi")
    }

    /// Load the config, or a default when it's missing/unreadable. A malformed
    /// file falls back to default rather than failing every command.
    pub fn load() -> Self {
        Self::load_in(&Self::dir())
    }

    /// Read-modify-write the on-disk config under `mutate`.
    ///
    /// [`save`](Self::save) replaces the whole file, so saving a struct that was
    /// loaded before some other code path wrote to it silently drops that
    /// code path's fields — which is how the wizard used to erase the CLI bearer
    /// that `login` had just persisted. Anything mutating a subset of the config
    /// should go through here so it always merges onto the current file.
    pub fn update(mutate: impl FnOnce(&mut Self)) -> Result<PathBuf> {
        Self::update_in(&Self::dir(), mutate)
    }

    /// Atomically write the config `0600`. Returns the path.
    ///
    /// Replaces the file wholesale. Prefer [`update`](Self::update) unless you
    /// genuinely own every field.
    pub fn save(&self) -> Result<PathBuf> {
        self.save_in(&Self::dir())
    }

    /// Directory-scoped variants. The config home is process-global, so tests
    /// address a directory directly rather than mutating `XDG_CONFIG_HOME` out
    /// from under whatever else is running in parallel.
    pub(crate) fn load_in(dir: &Path) -> Self {
        match std::fs::read_to_string(dir.join("config.toml")) {
            Ok(text) => toml::from_str(&text).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub(crate) fn update_in(dir: &Path, mutate: impl FnOnce(&mut Self)) -> Result<PathBuf> {
        let mut config = Self::load_in(dir);
        mutate(&mut config);
        config.save_in(dir)
    }

    pub(crate) fn save_in(&self, dir: &Path) -> Result<PathBuf> {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("failed to create {}", dir.display()))?;
        let path = dir.join("config.toml");
        let body = toml::to_string_pretty(self).context("failed to serialize config")?;
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, body).with_context(|| format!("failed to write {}", tmp.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &path)
            .with_context(|| format!("failed to rename {} -> {}", tmp.display(), path.display()))?;
        Ok(path)
    }
}
