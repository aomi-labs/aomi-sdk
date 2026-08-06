//! `connect` — install the Aomi GitHub App and save your activation token.

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;

use super::login;
use super::shared::{
    BACKEND_URL_ENV, BUILD_URL_ENV, resolve_activation_token, resolve_backend, resolve_build_url,
};
use crate::deploy::backend::BackendClient;
use crate::deploy::config::AomiConfig;
use crate::deploy::flow::{TokenCheck, oauth_install_url, validate_activation_token};
use crate::deploy::platform::{Platform, normalize_github_repo};
use crate::deploy::types::SyncSourceInput;

pub async fn run(args: ConnectArgs) -> eyre::Result<()> {
    args.run().await.map_err(crate::git_error)
}

#[derive(Debug, Args, Clone)]
pub struct ConnectArgs {
    /// Platform to connect for (scopes the install + token check).
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,

    /// Optionally scope the GitHub App install to a single repo.
    #[arg(long, value_name = "OWNER/REPO")]
    pub repo: Option<String>,

    /// Connected GitHub App installation id. Prompted if omitted (the value the
    /// portal redirect shows after you install).
    #[arg(long = "installation-id", value_name = "ID")]
    pub installation_id: Option<i64>,

    /// Backend base URL (default: AOMI_BACKEND_URL, then saved config).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Aomi Build URL used to authenticate the Builder that owns a selected
    /// `--repo`. Known staging/production URLs are inferred from `--backend`.
    #[arg(long = "build-url", value_name = "URL")]
    pub build_url: Option<String>,

    /// Activation token to store (issued by your Aomi admin). Prompted if omitted.
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// Re-consent an App that's already installed (portal's "verify existing
    /// install") instead of a fresh install.
    #[arg(long)]
    pub authorize: bool,

    /// Print the install URL without opening a browser.
    #[arg(long)]
    pub no_browser: bool,
}

fn prompt_installation_id() -> Result<i64> {
    inquire::Text::new("Paste your installation_id (the number in the URL GitHub sent you to):")
        .prompt()
        .context("connect cancelled")?
        .trim()
        .parse()
        .context("installation_id must be a number")
}

impl ConnectArgs {
    pub async fn run(self) -> Result<()> {
        let backend_url = resolve_backend(&self.backend).ok_or_else(|| {
            anyhow!("connect needs a backend URL — set --backend or {BACKEND_URL_ENV}")
        })?;
        let repo = match &self.repo {
            Some(r) => Some(normalize_github_repo(r)?),
            None => None,
        };
        let build_url = repo
            .as_ref()
            .map(|_| {
                resolve_build_url(&self.build_url, Some(&backend_url)).ok_or_else(|| {
                    anyhow!(
                        "connect --repo needs an Aomi Build URL to verify ownership; set --build-url or {BUILD_URL_ENV}"
                    )
                })
            })
            .transpose()?;

        // 1. GitHub App install URL. After the user installs, GitHub redirects
        //    *their browser* (not this CLI) to the App's configured callback,
        //    whose URL carries `installation_id` — so we can't poll for it; the
        //    user reads it off that page. `mode=authorize` re-consents an
        //    existing install; the default App (#1) needs no `app` param.
        let mode = if self.authorize {
            "authorize"
        } else {
            "install"
        };
        let start =
            oauth_install_url(&backend_url, self.platform.as_str(), repo.as_deref(), mode).await?;
        println!("Connect your source by installing the Aomi GitHub App:");
        println!("  {}", start.install_url);
        if self.no_browser {
            println!("  (open the URL above to install)");
        } else if let Err(e) = open::that(&start.install_url) {
            eprintln!("  (couldn't open a browser automatically: {e})");
            println!("  open the URL above to install.");
        } else {
            println!("  (opened in your browser)");
        }
        println!(
            "  After installing, GitHub returns you to a page whose URL ends in\n  \
             `installation_id=NNN` — that number is what you paste below."
        );
        println!();

        // 2. Capture the installation id: explicit flag, else paste it off the
        //    redirect URL.
        let installation_id: i64 = match self.installation_id {
            Some(id) => id,
            None => prompt_installation_id()?,
        };

        // 3. Activation token (issued by your Aomi admin).
        let token = match resolve_activation_token(&self.activation_token) {
            Some(t) => t,
            None => inquire::Password::new("Paste your activation token (from your Aomi admin):")
                .without_confirmation()
                .prompt()
                .context("connect cancelled")?
                .trim()
                .to_string(),
        };
        if token.is_empty() {
            bail!("an activation token is required to connect");
        }
        match validate_activation_token(&backend_url, &token, self.platform.as_str()).await {
            TokenCheck::Valid => println!("  token verified for platform `{}`", self.platform),
            TokenCheck::Invalid => println!(
                "  warning: that token was rejected for `{}` — saving anyway",
                self.platform
            ),
            TokenCheck::Unreachable => {
                println!("  couldn't reach the backend to verify the token — saving anyway")
            }
        }

        // 4. Authenticate the Builder. When connect names a repository, claim
        // it now instead of leaving an operator-only owner_builder_id repair
        // for the first deploy.
        let claimed = match (repo.as_ref(), build_url.as_deref()) {
            (Some(repo), Some(build_url)) => {
                let builder =
                    login::ensure_logged_in_with_options(build_url, self.no_browser).await?;
                let github_user_id =
                    login::verified_builder_github_user_id(&builder.status.github_user_id)?;
                let source = BackendClient::new(backend_url.clone(), token.clone())?
                    .sync_installed(
                        &self.platform,
                        &SyncSourceInput {
                            repo: repo.clone(),
                            github_user_id,
                        },
                    )
                    .await?;
                Some((builder.status, source))
            }
            _ => None,
        };

        // 5. Persist account and platform credentials. `ensure_logged_in`
        // saved the Builder session first, so load it again before merging the
        // connect fields rather than overwriting that identity.
        let mut config = AomiConfig::load();
        config.backend_url = Some(backend_url);
        if let Some(build_url) = build_url {
            config.build_url = Some(build_url);
        }
        config.platform = Some(self.platform.to_string());
        config.installation_id = Some(installation_id);
        config.activation_token = Some(token);
        let path = config.save()?;

        println!();
        println!("Connected. Saved to {}", path.display());
        println!("  installation_id: {installation_id}");
        if let Some((builder, source)) = claimed {
            println!("  Builder: @{}", builder.github_login);
            println!(
                "  claimed source: {} (app_source_id {})",
                source.source.repository_link, source.source.id
            );
        }
        println!();
        println!("Next:");
        println!(
            "  aomi-build scaffold --repo-name <name> --installation-id {installation_id} --platform {}",
            self.platform
        );
        println!("  # or, from an existing source repo:");
        println!(
            "  aomi-build source sync --repo <owner/repo> --platform {}",
            self.platform
        );
        Ok(())
    }
}
