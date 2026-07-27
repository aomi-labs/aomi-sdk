//! `activate` — activate platform releases by release tag.

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::Args;

use super::login::ensure_logged_in;
use super::shared::{
    ACTIVATION_TOKEN_ENV, BACKEND_URL_ENV, BUILD_TOKEN_ENV, BUILD_URL_ENV, bin_name, env_value,
    git_context, resolve_backend, resolve_build_token, resolve_build_url,
};
use crate::deploy::backend::BackendClient;
use crate::deploy::build_client::BuildClient;
use crate::deploy::platform::Platform;
use crate::deploy::types::{ActivateInput, BuildActivateInput, LocalDeployment, ReleaseTags};

pub async fn run(args: ActivateArgs) -> eyre::Result<()> {
    args.run().await.map_err(crate::git_error)
}

#[derive(Debug, Args, Clone)]
pub struct ActivateArgs {
    /// Apps to activate. Defaults to every app from `.aomi/deployment.json`.
    #[arg(value_name = "APP")]
    pub apps: Vec<String>,

    /// Platform tag. Defaults to `.aomi/deployment.json`, then `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// Activate explicit release tags. Repeat for multi-app activation. App
    /// names are optional; when provided, their count must match the tags.
    /// Defaults to the release tags recorded in `.aomi/deployment.json`.
    #[arg(long = "release-tag", value_name = "TAG")]
    pub release_tags: Vec<String>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Aomi Build URL for Builder-authenticated activation.
    #[arg(long = "build-url", value_name = "URL")]
    pub build_url: Option<String>,

    /// Activation token (default: `AOMI_APP_ACTIVATION_TOKEN`).
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// Required backend server tag (repeatable).
    #[arg(long = "target-tag", value_name = "TAG")]
    pub target_tags: Vec<String>,

    /// Source repo path for the `.aomi/deployment.json` lookup.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Print the planned request without sending it.
    #[arg(long)]
    pub dry_run: bool,

    /// Print the backend response as JSON.
    #[arg(long)]
    pub json: bool,
}

impl ActivateArgs {
    pub async fn run(self) -> Result<()> {
        let (git_root, _) = git_context(&self.path)?;
        let mut state = LocalDeployment::read(&git_root)?.ok_or_else(|| {
            anyhow!(
                "no .aomi/deployment.json at {} — run `{} deploy` first",
                git_root.display(),
                bin_name()
            )
        })?;
        let platform = self
            .platform
            .clone()
            .unwrap_or_else(|| Platform::new(&state.deployment.platform.platform));

        let request = self.activation_request(&state)?;

        if self.dry_run {
            let printable = serde_json::json!({
                "endpoint": format!("/api/platforms/{}/apps/activate", platform.as_str()),
                "request": request,
            });
            println!("{}", serde_json::to_string_pretty(&printable)?);
            return Ok(());
        }

        let backend_url = resolve_backend(&self.backend);
        let explicit_activation_token = self
            .activation_token
            .clone()
            .or_else(|| env_value(ACTIVATION_TOKEN_ENV));
        let response = if let Some(token) = explicit_activation_token {
            // Explicit headless/admin compatibility path. Saved activation
            // tokens are deliberately not used for interactive human deploys.
            let backend_url = backend_url.ok_or_else(|| {
                anyhow!("activate needs a backend URL — set --backend or {BACKEND_URL_ENV}")
            })?;
            BackendClient::new(backend_url, token)?
                .activate(&platform, &request)
                .await?
        } else {
            let build_url =
                resolve_build_url(&self.build_url, backend_url.as_deref()).ok_or_else(|| {
                    anyhow!("activate needs an Aomi Build URL — set --build-url or {BUILD_URL_ENV}")
                })?;
            ensure_logged_in(&build_url).await?;
            let token = resolve_build_token().ok_or_else(|| {
                anyhow!(
                    "activate requires a Builder login (or set {BUILD_TOKEN_ENV} for headless Builder automation)"
                )
            })?;
            let app_source_id = state.app_source_id().ok_or_else(|| {
                anyhow!("deployment has no app_source_id; deploy again while logged in")
            })?;
            BuildClient::new(build_url, token)?
                .activate(&BuildActivateInput {
                    platform: platform.to_string(),
                    app_source_id,
                    release_tags: request.target.value.clone(),
                    apps: request.apps.clone(),
                })
                .await?
        };
        state.apply_target_activation(&response);
        state.write(&git_root)?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            for app in &response.activation.apps {
                match &app.error {
                    Some(err) => println!("  - {} : FAILED ({err})", app.name),
                    None => println!(
                        "  - {} : active={} loaded={}",
                        app.name, app.is_active, app.loaded
                    ),
                }
            }
        }

        let failures = response
            .activation
            .apps
            .iter()
            .filter(|a| a.error.is_some() || !a.loaded)
            .count();
        if failures > 0 {
            bail!("{failures} app(s) failed to activate");
        }
        Ok(())
    }

    pub(crate) fn activation_request(&self, state: &LocalDeployment) -> Result<ActivateInput> {
        let explicit_release_tags = clean_list(&self.release_tags);
        let has_explicit_release_tags = !explicit_release_tags.is_empty();
        let app_names = if has_explicit_release_tags && self.apps.is_empty() {
            Vec::new()
        } else if self.apps.is_empty() {
            state.app_names()
        } else {
            clean_list(&self.apps)
        };

        if has_explicit_release_tags
            && !app_names.is_empty()
            && app_names.len() != explicit_release_tags.len()
        {
            bail!("--release-tag activation requires the same number of apps and release tags");
        }

        let release_tags = if has_explicit_release_tags {
            explicit_release_tags
        } else {
            if app_names.is_empty() {
                bail!("no apps to activate — deploy first or pass --release-tag");
            }
            app_names
                .iter()
                .map(|app| {
                    state
                        .release_tag_for(app)
                        .map(str::to_string)
                        .ok_or_else(|| {
                            anyhow!("app `{app}` is not in .aomi/deployment.json; deploy it first")
                        })
                })
                .collect::<Result<Vec<_>>>()?
        };

        if release_tags.is_empty() {
            bail!("activation needs at least one release tag");
        }

        Ok(ActivateInput {
            target: ReleaseTags::new(release_tags),
            apps: app_names,
            target_tags: clean_list(&self.target_tags),
        })
    }
}

fn clean_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}
