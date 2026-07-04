//! `activate` — activate platform releases by release tag.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use clap::Args;

use super::shared::{
    ACTIVATION_TOKEN_ENV, BACKEND_URL_ENV, bin_name, clean_list, git_context,
    resolve_activation_token, resolve_backend,
};
use crate::deploy::backend::BackendClient;
use crate::deploy::platform::Platform;
use crate::deploy::types::{ActivateInput, LocalDeployment, ReleaseTags};

#[derive(Debug, Args, Clone, Default)]
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

    /// Rewrite Cargo.toml/Cargo.lock to the backend-required aomi-sdk version
    /// before activating when a mismatch is detected.
    #[arg(long)]
    pub fix_sdk: bool,
}

impl ActivateArgs {
    pub async fn run(self) -> Result<()> {
        let (git_root, start_dir) = git_context(&self.path)?;
        let mut state = LocalDeployment::read(&git_root)?.ok_or_else(|| {
            anyhow!(
                "no .aomi/deployment.json at {} — run `{} deploy` first",
                git_root.display(),
                bin_name()
            )
        })?;
        if self.dry_run {
            let platform = self
                .platform
                .clone()
                .unwrap_or_else(|| Platform::new(&state.deployment.platform.platform));
            let printable = serde_json::json!({
                "endpoint": format!("/api/platforms/{}/apps/activate", platform.as_str()),
                "request": self.activation_request(&state)?,
            });
            println!("{}", serde_json::to_string_pretty(&printable)?);
            return Ok(());
        }

        let response = self.activate_with_state(&start_dir, &mut state).await?;
        state.write(&git_root)?;
        Self::print_activation(&response, self.json)?;
        Ok(())
    }

    pub(crate) async fn activate_with_state(
        &self,
        git_root: &std::path::Path,
        state: &mut LocalDeployment,
    ) -> Result<crate::deploy::types::ActivateResult> {
        let platform = self
            .platform
            .clone()
            .unwrap_or_else(|| Platform::new(&state.deployment.platform.platform));

        let request = self.activation_request(state)?;

        let backend_url = resolve_backend(&self.backend).ok_or_else(|| {
            anyhow!("activate needs a backend URL — set --backend or {BACKEND_URL_ENV}")
        })?;
        crate::sdk_guard::ensure_project_sdk(git_root, Some(&backend_url), self.fix_sdk).await?;
        let token = resolve_activation_token(&self.activation_token).ok_or_else(|| {
            anyhow!(
                "activate requires a token via --activation-token or {ACTIVATION_TOKEN_ENV} \
                 (or run `aomi-build connect`)"
            )
        })?;
        let client = BackendClient::new(backend_url, token)?;

        // One call activates every requested app; the response carries per-app
        // results with a partial-failure shape.
        let mut response = client.activate(&platform, &request).await?;
        verify_activation(&client, &platform, &mut response).await?;
        state.apply_target_activation(&response);
        Ok(response)
    }

    pub(crate) fn print_activation(
        response: &crate::deploy::types::ActivateResult,
        json: bool,
    ) -> Result<()> {
        if json {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            for app in &response.activation.apps {
                match &app.error {
                    Some(err) => println!("  - {} : FAILED ({err})", app.name),
                    None => println!(
                        "  - {} : active={} artifact_ready={} loaded={}",
                        app.name, app.is_active, app.artifact_ready, app.loaded
                    ),
                }
            }
        }

        let failures = response
            .activation
            .apps
            .iter()
            .filter(|a| a.error.is_some() || !a.is_active || !a.artifact_ready || !a.loaded)
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

async fn verify_activation(
    client: &BackendClient,
    platform: &Platform,
    response: &mut crate::deploy::types::ActivateResult,
) -> Result<()> {
    for app in &mut response.activation.apps {
        if app.error.is_some() {
            continue;
        }
        let release_tag = app
            .release_tag
            .as_deref()
            .ok_or_else(|| anyhow!("activation response for `{}` omitted release_tag", app.name))?;
        let started = Instant::now();
        loop {
            let live = match client.get_app(platform, &app.name, release_tag).await {
                Ok(live) => live,
                Err(err) => {
                    if started.elapsed() >= Duration::from_secs(5 * 60) {
                        return Err(anyhow!(
                            "activation verification failed for `{}`: {err}",
                            app.name
                        ));
                    }
                    tokio::time::sleep(Duration::from_secs(6)).await;
                    continue;
                }
            };
            let live_tag = live.app.app_release_tag.as_deref().unwrap_or("");
            if !live_tag.is_empty() && live_tag != release_tag {
                bail!(
                    "activation verification failed for `{}`: backend has release tag `{live_tag}`, expected `{release_tag}`",
                    app.name
                );
            }
            (app.is_active, app.artifact_ready, app.loaded) =
                (live.app.is_active, live.app.artifact_ready, live.app.loaded);
            if app.is_active && app.artifact_ready && app.loaded {
                break;
            }
            if started.elapsed() >= Duration::from_secs(5 * 60) {
                bail!(
                    "activation verification timed out for `{}` ({release_tag}): active={} artifact_ready={} loaded={}",
                    app.name,
                    app.is_active,
                    app.artifact_ready,
                    app.loaded
                );
            }
            tokio::time::sleep(Duration::from_secs(6)).await;
        }
    }
    Ok(())
}
