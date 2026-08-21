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
        let (git_root, _) = git_context(&self.path)?;
        let state = LocalDeployment::read(&git_root)?.ok_or_else(|| {
            anyhow!(
                "no .aomi/deployment.json at {} — run `{} deploy` first",
                git_root.display(),
                bin_name()
            )
        })?;

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
            let status = Session::at(&build_url, None)
                .await?
                .client
                .status(&state.deployment.platform.platform, &state.deployment.id)
                .await?;
            report.deployment = DeploymentBackendStatus::Found {
                state: status.state,
                message: status.message,
                ci_url: status.ci.and_then(|ci| ci.url),
            };
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
