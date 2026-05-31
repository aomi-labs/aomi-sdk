use std::fmt;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::platform::Platform;

pub(crate) const ACTIVATION_TOKEN_ENV: &str = "AOMI_APP_ACTIVATION_TOKEN";
pub(crate) const BACKEND_URL_ENV: &str = "AOMI_BACKEND_URL";
const ACTIVATION_PATH: &str = "/api/admin/apps/activate";

/// Bound the config request so a stalled connect can't hang the CLI forever
/// (same failure class the `status` probe guards against).
const CONFIG_TIMEOUT: Duration = Duration::from_secs(20);
const CONFIG_CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

#[derive(Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum)]
pub enum Visibility {
    Private,
    Public,
}

impl Visibility {
    fn is_public(self) -> bool {
        matches!(self, Visibility::Public)
    }
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Visibility::Private => "private",
            Visibility::Public => "public",
        })
    }
}

pub(crate) fn parse_release_tag(value: &str) -> Result<(String, String)> {
    let Some(stripped) = value.strip_prefix("apps-") else {
        bail!("release tag must start with `apps-`");
    };
    let Some((app_slug, short_commit)) = stripped.rsplit_once('-') else {
        bail!("release tag must follow apps-{{app_slug}}-{{short_commit}}");
    };
    if app_slug.is_empty() || short_commit.is_empty() {
        bail!("release tag must include app slug and short commit");
    }
    if !app_slug
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        bail!("release tag app slug contains unsupported characters");
    }
    if !short_commit.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("release tag short commit must be hexadecimal");
    }
    Ok((app_slug.to_string(), short_commit.to_string()))
}

#[derive(Clone, Debug)]
pub struct ActivationPlan {
    pub backend_url: String,
    pub activation_token: String,
    pub request: ActivateAppRequest,
}

impl ActivationPlan {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        release_tag: &str,
        platform: Platform,
        backend_url: String,
        activation_token: String,
        visibility: Visibility,
        source_repo: String,
        github_token: Option<String>,
        target_tags: Vec<String>,
        label: Option<String>,
        source_commit: Option<String>,
        source_tree: Option<String>,
        source_digest: Option<String>,
    ) -> Result<Self> {
        let (app_slug, short_commit) = parse_release_tag(release_tag)?;
        let backend_url = backend_url.trim().trim_end_matches('/').to_string();
        if backend_url.is_empty() {
            bail!("backend URL is required via --backend or {BACKEND_URL_ENV}");
        }
        if activation_token.trim().is_empty() {
            bail!("activation token is required via --activation-token or {ACTIVATION_TOKEN_ENV}");
        }
        let source_repo = source_repo.trim().to_string();
        if source_repo.is_empty() {
            bail!("source_repo is required — pass --git or ensure aomi.toml declares [app].git");
        }

        let request = ActivateAppRequest {
            name: app_slug,
            label: label.and_then(non_empty),
            platform,
            source_repo,
            app_release_tag: release_tag.to_string(),
            source_commit: source_commit.and_then(non_empty),
            source_tree: source_tree.and_then(non_empty),
            source_digest: source_digest.and_then(non_empty),
            is_active: true,
            is_public: visibility.is_public(),
            metadata: json!({
                "requested_by": "aomi-git",
                "release_tag": release_tag,
                "short_commit": short_commit,
            }),
            github_token: github_token.and_then(non_empty),
            target_tags: normalize_tags(target_tags)?,
        };

        Ok(Self {
            backend_url,
            activation_token,
            request,
        })
    }

    pub fn endpoint(&self) -> String {
        format!("{}{ACTIVATION_PATH}", self.backend_url)
    }

    pub async fn execute(&self) -> Result<Value> {
        let endpoint = self.endpoint();
        let response = reqwest::Client::new()
            .post(&endpoint)
            .bearer_auth(&self.activation_token)
            .json(&self.request)
            .send()
            .await
            .with_context(|| {
                format!(
                    "failed to call activation endpoint {} on {}",
                    endpoint, self.backend_url
                )
            })?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read activation response body")?;
        if !matches!(status.as_u16(), 200 | 201) {
            bail!(
                "activation endpoint {} returned {}: {}",
                endpoint,
                status,
                body.trim()
            );
        }

        serde_json::from_str(&body).context("activation response was not valid JSON")
    }
}

/// A metadata-only configuration edit, posted to the same
/// `/api/admin/apps/activate` endpoint the `activate` subcommand uses, but
/// keyed on the app **name** instead of a release tag and deliberately
/// omitting `source_repo` / `app_release_tag`.
///
/// Omitting those two fields is load-bearing: the backend only runs its
/// one-shot release fetch when BOTH are present (see `activate_app`). Without
/// them it takes the "set_public-style edit" path — it updates the existing
/// registry row in place and skips the fetch entirely. That keeps `config`
/// cheap and side-effect-free, and avoids the fetch path that currently 502s.
///
/// `config` sets exactly two things on the registry row: the visibility flag
/// and the display label. Everything else in the request is identity/auth
/// plumbing, not a setting.
///
/// Field omission controls what the backend touches:
/// - `is_public = None` → the backend leaves the row's public flag untouched
///   (it only calls `set_public` when the value is present AND differs).
///
/// `label` is the exception: the backend's upsert ALWAYS rewrites the row's
/// label (falling back to the app name when omitted). So `config` must send
/// the app's current `display_name` even when the user isn't changing it, or
/// the label would be clobbered with the bare slug. `ConfigArgs` resolves it
/// from `--display-name` → `.aomi/deployment.json` and passes it here.
///
/// The "did the user actually ask to change anything" check lives in
/// `ConfigArgs` (keyed on the explicit flags), not here — by the time we build
/// the request, `label` is almost always populated for preservation.
#[derive(Clone, Debug)]
pub struct ConfigPlan {
    pub backend_url: String,
    pub activation_token: String,
    pub request: ConfigRequest,
}

impl ConfigPlan {
    pub fn new(
        app_name: String,
        platform: Platform,
        backend_url: String,
        activation_token: String,
        is_public: Option<bool>,
        display_name: Option<String>,
    ) -> Result<Self> {
        let app_name = app_name.trim().to_ascii_lowercase();
        if app_name.is_empty() {
            bail!(
                "app name is required for config — pass --app or run from a deployed source repo"
            );
        }
        if !app_name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            bail!("app name `{app_name}` contains unsupported characters");
        }

        let backend_url = backend_url.trim().trim_end_matches('/').to_string();
        if backend_url.is_empty() {
            bail!("backend URL is required via --backend or {BACKEND_URL_ENV}");
        }
        if activation_token.trim().is_empty() {
            bail!("activation token is required via --activation-token or {ACTIVATION_TOKEN_ENV}");
        }

        let request = ConfigRequest {
            name: app_name,
            platform,
            label: display_name.and_then(non_empty),
            is_public,
            metadata: json!({ "requested_by": "aomi-git-config" }),
        };

        Ok(Self {
            backend_url,
            activation_token,
            request,
        })
    }

    pub fn endpoint(&self) -> String {
        format!("{}{ACTIVATION_PATH}", self.backend_url)
    }

    pub async fn execute(&self) -> Result<Value> {
        let endpoint = self.endpoint();
        let client = reqwest::Client::builder()
            .timeout(CONFIG_TIMEOUT)
            .connect_timeout(CONFIG_CONNECT_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let response = client
            .post(&endpoint)
            .bearer_auth(&self.activation_token)
            .json(&self.request)
            .send()
            .await
            .with_context(|| {
                format!(
                    "failed to call activation endpoint {} on {}",
                    endpoint, self.backend_url
                )
            })?;

        let status = response.status();
        let body = response
            .text()
            .await
            .context("failed to read config response body")?;
        if !matches!(status.as_u16(), 200 | 201) {
            bail!(
                "config endpoint {} returned {}: {}",
                endpoint,
                status,
                body.trim()
            );
        }

        serde_json::from_str(&body).context("config response was not valid JSON")
    }
}

/// Wire body for a metadata-only config edit. Optional fields are skipped when
/// absent so the backend's upsert preserves the corresponding column rather
/// than overwriting it. `name`, `platform`, and `metadata` are always sent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ConfigRequest {
    pub name: String,
    pub platform: Platform,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_public: Option<bool>,
    pub metadata: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActivateAppRequest {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub platform: Platform,
    pub source_repo: String,
    pub app_release_tag: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_tree: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_digest: Option<String>,
    pub is_active: bool,
    pub is_public: bool,
    pub metadata: Value,
    /// Required backend server tags. The backend loads only when these tags are
    /// a subset of its configured AOMI_SERVER_TAGS.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub target_tags: Vec<String>,
    /// Per ADR 0009 amended: transient GitHub read token resolved from
    /// `aomi.toml[app].access_token` (an env-var reference). Sent once, used
    /// once by the backend, never persisted. Skipped in serialization when
    /// absent so the wire body stays clean for public-release activations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_token: Option<String>,
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn normalize_tags(values: Vec<String>) -> Result<Vec<String>> {
    let mut tags = Vec::new();
    for raw in values {
        let tag = raw.trim().to_ascii_lowercase();
        if tag.is_empty() {
            continue;
        }
        if !tag
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            bail!("target tag `{raw}` contains unsupported characters");
        }
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    Ok(tags)
}
