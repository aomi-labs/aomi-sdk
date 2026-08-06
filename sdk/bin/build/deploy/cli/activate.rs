//! `activate` — activate platform releases by release tag.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use clap::Args;

use super::shared::{
    ACTIVATION_TOKEN_ENV, BACKEND_URL_ENV, bin_name, clean_list, env_value, git_context,
    resolve_backend,
};
use crate::deploy::backend::BackendClient;
use crate::deploy::build_client::BuildClient;
use crate::deploy::platform::Platform;
use crate::deploy::session::Session;
use crate::deploy::state::LocalDeployment;
use crate::deploy::types::{ActivateInput, BuildActivateInput, ReleaseTags};

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

    /// Rewrite Cargo.toml/Cargo.lock to the backend-required aomi-sdk version
    /// before activating when a mismatch is detected.
    #[arg(long)]
    pub fix_sdk: bool,
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

        let response = self.activate_with_state(&git_root, &mut state).await?;
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

        let backend_url = resolve_backend(&self.backend);
        crate::sdk_guard::ensure_project_sdk(git_root, backend_url.as_deref(), self.fix_sdk)
            .await?;
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
            let client = BackendClient::new(backend_url, token)?;
            let mut response = client.activate(&platform, &request).await?;
            verify_activation(&client, &platform, &mut response).await?;
            response
        } else {
            let session = Session::open(&self.backend, &self.build_url).await?;
            let app_source_id = state.app_source_id().ok_or_else(|| {
                anyhow!("deployment has no app_source_id; deploy again while logged in")
            })?;
            activate_until_loaded(
                &session.client,
                &BuildActivateInput {
                    platform: platform.to_string(),
                    project_id: app_source_id,
                    release_tags: request.target.value.clone(),
                    apps: request.apps.clone(),
                    // Was silently dropped here, so `--target-tag` was accepted
                    // and ignored on every Builder (i.e. every human) activation.
                    target_tags: request.target_tags.clone(),
                },
            )
            .await?
        };
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

/// How long to let the backend finish loading a freshly activated artifact.
/// Mirrors [`verify_activation`]'s budget on the activation-token path.
const ACTIVATION_SETTLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// Activate through the Builder BFF and wait for the result to actually settle.
///
/// The `is_active` / `artifact_ready` / `loaded` flags in an activation response
/// are a snapshot taken as activation is requested, and the backend loads the
/// artifact asynchronously — which is exactly why the activation-token path polls
/// `get_app` for five minutes before believing them. The Builder path had no such
/// wait, so it reported `N app(s) failed to activate` on deploys that had in fact
/// succeeded, and re-running `activate` "fixed" it.
///
/// There is no Builder-side read endpoint for per-app state, so this re-issues the
/// activation instead. Release-tag activation is declarative — "make these tags the
/// active ones" — so repeating it is what the portal does when a user clicks
/// Activate twice, not a second distinct mutation.
async fn activate_until_loaded(
    client: &BuildClient,
    input: &BuildActivateInput,
) -> Result<crate::deploy::types::ActivateResult> {
    let started = Instant::now();
    let mut announced = false;
    loop {
        let response = client.activate(input).await?;
        let apps = &response.activation.apps;
        // A hard per-app error is terminal; retrying only delays the report.
        let settled = apps.iter().any(|app| app.error.is_some())
            || apps
                .iter()
                .all(|app| app.is_active && app.artifact_ready && app.loaded);
        // On timeout, return the last response as-is so `print_activation`
        // names exactly which flags are still false.
        if settled || started.elapsed() >= ACTIVATION_SETTLE_TIMEOUT {
            return Ok(response);
        }
        if !announced {
            println!("  waiting for the backend to load the activated release…");
            announced = true;
        }
        tokio::time::sleep(Duration::from_secs(6)).await;
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
