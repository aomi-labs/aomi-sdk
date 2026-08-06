//! `status` — local `deployment.json` + backend per-app state.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use clap::Args;

use super::login::ensure_logged_in;
use super::shared::{
    bin_name, git_context, resolve_activation_token, resolve_backend, resolve_build_url,
};
use crate::deploy::status::StatusResult;
use crate::deploy::types::LocalDeployment;

pub async fn run(args: StatusArgs) -> eyre::Result<()> {
    args.run().await.map_err(crate::to_eyre)
}

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

        // Keep `--backend ''` as a complete remote-status opt-out unless the
        // caller explicitly asks for an Aomi Build endpoint.
        let build_url = if self
            .backend
            .as_deref()
            .is_some_and(|backend| backend.trim().is_empty())
            && self.build_url.is_none()
        {
            None
        } else {
            resolve_build_url(&self.build_url, backend_url.as_deref())
        };
        let mut report = StatusResult::collect(&state, backend_url, token).await;

        if let Some(build_url) = build_url {
            let build_status = ensure_logged_in(&build_url, false)
                .await?
                .client
                .status(&state.deployment.platform.platform, &state.deployment.id)
                .await?;
            report.build_state = Some(build_status.state);
            report.build_message = build_status.message;
            report.build_logs_url = build_status.ci.and_then(|ci| ci.url);
        }

        if self.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print!("{}", report.render());
        }
        Ok(())
    }
}
