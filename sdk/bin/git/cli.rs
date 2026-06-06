//! `aomi-git` CLI surface.
//!
//! Flag names mirror `aomi.toml [app]` field names so contributors don't have
//! to translate between two vocabularies. Where a CLI concept has no toml
//! equivalent (an escape-hatch local directory, the backend URL, dry-run), the
//! flag is named for what it does on the CLI.
//!
//! ```text
//! deploy
//!   --path <DIR>             # app source directory (default .)
//!   --platform <NAME>        # aomi.toml [app].platform
//!   --source-path <URL>      # GitHub source repo URL (default origin)
//!   --hash <SHA>             # commit hash (default HEAD)
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --dry-run                # plan + best-effort backend reads, no deploy
//!   --activate               # activate after deploy
//!   --allow-dirty
//!   --json
//!
//! activate [APP_RELEASE_TAG] # reservation without a release tag; activation with one
//!   --path <DIR>             # source repo (.aomi/deployment.json fallback)
//!   --platform <NAME>        # aomi.toml [app].platform
//!   --source-path <URL>      # GitHub source repo URL (default origin/state)
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --activation-token <T>   # AOMI_APP_ACTIVATION_TOKEN
//!   --target-tag <TAG>       # aomi.toml [app].server_tags (repeatable)
//!   --visibility <V>         # aomi.toml [app].public
//!   --display-name <STR>     # aomi.toml [app].display_name
//!   --dry-run
//!   --json
//!
//! request                    # legacy ops request
//!   --email <EMAIL>          # where ops sends activation details
//!   --git-account <USER>     # GitHub account for source ownership context
//!   --app <NAME>             # aomi.toml [app].name (default)
//!   --platform <NAME>        # aomi.toml [app].platform (default community)
//!   --path <DIR>             # source repo (aomi.toml lookup)
//!   --dry-run                # print the Discord message; don't post
//!
//! status [APP_RELEASE_TAG]   # or .aomi/deployment.json target.app_release_tag
//!   --path <DIR>             # source repo (.aomi/deployment.json lookup)
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --json
//!
//! ```
//!
//! Defaults pyramid (both commands): CLI flag -> `.aomi/deployment.json` in
//! `--path` -> backend lookup -> hardcoded default. Each step is best-effort -
//! a missing deployment.json or unreachable backend never aborts the plan,
//! only the operation that genuinely needs the unresolved value.

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};
use serde::Serialize;

use crate::activate::{
    ACTIVATION_TOKEN_ENV, ActivationIntent, ActivationPlan, BACKEND_URL_ENV, Visibility,
};
use crate::app::App;
use crate::backend::BackendClient;
use crate::deployment_state::{
    DeploymentState, read as read_deployment_state, write as write_deployment_state,
};
use crate::git::GitRepo;
use crate::plan::Deployment;
use crate::platform::{Platform, normalize_github_repo};

#[derive(Debug, Parser)]
#[command(name = "aomi-git")]
#[command(about = "Deploy Aomi app source through the Aomi backend.")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        match self.command {
            Command::Request(args) => args.run().await,
            Command::Deploy(args) => args.run().await,
            Command::Activate(args) => args.run().await,
            Command::Status(args) => args.run().await,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ask platform ops for legacy onboarding details.
    Request(RequestArgs),
    /// Deploy an Aomi app source commit through the backend.
    Deploy(DeployArgs),
    /// Activate a published Aomi app release in the backend.
    Activate(ActivateArgs),
    /// Check deploy status (CI build + release availability) for a deploy.
    Status(StatusArgs),
}

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct RequestArgs {
    /// Email where platform ops will send activation details.
    #[arg(long, value_name = "EMAIL")]
    pub email: String,

    /// GitHub account for source ownership context.
    #[arg(long = "git-account", value_name = "USER")]
    pub git_account: String,

    /// App slug (`aomi.toml [app].name`). Defaults to the value in aomi.toml.
    #[arg(long, value_name = "NAME")]
    pub app: Option<String>,

    /// Platform tag (`aomi.toml [app].platform`). Falls back to aomi.toml,
    /// then to `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// App source directory (for the `aomi.toml` lookup). Defaults to the
    /// current directory.
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

        // Resolve app/platform/repo from aomi.toml (best-effort: flags win).
        let discovered_repo = GitRepo::discover(&self.path).ok();
        let discovered = discovered_repo
            .as_ref()
            .and_then(|repo| App::discover(repo).ok());

        let app = self
            .app
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| discovered.as_ref().map(|a| a.name.clone()))
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
            .or_else(|| discovered.as_ref().and_then(|a| a.platform.clone()))
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "community".to_string());

        let repo = discovered
            .as_ref()
            .and_then(|a| a.git.clone())
            .or_else(|| {
                discovered_repo
                    .as_ref()
                    .and_then(|repo| repo.remote_origin().ok())
            })
            .map(|raw| normalize_github_repo(&raw))
            .transpose()?
            .ok_or_else(|| {
                anyhow!(
                    "source repo is unknown - run from a source repo with a GitHub origin"
                )
            })?;

        let request = crate::discord::ActivationRequest {
            email: email.to_string(),
            git_account: git_account.to_string(),
            app,
            platform,
            repo,
        };

        if self.dry_run {
            // Show exactly what would be POSTed to the webhook.
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
            "Ops will review `{}` for `{}` and send activation details to {}.",
            request.git_account, request.repo, request.email
        );
        println!(
            "Join the Aomi apps Discord if needed: {}",
            crate::discord::DISCORD_INVITE
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Deploy
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct DeployArgs {
    /// Platform tag (`aomi.toml [app].platform`). Defaults to the value in
    /// aomi.toml, then to `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// GitHub source repo URL. Defaults to local `origin`.
    #[arg(long = "source-path", value_name = "URL|owner/repo")]
    pub source_path: Option<String>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// App source directory. Defaults to the current directory.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Print the deploy plan + best-effort backend reads without deploying or
    /// activating. Refreshes `.aomi/deployment.json` with the
    /// resolved plan and check results.
    #[arg(long)]
    pub dry_run: bool,

    /// After a successful deploy, immediately call the backend activation endpoint
    /// using `AOMI_APP_ACTIVATION_TOKEN`. Normally run `status` first and invoke
    /// `activate` once the release asset exists.
    #[arg(long)]
    pub activate: bool,

    /// Commit hash to deploy. Defaults to local HEAD.
    #[arg(long = "hash", value_name = "SHA")]
    pub hash: Option<String>,

    /// Allow a dirty working tree in the printed plan.
    #[arg(long)]
    pub allow_dirty: bool,

    /// Print the deploy plan / outcome as JSON.
    #[arg(long)]
    pub json: bool,
}

impl DeployArgs {
    async fn run(self) -> Result<()> {
        let app = GitRepo::discover(&self.path)
            .and_then(|repo| App::discover(&repo))
            .ok();
        let platform = self.resolve_platform(app.as_ref());

        if self.dry_run {
            let deployment = Deployment::dry_run(&self.path, platform.clone(), self.allow_dirty)?;
            let mut state = deployment.to_state();

            // Preflight is no longer a separate flag - dry-run always tries
            // online checks if a backend URL is available. Offline still
            // produces a useful plan, just without backend-derived fields.
            if let Some(backend_url) = self.backend_url()
                && let Err(e) = crate::preflight::run(&mut state, &backend_url).await
            {
                state.errors.push(format!("backend preflight skipped: {e}"));
            }

            let state_path = write_deployment_state(&deployment.source.git_root, &state)?;
            if self.json {
                println!("{}", serde_json::to_string_pretty(&state)?);
            } else {
                println!("{}", deployment.render());
                print!("{}", state.render_preflight());
                println!("  deployment_state    : {}", state_path.display());
            }
            return Ok(());
        }

        // Live deploy. The CLI only relays source_path + commit_hash to the
        // backend; it never clones or pushes the platform repo.
        let backend_url = self.backend_url().ok_or_else(|| {
            anyhow!(
                "deploy needs a backend URL — set --backend or {BACKEND_URL_ENV}"
            )
        })?;
        let token = std::env::var(ACTIVATION_TOKEN_ENV)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| anyhow!("deploy requires an app activation token via {ACTIVATION_TOKEN_ENV}"))?;
        let repo = GitRepo::discover(&self.path)?;
        let app = App::discover(&repo)?;
        let source = repo.snapshot(&app.source_path, self.allow_dirty)?;
        let source_path = self
            .source_path
            .clone()
            .or_else(|| repo.remote_origin().ok())
            .ok_or_else(|| anyhow!("deploy needs --source-path or a local git origin remote"))?;
        let commit_hash = self.hash.clone().unwrap_or_else(|| source.commit.clone());
        let source_subdir = if app.source_path.as_os_str().is_empty() {
            ".".to_string()
        } else {
            app.source_path.display().to_string()
        };
        let request = DeployAppRequest {
            source_path: source_path.clone(),
            commit_hash: commit_hash.clone(),
            source_subdir: source_subdir.clone(),
            is_public: app.public.unwrap_or(false),
            server_tags: app.server_tags.clone(),
            label: Some(app.display_name.clone()),
        };
        let endpoint = format!(
            "/api/admin/platforms/{}/apps/{}/deploy",
            platform.as_str(),
            app.name
        );
        let response = BackendClient::new(backend_url.clone(), token)?
            .post_json(&endpoint, &request, "deploy")
            .await?;

        // Refresh .aomi/deployment.json with the post-push state. Start from
        // any prior dry-run state on disk so we don't drop earlier check rows.
        let git_root = repo.root();
        let mut state = match read_deployment_state(git_root) {
            Ok(Some(prior)) => prior,
            _ => Deployment::dry_run(&self.path, platform.clone(), self.allow_dirty)?.to_state(),
        };
        state.source.commit = commit_hash.clone();
        if let Some(release_tag) = response.get("release_tag").and_then(|value| value.as_str()) {
            state.target.app_release_tag = release_tag.to_string();
        }
        state.app.git = Some(source_path.clone());
        state.platform.github_repo = Some(source_path.clone());
        state.state.pushed = true;
        state.state.deployed = true;

        if self.activate {
            let token = std::env::var(ACTIVATION_TOKEN_ENV)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    anyhow!("--activate requires an activation token via {ACTIVATION_TOKEN_ENV}")
                })?;
            let url = self.backend_url().ok_or_else(|| {
                anyhow!("--activate requires a backend URL via --backend or {BACKEND_URL_ENV}")
            })?;
            let visibility = match app.public {
                Some(true) => Visibility::Public,
                _ => Visibility::Private,
            };
            let intent = ActivationIntent::new(
                &state.target.app_release_tag,
                platform.clone(),
                visibility,
            )?
            .source_path(Some(source_path.clone()))
            .source_subdir(Some(source_subdir.clone()))
            .server_tags(app.server_tags.clone())
            .label(Some(app.display_name.clone()));
            let plan = ActivationPlan::from_intent(url, token, intent)?;
            match plan.execute().await {
                Ok(_) => {
                    state.state.activated = true;
                }
                Err(e) => {
                    state.errors.push(format!("auto-activation failed: {e}"));
                }
            }
        }

        state.touch();
        let state_path = write_deployment_state(git_root, &state)?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            println!("{}", serde_json::to_string_pretty(&response)?);
            println!("  deployment_state    : {}", state_path.display());
            println!(
                "  state               : pushed={} deployed={} activated={}",
                state.state.pushed, state.state.deployed, state.state.activated
            );
            if !state.state.activated {
                self.print_next_steps();
            }
        }
        Ok(())
    }

    fn resolve_platform(&self, app: Option<&App>) -> Platform {
        if let Some(p) = self.platform.clone() {
            return p;
        }
        if let Some(name) = app
            .and_then(|a| a.platform.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Platform::new(name);
        }
        Platform::community()
    }

    fn backend_url(&self) -> Option<String> {
        self.backend
            .clone()
            .or_else(|| std::env::var(BACKEND_URL_ENV).ok())
            .filter(|s| !s.is_empty())
    }

    fn print_next_steps(&self) {
        println!();
        println!("Next steps:");
        println!("  1. Track the build and release:");
        println!("       aomi-git status --path {}", self.path.display());
        println!("     This polls CI and tells you when the release is ready to activate.");
        println!();
        println!("  2. Activate the release once CI is green (with your per-app code):");
        println!("       aomi-git activate --path {}", self.path.display());
        println!("     Set AOMI_APP_ACTIVATION_TOKEN (or pass --activation-token) to the");
        println!("     per-app code platform ops issued you.");
        println!();
        println!("     First time? Request activation before deploying:");
        println!("       aomi-git request --email <you@example.com> --git-account <github-user>");
    }
}

#[derive(Debug, Serialize)]
struct DeployAppRequest {
    source_path: String,
    commit_hash: String,
    source_subdir: String,
    is_public: bool,
    server_tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

// ---------------------------------------------------------------------------
// Activate
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct ActivateArgs {
    /// app_release_tag to activate (e.g. `apps-my-bot-abc1234`). When omitted,
    /// read from `.aomi/deployment.json` at `--path`.
    pub app_release_tag: Option<String>,

    /// Platform tag (`aomi.toml [app].platform`). Falls back to
    /// deployment.json's app.platform, then to `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// GitHub source repo URL. Falls back to deployment.json's app.git.
    #[arg(long = "source-path", value_name = "URL|owner/repo")]
    pub source_path: Option<String>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Platform activation token (default: `AOMI_APP_ACTIVATION_TOKEN`).
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// Activation visibility (`aomi.toml [app].public`). Falls back to
    /// deployment.json's app.public, then to `private`.
    #[arg(long, value_enum)]
    pub visibility: Option<Visibility>,

    /// Display label for the app registry row (`aomi.toml [app].display_name`).
    /// Falls back to deployment.json's app.display_name.
    #[arg(long, value_name = "STR")]
    pub display_name: Option<String>,

    /// Required backend server tag (repeatable).
    #[arg(long = "target-tag", value_name = "TAG")]
    pub server_tags: Vec<String>,

    /// Source repo path for the `.aomi/deployment.json` fallback. Defaults to
    /// the current directory.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Print the planned activation request without sending it.
    #[arg(long)]
    pub dry_run: bool,

    /// Print the backend response as JSON.
    #[arg(long)]
    pub json: bool,
}

impl ActivateArgs {
    pub async fn run(self) -> Result<()> {
        let (plan, mut state) = self.plan_with_state().await?;
        if self.dry_run {
            // No HTTP. Print what we'd send.
            let printable = serde_json::json!({
                "endpoint": plan.endpoint(),
                "request":  plan.request,
            });
            println!("{}", serde_json::to_string_pretty(&printable)?);
            if let Some(state) = &mut state {
                state.touch();
                write_deployment_state(&self.path, state)?;
            }
            return Ok(());
        }
        let response = plan.execute().await?;
        if let Some(state) = &mut state {
            state.state.activated = true;
            state.touch();
            write_deployment_state(&self.path, state)?;
        }
        if self.json {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            println!("activated {}", plan.request.name);
        }
        Ok(())
    }

    #[cfg(test)]
    pub async fn plan(&self) -> Result<ActivationPlan> {
        self.plan_with_state().await.map(|(plan, _)| plan)
    }

    async fn plan_with_state(&self) -> Result<(ActivationPlan, Option<DeploymentState>)> {
        // Load .aomi/deployment.json once for the fallback pyramid. Missing
        // is fine - we just have less to fall back on.
        let fallback = read_deployment_state(&self.path).ok().flatten();

        let app_release_tag = self
            .app_release_tag
            .clone()
            .or_else(|| fallback.as_ref().map(|s| s.target.app_release_tag.clone()))
            .ok_or_else(|| {
                anyhow!(
                    "app_release_tag is required - pass it positionally, or run from a directory \
                     with a prior `aomi-git deploy`'s .aomi/deployment.json"
                )
            })?;

        let platform = self
            .platform
            .clone()
            .or_else(|| {
                fallback
                    .as_ref()
                    .and_then(|s| s.app.platform.as_deref())
                    .map(Platform::new)
            })
            .unwrap_or_else(Platform::community);

        let backend_url = self.backend_url()?;
        let activation_token = self.activation_token()?;
        let source_path = self
            .source_path(fallback.as_ref())
            .await?;
        let server_tags = self.resolve_server_tags(fallback.as_ref())?;

        let display_name = self
            .display_name
            .clone()
            .or_else(|| fallback.as_ref().map(|s| s.app.display_name.clone()));

        // Visibility follows the same defaults pyramid as every other field:
        // CLI flag -> deployment.json's app.public -> hardcoded `private`.
        let visibility = self
            .visibility
            .or_else(|| {
                fallback.as_ref().and_then(|s| s.app.public).map(|public| {
                    if public {
                        Visibility::Public
                    } else {
                        Visibility::Private
                    }
                })
            })
            .unwrap_or(Visibility::Private);

        let intent = ActivationIntent::new(&app_release_tag, platform.clone(), visibility)?
            .source_path(source_path.clone())
            .source_subdir(Some(
                fallback
                    .as_ref()
                    .map(|s| s.source.source_path.display().to_string())
                    .unwrap_or_else(|| ".".to_string()),
            ))
            .server_tags(server_tags.clone())
            .label(display_name.clone());
        let plan = ActivationPlan::from_intent(backend_url, activation_token, intent)?;

        let state = fallback.map(|mut state| {
            state.target.app_release_tag = app_release_tag;
            state.app.platform = Some(platform.to_string());
            state.platform.name = Some(platform.to_string());
            if let Some(source_path) = source_path.clone() {
                state.app.git = Some(source_path.clone());
                state.platform.github_repo = Some(source_path);
            }
            state.app.public = Some(visibility == Visibility::Public);
            if let Some(display_name) = display_name {
                state.app.display_name = display_name.trim().to_string();
            }
            if !server_tags.is_empty() {
                state.target.server_tags = server_tags.clone();
                state.app.server_tags = server_tags;
            }
            state
        });

        Ok((plan, state))
    }

    fn backend_url(&self) -> Result<String> {
        self.backend
            .clone()
            .or_else(|| std::env::var(BACKEND_URL_ENV).ok())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("backend URL is required via --backend or {BACKEND_URL_ENV}"))
    }

    fn activation_token(&self) -> Result<String> {
        self.activation_token
            .clone()
            .or_else(|| std::env::var(ACTIVATION_TOKEN_ENV).ok())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "activation token is required via --activation-token or {ACTIVATION_TOKEN_ENV}"
                )
            })
    }

    async fn source_path(
        &self,
        fallback: Option<&DeploymentState>,
    ) -> Result<Option<String>> {
        if let Some(git) = self
            .source_path
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Ok(Some(git.to_string()));
        }
        if let Some(git) = fallback
            .and_then(|s| s.app.git.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Ok(Some(git.to_string()));
        }
        Ok(None)
    }

    /// Resolve `server_tags` with two rules:
    ///
    /// 1. If `--target-tag` is omitted, default to deployment.json's
    ///    `target.server_tags` (the build's declared intent). One less flag
    ///    in the happy path.
    /// 2. If `--target-tag` IS passed, enforce subset against the build's
    ///    `server_tags`: operator can narrow but cannot widen. The
    ///    contributor's intent at build time is the activation ceiling.
    ///
    /// Empty result is an error - activation needs at least one target tag.
    fn resolve_server_tags(&self, fallback: Option<&DeploymentState>) -> Result<Vec<String>> {
        let server_tags: Vec<String> = fallback
            .map(|s| s.target.server_tags.clone())
            .unwrap_or_default();

        if self.server_tags.is_empty() {
            if server_tags.is_empty() {
                bail!(
                    "no target tags supplied - pass `--target-tag <TAG>` (repeatable), \
                     or run from a source repo whose aomi.toml [app].server_tags declares them \
                     (and whose .aomi/deployment.json carries them)"
                );
            }
            return Ok(server_tags);
        }

        // CLI flag is non-empty; enforce subset if we have a server_tags ceiling.
        if !server_tags.is_empty() {
            let normalized_server: Vec<String> = server_tags
                .iter()
                .map(|t| t.trim().to_ascii_lowercase())
                .collect();
            let normalized_targets: Vec<String> = self
                .server_tags
                .iter()
                .map(|t| t.trim().to_ascii_lowercase())
                .collect();
            let widened: Vec<String> = normalized_targets
                .iter()
                .filter(|t| !normalized_server.contains(t))
                .cloned()
                .collect();
            if !widened.is_empty() {
                bail!(
                    "--target-tag [{}] would widen activation beyond aomi.toml \
                     [app].server_tags = [{}]\n\n\
                     This release was built with intent to ship to those backends only. \
                     The operator can narrow activation (subset OK) but cannot widen \
                     beyond the contributor's declared intent.\n\n\
                     To fix:\n  \
                     - if you DO want this app on the widened scope, re-deploy from the \
                     source repo with the desired server_tags in aomi.toml\n  \
                     - to activate just the intended subset, drop the widening tag(s) \
                     from --target-tag",
                    widened.join(", "),
                    normalized_server.join(", "),
                );
            }
        }
        Ok(self.server_tags.clone())
    }

}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct StatusArgs {
    /// app_release_tag to check (e.g. `apps-my-bot-abc1234`). When omitted, read
    /// from `.aomi/deployment.json` at `--path`.
    pub app_release_tag: Option<String>,

    /// Backend base URL. Defaults to `AOMI_BACKEND_URL`, then to the public
    /// backend implied by the build's `server_tags` (`staging` -> staging,
    /// `prod` -> prod). Pass `--backend ''` to skip.
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Source repo path for the `.aomi/deployment.json` lookup. Defaults to the
    /// current directory.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Print the status report as JSON.
    #[arg(long)]
    pub json: bool,
}

impl StatusArgs {
    pub async fn run(self) -> Result<()> {
        let mut state = read_deployment_state(&self.path)?.ok_or_else(|| {
            anyhow!(
                "no .aomi/deployment.json at {} - run `aomi-git deploy` first, or pass --path \
                 to the source repo",
                self.path.display()
            )
        })?;

        let app_release_tag = self
            .app_release_tag
            .clone()
            .unwrap_or_else(|| state.target.app_release_tag.clone());

        let backend_url = self.backend_url(&state);

        let req = crate::status::StatusRequest {
            app_name: state.app.name.clone(),
            app_release_tag: app_release_tag.clone(),
            backend_url,
            local: crate::status::LocalState {
                pushed: state.state.pushed,
                deployed: state.state.deployed,
                activated: state.state.activated,
                updated_at: state.updated_at,
            },
        };

        let report = crate::status::StatusReport::collect(req).await;
        state.target.app_release_tag = app_release_tag;
        match &report.backend {
            crate::status::BackendStatus::Found { is_active, .. } => {
                state.state.activated = is_active.unwrap_or(true);
            }
            crate::status::BackendStatus::NotRegistered { .. } => {
                state.state.activated = false;
            }
            _ => {}
        }
        state.touch();
        write_deployment_state(&self.path, &state)?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print!("{}", report.render());
        }
        Ok(())
    }

    /// Resolve the backend URL for the registry/health probe. Explicit
    /// `--backend` wins (including `--backend ''` to opt out); then
    /// `AOMI_BACKEND_URL`; then the public backend implied by the build's
    /// `server_tags`.
    fn backend_url(&self, state: &DeploymentState) -> Option<String> {
        if let Some(flag) = &self.backend {
            let trimmed = flag.trim();
            return (!trimmed.is_empty()).then(|| trimmed.to_string());
        }
        if let Ok(env) = std::env::var(BACKEND_URL_ENV) {
            let trimmed = env.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
        Self::default_backend_from_tags(&state.target.server_tags)
    }

    /// Map declared `server_tags` to the public backend most likely to run it:
    /// staging first, then prod. Custom tags return None so we never guess.
    fn default_backend_from_tags(tags: &[String]) -> Option<String> {
        if tags
            .iter()
            .any(|t| t.trim().eq_ignore_ascii_case("staging"))
        {
            Some("https://staging-api.aomi.dev".to_string())
        } else if tags.iter().any(|t| t.trim().eq_ignore_ascii_case("prod")) {
            Some("https://api.aomi.dev".to_string())
        } else {
            None
        }
    }
}
