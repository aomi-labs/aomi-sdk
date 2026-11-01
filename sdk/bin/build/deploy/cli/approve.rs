//! `approve` — an Aomi approver approves the exact app libraries of a
//! deployment a builder promoted, which runs the activation that was waiting
//! for review. Only approvers may call it; Build checks their GitHub login.

use anyhow::{Result, bail};
use clap::Args;

use crate::deploy::session::Session;
use crate::deploy::types::BuildApproveInput;

#[derive(Debug, Args, Clone)]
pub struct ApproveArgs {
    /// The promoted deployment (`dep_…`), as shown on its Build page.
    #[arg(required_unless_present = "grandfather")]
    pub deployment: Option<String>,

    /// Platform the deployment belongs to. Defaults to Build's default.
    #[arg(long)]
    pub platform: Option<String>,

    /// Approve only these apps of the deployment (repeatable). Default: all.
    #[arg(long = "app", value_name = "APP")]
    pub apps: Vec<String>,

    /// Approve the library every live hosted app serves now. Run once, just
    /// before the backend's load gate is switched on.
    #[arg(long, conflicts_with = "deployment")]
    pub grandfather: bool,

    /// Backend base URL (default: `AOMI_BACKEND_URL`); picks the Build URL.
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Aomi Build URL (default: `AOMI_BUILD_URL` or the saved login).
    #[arg(long = "build-url", value_name = "URL")]
    pub build_url: Option<String>,
}

impl ApproveArgs {
    pub async fn run(self) -> Result<()> {
        let session = Session::open(&self.backend, &self.build_url).await?;
        let response = if self.grandfather {
            session.client.grandfather_approvals().await?
        } else {
            session
                .client
                .approve(&BuildApproveInput {
                    deployment_id: self.deployment.unwrap_or_default(),
                    platform: self.platform,
                    apps: self.apps,
                })
                .await?
        };
        println!("{}", serde_json::to_string_pretty(&response)?);
        if !self.grandfather {
            return Ok(());
        }
        let failed = response["apps"].as_array().map_or(0, |apps| {
            apps.iter().filter(|app| app.get("error").is_some()).count()
        });
        if failed > 0 {
            bail!("{failed} app(s) were not grandfathered; fix them before switching the gate on");
        }
        Ok(())
    }
}
