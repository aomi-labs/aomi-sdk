use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::git::{GitRepo, Source};
use crate::plan::{Deployment, short_hash};

/// Universal publish conventions. Previously these lived in a checked-in
/// `platforms.json` registry; per ADR 0009 the only sources of truth are
/// the user's `aomi.toml` (for `git` / `platform` / `branch`) and the
/// backend's `platforms` table (for the contractual `deployment_branch`,
/// resolved during preflight). Conventions that don't vary per-platform
/// stay here as constants.
pub const DEFAULT_PUBLISH_BRANCH: &str = "publish";
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
pub struct PublishTarget {
    pub source_repo: String,
    pub publish_branch: String,
    pub app_path: String,
    pub release_tag: String,
}

impl PublishTarget {
    /// Resolve a deployment target from the user's `aomi.toml` + the git
    /// snapshot. `aomi.toml[app].git` IS the source repo — no registry
    /// fallback. If `git` is missing, the deploy cannot proceed and we
    /// surface a clear error pointing the user at their aomi.toml.
    pub fn resolve(_platform: &Platform, app: &App, source: &Source) -> Result<Self> {
        let raw_git = app
            .git
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "aomi.toml is missing `[app].git` — set it to the publish repo, e.g. `git = \"https://github.com/aomi-labs/community-apps\"`"
                )
            })?;
        let source_repo = normalize_github_repo(raw_git)?;

        let app_path = format!("{}/{}", DEFAULT_APP_PATH_PREFIX, app.name.trim_matches('/'));
        let release_tag = DEFAULT_RELEASE_TAG_TEMPLATE
            .replace("{app_slug}", &app.name)
            .replace("{short_commit}", &short_hash(&source.commit));

        Ok(Self {
            source_repo,
            publish_branch: DEFAULT_PUBLISH_BRANCH.to_string(),
            app_path,
            release_tag,
        })
    }
}

pub(crate) fn verify_remote_origin(repo: &GitRepo, expected_repo: &str) -> Result<()> {
    let remote = repo.remote_origin().with_context(|| {
        format!(
            "platform repo {} must have an origin remote",
            repo.root().display()
        )
    })?;
    let actual = normalize_github_repo(&remote)?;
    let expected = normalize_github_repo(expected_repo)?;
    if actual != expected {
        bail!(
            "platform repo origin `{remote}` does not match expected publish repo `{expected_repo}`"
        );
    }
    Ok(())
}

pub(crate) fn ensure_dirty_scope(repo: &GitRepo, app_path: &str) -> Result<()> {
    let app_prefix = Path::new(app_path);
    let unowned: Vec<_> = repo
        .dirty_paths()?
        .into_iter()
        .filter(|path| !path.starts_with(app_prefix))
        .collect();
    if !unowned.is_empty() {
        bail!(
            "platform repo has dirty files outside owned publish path {}: {}",
            app_path,
            unowned
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}

pub(crate) fn commit_message(deployment: &Deployment) -> (String, String) {
    let subject = format!(
        "Deploy {} to {} from {} as {}",
        deployment.app.name,
        deployment.platform,
        short_hash(&deployment.source.commit),
        deployment.publish.release_tag
    );
    let body = format!(
        "app_slug: {}\nplatform: {}\nsource_commit: {}\nrelease_tag: {}",
        deployment.app.name,
        deployment.platform,
        deployment.source.commit,
        deployment.publish.release_tag
    );
    (subject, body)
}

fn normalize_github_repo(value: &str) -> Result<String> {
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
        return Ok(repo);
    }
    bail!("unsupported GitHub repo remote `{value}`");
}
