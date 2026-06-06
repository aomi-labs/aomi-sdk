use std::fmt;
use std::str::FromStr;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::git::Source;
use crate::plan::short_hash;

/// Universal deploy conventions. Previously these lived in a checked-in
/// `platforms.json` registry; per ADR 0009 the only sources of truth are
/// the user's `aomi.toml` (for app/platform intent), the local Git remote (for
/// Alice's source repo), and the backend's `platforms` table. The deployment
/// branch and platform repo are never client inputs on the live path.
pub const DEFAULT_APP_PATH_PREFIX: &str = "apps";
pub const DEFAULT_RELEASE_TAG_TEMPLATE: &str = "apps-{app_slug}-{short_commit}";

/// Opaque platform identifier per ADR 0009 F-2.
///
/// Any string is accepted at the CLI; the backend's `platforms` table is the
/// authority on what's valid. `aomi-git` validates against
/// `GET /api/control/platforms` during preflight.
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
        if !s
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            bail!(
                "platform name `{s}` contains unsupported characters (ASCII alphanumeric, '-', '_' only)"
            );
        }
        Ok(Self::new(s))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct DeployTarget {
    /// Legacy deployment-state field. New deploy requests send `source_path`
    /// separately and the backend owns the platform repo.
    pub source_repo: String,
    pub app_path: String,
    pub app_release_tag: String,
}

impl DeployTarget {
    /// Resolve a deployment target from the user's `aomi.toml` + the git
    /// snapshot. `aomi.toml[app].git` is intentionally ignored for new configs:
    /// source repo identity is resolved by `aomi-git deploy` from `origin` or
    /// `--source-path` and then sent to the backend.
    pub fn resolve(_platform: &Platform, app: &App, source: &Source) -> Result<Self> {
        let source_repo = app
            .git
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(normalize_github_repo)
            .transpose()?
            .unwrap_or_default();

        let app_path = format!("{}/{}", DEFAULT_APP_PATH_PREFIX, app.name.trim_matches('/'));
        let app_release_tag = DEFAULT_RELEASE_TAG_TEMPLATE
            .replace("{app_slug}", &app.name)
            .replace("{short_commit}", &short_hash(&source.commit));

        Ok(Self {
            source_repo,
            app_path,
            app_release_tag,
        })
    }
}

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
