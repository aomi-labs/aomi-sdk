use std::fmt;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::backend::BackendClient;
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

pub(crate) fn parse_app_release_tag(value: &str) -> Result<(String, String)> {
    let Some(stripped) = value.strip_prefix("apps-") else {
        bail!("app_release_tag must start with `apps-`");
    };
    let Some((app_slug, short_commit)) = stripped.rsplit_once('-') else {
        bail!("app_release_tag must follow apps-{{app_slug}}-{{short_commit}}");
    };
    if app_slug.is_empty() || short_commit.is_empty() {
        bail!("app_release_tag must include app slug and short commit");
    }
    if !app_slug
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        bail!("app_release_tag app slug contains unsupported characters");
    }
    if !short_commit.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("app_release_tag short commit must be hexadecimal");
    }
    Ok((app_slug.to_string(), short_commit.to_string()))
}

#[derive(Clone, Debug)]
pub struct ActivationPlan {
    backend: BackendClient,
    pub request: ActivateAppRequest,
}

impl ActivationPlan {
    pub fn from_intent(
        backend_url: String,
        activation_token: String,
        intent: ActivationIntent,
    ) -> Result<Self> {
        Ok(Self {
            backend: BackendClient::new(backend_url, activation_token).map_err(|error| {
                rewrite_backend_error(error, "activation token", ACTIVATION_TOKEN_ENV)
            })?,
            request: intent.into_request()?,
        })
    }

    pub fn endpoint(&self) -> String {
        self.backend.endpoint(ACTIVATION_PATH)
    }

    pub async fn execute(&self) -> Result<Value> {
        self.backend
            .post_json(ACTIVATION_PATH, &self.request, "activation")
            .await
    }
}

#[derive(Clone, Debug)]
pub struct ActivationIntent {
    app_release_tag: String,
    platform: Platform,
    visibility: Visibility,
    source_repo: String,
    github_token: Option<String>,
    server_tags: Vec<String>,
    label: Option<String>,
    source_commit: Option<String>,
    source_tree: Option<String>,
    source_digest: Option<String>,
}

impl ActivationIntent {
    pub fn new(app_release_tag: &str, platform: Platform, visibility: Visibility) -> Result<Self> {
        parse_app_release_tag(app_release_tag)?;
        Ok(Self {
            app_release_tag: app_release_tag.to_string(),
            platform,
            visibility,
            source_repo: String::new(),
            github_token: None,
            server_tags: Vec::new(),
            label: None,
            source_commit: None,
            source_tree: None,
            source_digest: None,
        })
    }

    pub fn source_repo(mut self, value: String) -> Self {
        self.source_repo = value;
        self
    }

    pub fn github_token(mut self, value: Option<String>) -> Self {
        self.github_token = value;
        self
    }

    pub fn server_tags(mut self, value: Vec<String>) -> Self {
        self.server_tags = value;
        self
    }

    pub fn label(mut self, value: Option<String>) -> Self {
        self.label = value;
        self
    }

    pub fn source_provenance(
        mut self,
        commit: Option<String>,
        tree: Option<String>,
        digest: Option<String>,
    ) -> Self {
        self.source_commit = commit;
        self.source_tree = tree;
        self.source_digest = digest;
        self
    }

    fn into_request(self) -> Result<ActivateAppRequest> {
        let (app_slug, short_commit) = parse_app_release_tag(&self.app_release_tag)?;
        let app_release_tag = self.app_release_tag;
        let source_repo = self.source_repo.trim().to_string();
        if source_repo.is_empty() {
            bail!(
                "source_repo is required - pass --source-repo or ensure aomi.toml declares [app].git"
            );
        }

        Ok(ActivateAppRequest {
            name: app_slug,
            label: self.label.and_then(non_empty),
            platform: self.platform,
            source_repo,
            app_release_tag: app_release_tag.clone(),
            source_commit: self.source_commit.and_then(non_empty),
            source_tree: self.source_tree.and_then(non_empty),
            source_digest: self.source_digest.and_then(non_empty),
            is_active: true,
            is_public: self.visibility.is_public(),
            metadata: json!({
                "requested_by": "aomi-git",
                "app_release_tag": app_release_tag,
                "short_commit": short_commit,
            }),
            github_token: self.github_token.and_then(non_empty),
            server_tags: normalize_tags(self.server_tags)?,
        })
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
    /// Required backend server tags. The backend loads only when these tags are
    /// a subset of its configured AOMI_SERVER_TAGS.
    #[serde(default, rename = "target_tags", skip_serializing_if = "Vec::is_empty")]
    pub server_tags: Vec<String>,
    /// Transient GitHub read token resolved from `aomi.toml[app].access_token`.
    /// Sent once, used once by the backend, never persisted.
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

fn rewrite_backend_error(error: anyhow::Error, token_name: &str, token_env: &str) -> anyhow::Error {
    match error.to_string().as_str() {
        "backend URL is required" => {
            anyhow::anyhow!("backend URL is required via --backend or {BACKEND_URL_ENV}")
        }
        "backend bearer token is required" => {
            anyhow::anyhow!("{token_name} is required via --activation-token or {token_env}")
        }
        _ => error,
    }
}
