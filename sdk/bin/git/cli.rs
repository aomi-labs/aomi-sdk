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
//!   --source-repo <URL|owner/repo> # aomi.toml [app].git
//!   --platform-dir <DIR>     # escape hatch: hand-managed local clone
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --dry-run                # plan + best-effort backend reads, no writes
//!   --allow-dirty
//!   --json
//!
//! activate [APP_RELEASE_TAG] # or .aomi/deployment.json target.app_release_tag
//!   --path <DIR>             # source repo (.aomi/deployment.json fallback)
//!   --platform <NAME>        # aomi.toml [app].platform
//!   --source-repo <URL|owner/repo> # aomi.toml [app].git
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --activation-token <T>   # AOMI_APP_ACTIVATION_TOKEN
//!   --access-token <$ENV|VAL># aomi.toml [app].access_token
//!   --target-tag <TAG>       # aomi.toml [app].server_tags (repeatable)
//!   --visibility <V>         # aomi.toml [app].public
//!   --display-name <STR>     # aomi.toml [app].display_name
//!   --source-commit <SHA>    # .aomi/deployment.json source.commit
//!   --source-tree <SHA>      # .aomi/deployment.json source.tree
//!   --source-digest <SHA>    # .aomi/deployment.json source.digest
//!   --dry-run
//!   --json
//!
//! request                    # ask ops for activation (invite + activation code)
//!   --email <EMAIL>          # where ops sends your activation code
//!   --git-account <USER>     # GitHub account to invite as a collaborator
//!   --app <NAME>             # aomi.toml [app].name (default)
//!   --platform <NAME>        # aomi.toml [app].platform (default community)
//!   --path <DIR>             # source repo (aomi.toml lookup)
//!   --dry-run                # print the Discord message; don't post
//!
//! status [APP_RELEASE_TAG]   # or .aomi/deployment.json target.app_release_tag
//!   --path <DIR>             # source repo (.aomi/deployment.json lookup)
//!   --source-repo <URL|owner/repo> # aomi.toml [app].git
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --access-token <$ENV|VAL># aomi.toml [app].access_token
//!   --json
//!
//! config
//!   --path <DIR>             # source repo (.aomi/deployment.json lookup)
//!   --app <NAME>             # aomi.toml [app].name
//!   --platform <NAME>        # aomi.toml [app].platform
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --activation-token <T>   # AOMI_APP_ACTIVATION_TOKEN
//!   --public <BOOL>          # aomi.toml [app].public
//!   --display-name <STR>     # aomi.toml [app].display_name
//!   --dry-run
//!   --json
//! ```
//!
//! `config` is a metadata-only edit: it reuses the activate endpoint with the
//! platform activation token but omits source_repo/app_release_tag, so the
//! backend updates the existing registry row in place without re-fetching the
//! release bundle.
//!
//! Defaults pyramid (both commands): CLI flag -> `.aomi/deployment.json` in
//! `--path` -> backend lookup -> hardcoded default. Each step is best-effort -
//! a missing deployment.json or unreachable backend never aborts the plan,
//! only the operation that genuinely needs the unresolved value.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};

use crate::activate::{
    ACTIVATION_TOKEN_ENV, ActivationPlan, BACKEND_URL_ENV, ConfigPlan, Visibility,
};
use crate::app::App;
use crate::deployment_state::{
    DeploymentState, read as read_deployment_state, write as write_deployment_state,
};
use crate::git::GitRepo;
use crate::plan::Deployment;
use crate::platform::{Platform, normalize_github_repo};
use crate::transit::TransitCache;

#[derive(Debug, Parser)]
#[command(name = "aomi-git")]
#[command(about = "Publish Aomi app source through platform Git policy.")]
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
            Command::Config(args) => args.run().await,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Ask platform ops for activation: a collaborator invite for your GitHub
    /// account plus a per-app activation code, delivered out-of-band. Run this
    /// once before your first deploy.
    Request(RequestArgs),
    /// Prepare and push an Aomi app source publication.
    Deploy(DeployArgs),
    /// Activate a published Aomi app release in the backend.
    Activate(ActivateArgs),
    /// Check publication status (CI build + release availability) for a deploy.
    Status(StatusArgs),
    /// Edit a live app's registry config (visibility, label, target tags)
    /// without re-deploying or re-fetching the release.
    Config(ConfigArgs),
}

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct RequestArgs {
    /// Email where platform ops will send your per-app activation code.
    #[arg(long, value_name = "EMAIL")]
    pub email: String,

    /// GitHub account to invite as a platform-repo collaborator.
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
        let discovered = GitRepo::discover(&self.path)
            .ok()
            .and_then(|repo| App::discover(&repo).ok());

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
            .map(|raw| normalize_github_repo(&raw))
            .transpose()?
            .ok_or_else(|| {
                anyhow!(
                    "platform repo is unknown - run from a source repo whose aomi.toml \
                     declares [app].git"
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
            "Posted activation request for `{}` to the Aomi apps Discord.",
            request.app
        );
        println!(
            "Ops will invite `{}` to `{}` and send your activation code to {}.",
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

    /// Platform publish repo (`aomi.toml [app].git`): URL, `owner/repo`, or SSH.
    /// When omitted, resolved from the backend's platform record.
    #[arg(long = "source-repo", value_name = "URL|owner/repo")]
    pub source_repo: Option<String>,

    /// Escape hatch: a hand-managed local clone to stage and push from.
    /// Skips the managed transit cache entirely. Useful for air-gapped CI or
    /// custom auth flows.
    #[arg(long, value_name = "DIR")]
    pub platform_dir: Option<PathBuf>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// App source directory. Defaults to the current directory.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Print the publish plan + best-effort backend reads without staging,
    /// pushing, or activating. Refreshes `.aomi/deployment.json` with the
    /// resolved plan and check results.
    #[arg(long)]
    pub dry_run: bool,

    /// Allow a dirty working tree in the printed plan and during staging.
    #[arg(long)]
    pub allow_dirty: bool,

    /// Print the publish plan / outcome as JSON.
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

        // Live deploy. Resolve the local clone path: either the user's
        // escape-hatch dir or the managed transit cache.
        let clone_path = self.resolve_clone_path(&platform, app.as_ref()).await?;

        let outcome = Deployment::git_transport(&self.path, platform.clone(), &clone_path, true)?;

        // Refresh .aomi/deployment.json with the post-push state. Start from
        // any prior dry-run state on disk so we don't drop earlier check rows.
        let git_root = &outcome.deployment.source.git_root;
        let mut state = match read_deployment_state(git_root) {
            Ok(Some(prior)) => prior,
            _ => outcome.deployment.to_state(),
        };
        state.state.pushed = outcome.pushed;
        state.recompute_deployed();

        // Auto-activate when an activation token is in the env and the push
        // landed. Per ADR 0009 we attempt even when state.deployed = false;
        // the backend is the authority on whether to reject.
        let activation_token = std::env::var(ACTIVATION_TOKEN_ENV).ok();
        let backend_url = self.backend_url();
        if outcome.pushed
            && let (Some(token), Some(url)) = (activation_token, backend_url)
        {
            let visibility = match outcome.deployment.app.public {
                Some(true) => Visibility::Public,
                _ => Visibility::Private,
            };
            let github_token = outcome.deployment.app.resolved_access_token()?;
            let plan = ActivationPlan::new(
                &outcome.deployment.publish.app_release_tag,
                platform.clone(),
                url,
                token,
                visibility,
                outcome.deployment.publish.source_repo.clone(),
                github_token,
                outcome.deployment.app.server_tags.clone(),
                Some(outcome.deployment.app.display_name.clone()),
                Some(outcome.deployment.source.commit.clone()),
                Some(outcome.deployment.source.tree.clone()),
                Some(outcome.deployment.source.digest.clone()),
            )?;
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
            println!("{}", serde_json::to_string_pretty(&outcome)?);
        } else {
            println!("{}", outcome.render());
            println!("  deployment_state    : {}", state_path.display());
            println!(
                "  state               : pushed={} deployed={} activated={}",
                state.state.pushed, state.state.deployed, state.state.activated
            );
            if outcome.pushed && !state.state.activated {
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

    /// Resolve the local clone path: explicit `--platform-dir` if set,
    /// otherwise initialize/refresh the managed transit cache.
    async fn resolve_clone_path(&self, platform: &Platform, app: Option<&App>) -> Result<PathBuf> {
        if let Some(dir) = &self.platform_dir {
            return Ok(dir.clone());
        }

        // Need a platform git URL to clone. Order: --source-repo -> aomi.toml
        // [app].git -> backend lookup.
        let git_url = self.resolve_source_repo(platform, app).await?;

        // Need the target branch. Use aomi.toml's [app].branch if present,
        // else default to "publish" (matches today's behavior and the
        // platforms.deployment_branch we observe in CI).
        let branch = app
            .and_then(|a| a.branch.clone())
            .unwrap_or_else(|| "publish".to_string());

        TransitCache::load()?.resolve(&git_url, &branch).with_context(|| {
            "could not initialize transit clone - pass --platform-dir <DIR> to use a hand-managed clone"
        })
    }

    async fn resolve_source_repo(&self, platform: &Platform, app: Option<&App>) -> Result<String> {
        if let Some(g) = self
            .source_repo
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Ok(g.to_string());
        }
        if let Some(g) = app
            .and_then(|a| a.git.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return Ok(g.to_string());
        }
        let backend_url = self.backend_url().ok_or_else(|| {
            anyhow!(
                "platform repo URL is not declared (no --source-repo, no aomi.toml [app].git, and no \
                 --backend / {BACKEND_URL_ENV} for backend lookup)"
            )
        })?;
        crate::preflight::lookup_platform_git(&backend_url, platform).await
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

    /// Platform publish repo. Falls back to deployment.json's app.git, then
    /// to a backend lookup keyed on `--platform`.
    #[arg(long = "source-repo", value_name = "URL|owner/repo")]
    pub source_repo: Option<String>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Platform activation token (default: `AOMI_APP_ACTIVATION_TOKEN`).
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// GitHub PAT (or `$ENV_VAR_NAME` reference, matching the toml form) for
    /// the backend's one-shot release-tarball fetch. Required for private
    /// platform repos; omit for public ones. Falls back to deployment.json's
    /// app.access_token if present.
    #[arg(long, value_name = "$ENV|VAL")]
    pub access_token: Option<String>,

    /// Activation visibility (`aomi.toml [app].public`). Falls back to
    /// deployment.json's app.public, then to `private`.
    #[arg(long, value_enum)]
    pub visibility: Option<Visibility>,

    /// Display label for the app registry row (`aomi.toml [app].display_name`).
    /// Falls back to deployment.json's app.display_name.
    #[arg(long, value_name = "STR")]
    pub display_name: Option<String>,

    /// Source provenance: full commit. Falls back to deployment.json.
    #[arg(long)]
    pub source_commit: Option<String>,

    /// Source provenance: tree hash. Falls back to deployment.json.
    #[arg(long)]
    pub source_tree: Option<String>,

    /// Source provenance: digest. Falls back to deployment.json.
    #[arg(long)]
    pub source_digest: Option<String>,

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
        let source_repo = self
            .source_repo(fallback.as_ref(), &platform, &backend_url)
            .await?;
        let github_token = self.github_token(fallback.as_ref())?;
        let server_tags = self.resolve_server_tags(fallback.as_ref())?;

        let display_name = self
            .display_name
            .clone()
            .or_else(|| fallback.as_ref().map(|s| s.app.display_name.clone()));

        let source_commit = self
            .source_commit
            .clone()
            .or_else(|| fallback.as_ref().map(|s| s.source.commit.clone()));
        let source_tree = self
            .source_tree
            .clone()
            .or_else(|| fallback.as_ref().map(|s| s.source.tree.clone()));
        let source_digest = self
            .source_digest
            .clone()
            .or_else(|| fallback.as_ref().map(|s| s.source.digest.clone()));

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

        let plan = ActivationPlan::new(
            &app_release_tag,
            platform.clone(),
            backend_url,
            activation_token,
            visibility,
            source_repo.clone(),
            github_token,
            server_tags.clone(),
            display_name.clone(),
            source_commit.clone(),
            source_tree.clone(),
            source_digest.clone(),
        )?;

        let state = fallback.map(|mut state| {
            state.target.app_release_tag = app_release_tag;
            state.app.platform = Some(platform.to_string());
            state.platform.name = Some(platform.to_string());
            state.app.git = Some(source_repo.clone());
            state.platform.github_repo = Some(source_repo);
            state.app.public = Some(visibility == Visibility::Public);
            if let Some(display_name) = display_name {
                state.app.display_name = display_name.trim().to_string();
            }
            if let Some(source_commit) = source_commit {
                state.source.commit = source_commit;
            }
            if let Some(source_tree) = source_tree {
                state.source.tree = source_tree;
            }
            if let Some(source_digest) = source_digest {
                state.source.digest = source_digest;
            }
            if !server_tags.is_empty() {
                state.target.server_tags = server_tags.clone();
                state.app.server_tags = server_tags;
            }
            if let Some(access_token) = self
                .access_token
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                state.app.access_token = Some(access_token.to_string());
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

    async fn source_repo(
        &self,
        fallback: Option<&DeploymentState>,
        platform: &Platform,
        backend_url: &str,
    ) -> Result<String> {
        if let Some(git) = self
            .source_repo
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return normalize_github_repo(git);
        }
        if let Some(git) = fallback
            .and_then(|s| s.app.git.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return normalize_github_repo(git);
        }
        crate::preflight::lookup_platform_git(backend_url, platform).await
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

    fn github_token(&self, fallback: Option<&DeploymentState>) -> Result<Option<String>> {
        let Some(value) = self
            .access_token
            .clone()
            .or_else(|| fallback.and_then(|s| s.app.access_token.clone()))
        else {
            return Ok(None);
        };
        let value = value.trim();
        if value.is_empty() {
            return Ok(None);
        }
        let Some(env_name) = value.strip_prefix('$') else {
            return Ok(Some(value.to_string()));
        };
        match std::env::var(env_name) {
            Ok(v) if !v.is_empty() => Ok(Some(v)),
            Ok(_) => bail!("env var `{env_name}` (from --access-token) is empty"),
            Err(_) => bail!("env var `{env_name}` (from --access-token) is not set"),
        }
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

    /// Platform publish repo. Falls back to deployment.json's app.git.
    #[arg(long = "source-repo", value_name = "URL|owner/repo")]
    pub source_repo: Option<String>,

    /// Backend base URL. When CI has finished, status also reports the app's
    /// backend registry row + runtime health. Defaults to `AOMI_BACKEND_URL`,
    /// then to the public backend implied by the build's `server_tags`
    /// (`staging` -> staging, `prod` -> prod). Pass `--backend ''` to skip.
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// GitHub PAT (or `$ENV_VAR_NAME` reference) for reading a private platform
    /// repo's Actions/release status. Omit for public repos. Falls back to
    /// deployment.json's app.access_token.
    #[arg(long, value_name = "$ENV|VAL")]
    pub access_token: Option<String>,

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

        // Resolve owner/repo: --source-repo -> deployment.json [app].git.
        let raw_repo = self
            .source_repo
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| state.app.git.clone())
            .ok_or_else(|| {
                anyhow!(
                    "platform repo is unknown - pass --source-repo <URL|owner/repo> or run from a source \
                     repo whose aomi.toml declares [app].git"
                )
            })?;
        let repo = normalize_github_repo(&raw_repo)?;

        let github_token = self.github_token(&state)?;

        let backend_url = self.backend_url(&state);

        let req = crate::status::StatusRequest {
            app_name: state.app.name.clone(),
            repo: repo.clone(),
            app_release_tag: app_release_tag.clone(),
            branch: state.target.branch.clone(),
            github_token,
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
        state.app.git = Some(repo.clone());
        state.platform.github_repo = Some(repo);
        if let Some(access_token) = self
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            state.app.access_token = Some(access_token.to_string());
        }
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

    /// Resolve GitHub read token: CLI flag, then deployment.json [app].access_token.
    fn github_token(&self, state: &DeploymentState) -> Result<Option<String>> {
        let Some(value) = self
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                state
                    .app
                    .access_token
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            })
        else {
            return Ok(None);
        };
        let Some(env_name) = value.strip_prefix('$') else {
            return Ok(Some(value));
        };
        match std::env::var(env_name) {
            Ok(v) if !v.is_empty() => Ok(Some(v)),
            Ok(_) => bail!("env var `{env_name}` (from --access-token) is empty"),
            Err(_) => bail!("env var `{env_name}` (from --access-token) is not set"),
        }
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

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct ConfigArgs {
    /// App slug to reconfigure. Falls back to `.aomi/deployment.json`'s
    /// `app.name` at `--path`.
    #[arg(long, value_name = "NAME")]
    pub app: Option<String>,

    /// Platform tag (`aomi.toml [app].platform`). Falls back to
    /// deployment.json's app.platform, then to `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Platform activation token (default: `AOMI_APP_ACTIVATION_TOKEN`).
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// Flip the app's visibility live (`aomi.toml [app].public`). Omit to leave
    /// the backend's current visibility untouched.
    #[arg(long, value_name = "BOOL")]
    pub public: Option<bool>,

    /// New display label for the registry row. When omitted, the app's existing
    /// `display_name` (from deployment.json) is re-sent so the backend's upsert
    /// doesn't overwrite the label with the bare slug.
    #[arg(long, value_name = "STR")]
    pub display_name: Option<String>,

    /// Source repo path for the `.aomi/deployment.json` fallback. Defaults to
    /// the current directory.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Print the planned config request without sending it.
    #[arg(long)]
    pub dry_run: bool,

    /// Print the backend response as JSON.
    #[arg(long)]
    pub json: bool,
}

impl ConfigArgs {
    pub async fn run(self) -> Result<()> {
        // At least one mutating flag must be present — config without an intent
        // is a no-op (and would needlessly re-send the label/metadata).
        if self.public.is_none() && self.display_name.is_none() {
            bail!("nothing to configure — pass --public <BOOL> and/or --display-name <STR>");
        }

        let (plan, mut state) = self.plan_with_state()?;
        if self.dry_run {
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
            state.touch();
            write_deployment_state(&self.path, state)?;
        }
        if self.json {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            println!("configured {}", plan.request.name);
        }
        Ok(())
    }

    fn plan_with_state(&self) -> Result<(ConfigPlan, Option<DeploymentState>)> {
        // Load .aomi/deployment.json once for the fallback pyramid. Missing is
        // fine — the user can still drive everything via flags.
        let fallback = read_deployment_state(&self.path).ok().flatten();

        let app_name = self
            .app
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| fallback.as_ref().map(|s| s.app.name.clone()))
            .ok_or_else(|| {
                anyhow!(
                    "app name is required — pass --app <NAME>, or run from a source repo with a \
                     prior `aomi-git deploy`'s .aomi/deployment.json"
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

        let backend_url = self
            .backend
            .clone()
            .or_else(|| std::env::var(BACKEND_URL_ENV).ok())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("backend URL is required via --backend or {BACKEND_URL_ENV}"))?;

        let activation_token = self
            .activation_token
            .clone()
            .or_else(|| std::env::var(ACTIVATION_TOKEN_ENV).ok())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                anyhow!(
                    "activation token is required via --activation-token or {ACTIVATION_TOKEN_ENV}"
                )
            })?;

        // Always resolve a display label to send: the explicit flag, else the
        // app's existing display_name. The backend rewrites the label on every
        // upsert, so re-sending the current value is what keeps it stable.
        let display_name = self
            .display_name
            .clone()
            .or_else(|| fallback.as_ref().map(|s| s.app.display_name.clone()));

        let plan = ConfigPlan::new(
            app_name.clone(),
            platform.clone(),
            backend_url,
            activation_token,
            self.public,
            display_name.clone(),
        )?;

        let state = fallback.map(|mut state| {
            state.app.name = app_name;
            state.app.platform = Some(platform.to_string());
            state.platform.name = Some(platform.to_string());
            if let Some(public) = self.public {
                state.app.public = Some(public);
            }
            if let Some(display_name) = display_name {
                state.app.display_name = display_name.trim().to_string();
            }
            state
        });

        Ok((plan, state))
    }
}
