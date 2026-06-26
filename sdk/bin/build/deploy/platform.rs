use std::fmt;
use std::str::FromStr;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// Opaque platform identifier. Any well-formed string is accepted at the CLI;
/// the backend's `platforms` table is the authority on what's valid. It names
/// the platform path segment of the repo-scoped deploy endpoint.
#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Platform(String);

impl Platform {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into().trim().to_ascii_lowercase())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn community() -> Self {
        Self::new("community")
    }
}

impl Default for Platform {
    fn default() -> Self {
        Self::community()
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Platform {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            bail!("platform name cannot be empty");
        }
        if s.starts_with('.') || s.ends_with('.') || s.contains("..") {
            bail!("platform name `{s}` has invalid dot placement");
        }
        if !s
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        {
            bail!(
                "platform name `{s}` contains unsupported characters (ASCII alphanumeric, '-', '_', '.' only)"
            );
        }
        Ok(Self::new(s))
    }
}

/// Normalize a GitHub remote (`git@…`, `https://…`, `owner/repo.git`, …) to the
/// canonical lowercase `owner/repo`. Used by the legacy `request` flow to print
/// a clean repo for ops; the live deploy path never sends a source repo.
pub(crate) fn normalize_github_repo(value: &str) -> Result<String> {
    let mut repo = value
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_string();
    for prefix in [
        "git@github.com:",
        "ssh://git@github.com/",
        "https://github.com/",
        "http://github.com/",
        "github.com/",
    ] {
        if let Some(stripped) = repo.strip_prefix(prefix) {
            repo = stripped.to_string();
            break;
        }
    }
    if repo.split('/').count() == 2
        && repo
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/'))
    {
        return Ok(repo.to_ascii_lowercase());
    }
    bail!("unsupported GitHub repo remote `{value}`");
}
