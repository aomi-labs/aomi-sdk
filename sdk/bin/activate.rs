use std::fmt;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::platform::Platform;

pub(crate) const ACTIVATION_TOKEN_ENV: &str = "AOMI_APP_ACTIVATION_TOKEN";
pub(crate) const BACKEND_URL_ENV: &str = "AOMI_BACKEND_URL";
const ACTIVATION_PATH: &str = "/api/admin/apps/activate";

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
        label: Option<String>,
        source_commit: Option<String>,
        source_tree: Option<String>,
        source_digest: Option<String>,
    ) -> Result<Self> {
        let (app_slug, short_commit) = parse_release_tag(release_tag)?;
        let backend_url = backend_url.trim().trim_end_matches('/').to_string();
        if backend_url.is_empty() {
            bail!("backend URL is required via --backend-url or {BACKEND_URL_ENV}");
        }
        if activation_token.trim().is_empty() {
            bail!("activation token is required via --activation-token or {ACTIVATION_TOKEN_ENV}");
        }
        let source_repo = source_repo.trim().to_string();
        if source_repo.is_empty() {
            bail!(
                "source_repo is required — pass --source-repo or ensure aomi.toml declares [app].git"
            );
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
