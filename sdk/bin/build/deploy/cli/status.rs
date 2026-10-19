//! `status` — local `deployment.json` + backend per-app state.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Args;

use super::shared::{
    bin_name, git_context, resolve_activation_token, resolve_backend, resolve_build_url,
};
use crate::deploy::session::Session;
use crate::deploy::state::LocalDeployment;
use crate::deploy::status::{DeploymentBackendStatus, StatusResult};

#[derive(Debug, Args, Clone)]
pub struct StatusArgs {
    #[command(flatten)]
    pub selector: super::selector::DeploymentSelector,
    /// Backend base URL (default: `AOMI_BACKEND_URL`). Pass `--backend ''` to
    /// skip the backend probe.
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Aomi Build URL. Defaults to `AOMI_BUILD_URL`, saved login config, or
    /// the known staging/production URL associated with the backend.
    #[arg(long = "build-url", value_name = "URL")]
    pub build_url: Option<String>,

    /// Source repo path for the `.aomi/deployment.json` lookup.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Activation token for backend app status checks.
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// Print the status report as JSON.
    #[arg(long)]
    pub json: bool,
}

impl StatusArgs {
    pub async fn run(self) -> Result<()> {
        let local = git_context(&self.path)
            .ok()
            .map(|(root, _)| LocalDeployment::read(&root))
            .transpose()?
            .flatten();
        let state = if self.selector.explicit() || local.is_none() {
            self.selector
                .resolve(&self.path, &self.backend, &self.build_url)
                .await?
                .1
        } else {
            local.ok_or_else(|| {
                anyhow!(
                    "no .aomi/deployment.json at {} — run `{} deploy` first",
                    self.path.display(),
                    bin_name()
                )
            })?
        };

        // `--backend ''` explicitly opts out; otherwise flag/env.
        let backend_url = match &self.backend {
            Some(flag) if flag.trim().is_empty() => None,
            other => resolve_backend(other),
        };
        let token = backend_url
            .as_ref()
            .and_then(|_| resolve_activation_token(&self.activation_token));

        let build_url = if matches!(&self.backend, Some(flag) if flag.trim().is_empty())
            && self.build_url.is_none()
        {
            None
        } else {
            resolve_build_url(&self.build_url, backend_url.as_deref())
        };
        let mut report = StatusResult::collect(&state, backend_url, token).await;
        if let Some(build_url) = build_url {
            let session = Session::at(&build_url, None).await?;
            let status = session
                .client
                .status(&state.deployment.platform.platform, &state.deployment.id)
                .await?;
            if let Some(manifest) = &status.deployment {
                if manifest.source.repository_id != state.deployment.source.repository_id
                    || manifest.source.commit_hash != state.deployment.source.commit_hash
                {
                    anyhow::bail!(
                        "local deployment cache does not match this environment; select --project-id and --commit explicitly"
                    );
                }
            }
            report.deployment = DeploymentBackendStatus::Found {
                state: status.state,
                message: status.message,
                ci_url: status.ci.and_then(|ci| ci.url),
            };
            let live = session
                .client
                .get_json(
                    "/api/bff/launch/apps",
                    &[("projectId", state.project_id.to_string())],
                )
                .await?;
            for app in &mut report.apps {
                if let Some(row) = live["apps"]
                    .as_array()
                    .and_then(|rows| rows.iter().find(|row| row["name"] == app.name))
                {
                    app.application_id = row["id"].as_i64();
                    let matches = row["app_release_tag"].as_str() == Some(app.release_tag.as_str());
                    let loaded = matches && row["loaded"].as_bool() == Some(true);
                    app.backend = crate::deploy::status::BackendAppStatus::Found {
                        is_active: matches && row["is_active"].as_bool() == Some(true),
                        artifact_ready: loaded.then_some(true),
                        loaded,
                    };
                }
            }
            report.activated = !report.apps.is_empty()
                && report.apps.iter().all(|app| {
                    matches!(
                        app.backend,
                        crate::deploy::status::BackendAppStatus::Found {
                            is_active: true,
                            loaded: true,
                            ..
                        }
                    )
                });
        }
        if self.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print!("{}", report.render());
            // The reason people run `status` mid-deploy is to find out whether
            // they can proceed — say what proceeding looks like.
            if !report.activated {
                println!(
                    "Resume: {} activate --path {}",
                    bin_name(),
                    self.path.display()
                );
            }
        }
        Ok(())
    }
}
