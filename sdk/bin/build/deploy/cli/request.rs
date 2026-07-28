//! `request` — legacy ops onboarding request (Discord).

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::Args;

use super::shared::{git_context, remote_origin};
use crate::deploy::app::AomiAppFiles;
use crate::deploy::discord;
use crate::deploy::platform::{Platform, normalize_github_repo};

#[derive(Debug, Args, Clone)]
pub struct RequestArgs {
    /// Email where platform ops will send activation details.
    #[arg(long, value_name = "EMAIL")]
    pub email: String,

    /// GitHub account for source ownership context.
    #[arg(long = "git-account", value_name = "USER")]
    pub git_account: String,

    /// App slug (`aomi.toml [app].name`). Defaults to aomi.toml.
    #[arg(long, value_name = "NAME")]
    pub app: Option<String>,

    /// Platform tag. Falls back to aomi.toml, then `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// App source directory for the aomi.toml lookup.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Print the Discord message without posting it.
    #[arg(long)]
    pub dry_run: bool,
}

impl RequestArgs {
    pub async fn run(self) -> Result<()> {
        let email = self.email.trim();
        if email.is_empty() || !email.contains('@') {
            bail!(
                "`--email` must be a valid email address (got {:?})",
                self.email
            );
        }
        let git_account = self.git_account.trim();
        if git_account.is_empty() {
            bail!("`--git-account` must not be empty");
        }

        let git = git_context(&self.path).ok();
        let app_cfg = git
            .as_ref()
            .and_then(|(root, start)| AomiAppFiles::discover(start, root).ok());

        let app = self
            .app
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| app_cfg.as_ref().map(|a| a.name.clone()))
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "app slug is unknown - pass --app or run from a source repo whose \
                     aomi.toml declares [app].name"
                )
            })?;

        let platform = self
            .platform
            .as_ref()
            .map(|p| p.to_string())
            .or_else(|| app_cfg.as_ref().and_then(|a| a.platform.clone()))
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "community".to_string());

        let repo_slug = app_cfg
            .as_ref()
            .and_then(|a| a.git.clone())
            .or_else(|| git.as_ref().and_then(|(root, _)| remote_origin(root).ok()))
            .map(|raw| normalize_github_repo(&raw))
            .transpose()?
            .ok_or_else(|| {
                anyhow!("source repo is unknown - run from a source repo with a GitHub origin")
            })?;

        let request = discord::ActivationRequest {
            email: email.to_string(),
            git_account: git_account.to_string(),
            app,
            platform,
            repo: repo_slug,
        };

        if self.dry_run {
            println!("{}", serde_json::to_string_pretty(&request.webhook_body())?);
            println!("\n(dry-run: not posted to Discord)");
            return Ok(());
        }

        request.post().await?;
        println!(
            "Posted onboarding request for `{}` to the Aomi apps Discord.",
            request.app
        );
        println!(
            "Join the Aomi apps Discord if needed: {}",
            discord::DISCORD_INVITE
        );
        Ok(())
    }
}
