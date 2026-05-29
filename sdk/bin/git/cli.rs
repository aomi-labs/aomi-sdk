//! `aomi-git` CLI surface.
//!
//! Flag names mirror `aomi.toml [app]` field names so contributors don't have
//! to translate between two vocabularies. Where a CLI concept has no toml
//! equivalent (an escape-hatch local directory, the backend URL, dry-run), the
//! flag is named for what it does on the CLI.
//!
//! ```text
//! deploy [PATH]
//!   --platform <NAME>        # aomi.toml [app].platform
//!   --git <URL|owner/repo>   # aomi.toml [app].git
//!   --platform-dir <DIR>     # escape hatch: hand-managed local clone
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --dry-run                # plan + best-effort backend reads, no writes
//!   --allow-dirty
//!   --json
//!
//! activate [RELEASE_TAG]
//!   --platform <NAME>
//!   --git <URL|owner/repo>
//!   --backend <URL>
//!   --activation-token <T>   # AOMI_APP_ACTIVATION_TOKEN
//!   --access-token <$ENV|VAL># aomi.toml [app].access_token form
//!   --target-tag <TAG>       # repeatable
//!   --visibility <V>
//!   --display-name <STR>     # aomi.toml [app].display_name
//!   --source-commit <SHA>
//!   --source-tree <SHA>
//!   --source-digest <SHA>
//!   --path <DIR>             # source repo (for .aomi/deployment.json fallback)
//!   --dry-run
//!   --json
//!
//! status [RELEASE_TAG]
//!   --git <URL|owner/repo>   # falls back to .aomi/deployment.json [app].git
//!   --access-token <$ENV|VAL># private-repo GitHub reads
//!   --path <DIR>             # source repo (for .aomi/deployment.json lookup)
//!   --json
//! ```
//!
//! Defaults pyramid (both commands): CLI flag → `.aomi/deployment.json` in
//! `--path` → backend lookup → hardcoded default. Each step is best-effort —
//! a missing deployment.json or unreachable backend never aborts the plan,
//! only the operation that genuinely needs the unresolved value.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};

use crate::activate::{ACTIVATION_TOKEN_ENV, ActivationPlan, BACKEND_URL_ENV, Visibility};
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
            Command::Deploy(args) => args.run().await,
            Command::Activate(args) => args.run().await,
            Command::Status(args) => args.run().await,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Prepare and push an Aomi app source publication.
    Deploy(DeployArgs),
    /// Activate a published Aomi app release in the backend.
    Activate(ActivateArgs),
    /// Check publication status (CI build + release availability) for a deploy.
    Status(StatusArgs),
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

    /// Platform repo location: accepts URL, `owner/repo`, or SSH form
    /// (`aomi.toml [app].git`). When omitted, resolved from the backend's
    /// platform record.
    #[arg(long, value_name = "URL|owner/repo")]
    pub git: Option<String>,

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

            // Preflight is no longer a separate flag — dry-run always tries
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
                &outcome.deployment.publish.release_tag,
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
                print_next_steps(&outcome, &state);
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

        // Need a platform git URL to clone. Order: --git flag → aomi.toml
        // [app].git → backend lookup.
        let git_url = self.resolve_platform_git(platform, app).await?;

        // Need the target branch. Use aomi.toml's [app].branch if present,
        // else default to "publish" (matches today's behavior and the
        // platforms.deployment_branch we observe in CI).
        let branch = app
            .and_then(|a| a.branch.clone())
            .unwrap_or_else(|| "publish".to_string());

        TransitCache::load()?.resolve(&git_url, &branch).with_context(|| {
            "could not initialize transit clone — pass --platform-dir <DIR> to use a hand-managed clone"
        })
    }

    async fn resolve_platform_git(&self, platform: &Platform, app: Option<&App>) -> Result<String> {
        if let Some(g) = self.git.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
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
                "platform repo URL is not declared (no --git flag, no aomi.toml [app].git, and no \
                 --backend / {BACKEND_URL_ENV} for backend lookup)"
            )
        })?;
        crate::preflight::lookup_platform_git(&backend_url, platform).await
    }
}

// ---------------------------------------------------------------------------
// Activate
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct ActivateArgs {
    /// Release tag to activate (e.g. `apps-my-bot-abc1234`). When omitted,
    /// read from `.aomi/deployment.json` at `--path`.
    pub release_tag: Option<String>,

    /// Platform tag (`aomi.toml [app].platform`). Falls back to
    /// deployment.json's app.platform, then to `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// Platform repo location. Falls back to deployment.json's app.git, then
    /// to a backend lookup keyed on `--platform`.
    #[arg(long, value_name = "URL|owner/repo")]
    pub git: Option<String>,

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
    pub target_tags: Vec<String>,

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
        let plan = self.plan().await?;
        if self.dry_run {
            // No HTTP. Print what we'd send.
            let printable = serde_json::json!({
                "endpoint": plan.endpoint(),
                "request":  plan.request,
            });
            println!("{}", serde_json::to_string_pretty(&printable)?);
            return Ok(());
        }
        let response = plan.execute().await?;
        if self.json {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            println!("activated {}", plan.request.name);
        }
        Ok(())
    }

    pub async fn plan(&self) -> Result<ActivationPlan> {
        // Load .aomi/deployment.json once for the fallback pyramid. Missing
        // is fine — we just have less to fall back on.
        let fallback = read_deployment_state(&self.path).ok().flatten();

        let release_tag = self
            .release_tag
            .clone()
            .or_else(|| fallback.as_ref().map(|s| s.target.release_tag.clone()))
            .ok_or_else(|| {
                anyhow!(
                    "release tag is required — pass it positionally, or run from a directory \
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
        let target_tags = self.resolve_target_tags(fallback.as_ref())?;

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
        // CLI flag → deployment.json's app.public → hardcoded `private`.
        let visibility = self
            .visibility
            .or_else(|| {
                fallback
                    .as_ref()
                    .and_then(|s| s.app.public)
                    .map(|public| {
                        if public {
                            Visibility::Public
                        } else {
                            Visibility::Private
                        }
                    })
            })
            .unwrap_or(Visibility::Private);

        ActivationPlan::new(
            &release_tag,
            platform,
            backend_url,
            activation_token,
            visibility,
            source_repo,
            github_token,
            target_tags,
            display_name,
            source_commit,
            source_tree,
            source_digest,
        )
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
        if let Some(git) = self.git.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
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

    /// Resolve `target_tags` with two rules:
    ///
    /// 1. If `--target-tag` is omitted, default to deployment.json's
    ///    `target.server_tags` (the build's declared intent). One less flag
    ///    in the happy path.
    /// 2. If `--target-tag` IS passed, enforce subset against the build's
    ///    `server_tags`: operator can narrow but cannot widen. The
    ///    contributor's intent at build time is the activation ceiling.
    ///
    /// Empty result is an error — activation needs at least one target tag.
    fn resolve_target_tags(&self, fallback: Option<&DeploymentState>) -> Result<Vec<String>> {
        let server_tags: Vec<String> = fallback
            .map(|s| s.target.server_tags.clone())
            .unwrap_or_default();

        if self.target_tags.is_empty() {
            if server_tags.is_empty() {
                bail!(
                    "no target tags supplied — pass `--target-tag <TAG>` (repeatable), \
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
                .target_tags
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
        Ok(self.target_tags.clone())
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
    /// Release tag to check (e.g. `apps-my-bot-abc1234`). When omitted, read
    /// from `.aomi/deployment.json` at `--path`.
    pub release_tag: Option<String>,

    /// Platform repo location. Falls back to deployment.json's app.git.
    #[arg(long, value_name = "URL|owner/repo")]
    pub git: Option<String>,

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
        let state = read_deployment_state(&self.path)?.ok_or_else(|| {
            anyhow!(
                "no .aomi/deployment.json at {} — run `aomi-git deploy` first, or pass --path \
                 to the source repo",
                self.path.display()
            )
        })?;

        let release_tag = self
            .release_tag
            .clone()
            .unwrap_or_else(|| state.target.release_tag.clone());

        // Resolve owner/repo: --git flag → deployment.json [app].git.
        let raw_repo = self
            .git
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| state.app.git.clone())
            .ok_or_else(|| {
                anyhow!(
                    "platform repo is unknown — pass --git <URL|owner/repo> or run from a source \
                     repo whose aomi.toml declares [app].git"
                )
            })?;
        let repo = normalize_github_repo(&raw_repo)?;

        let github_token = resolve_access_token(
            self.access_token.as_deref(),
            state.app.access_token.as_deref(),
        )?;

        let req = crate::status::StatusRequest {
            repo,
            release_tag,
            branch: state.target.branch.clone(),
            github_token,
            local: crate::status::LocalState {
                pushed: state.state.pushed,
                deployed: state.state.deployed,
                activated: state.state.activated,
                updated_at: state.updated_at,
            },
        };

        let report = crate::status::StatusReport::collect(req).await;
        if self.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print!("{}", report.render());
        }
        Ok(())
    }
}

/// Resolve an `--access-token` value (or its deployment.json fallback) to a
/// concrete token. `$ENV_VAR_NAME` references are dereferenced from the
/// environment; a bare value is taken literally. `None` (public repo) is fine.
fn resolve_access_token(flag: Option<&str>, fallback: Option<&str>) -> Result<Option<String>> {
    let Some(value) = flag
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| fallback.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string))
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

/// URL contributors click to request activation from platform ops. The
/// release tag and repo ride along as query params so the landing page can
/// post a pre-filled, admin-tagging message into the `#aomi-apps` Discord
/// channel on the contributor's behalf (server-side webhook) — the contributor
/// never holds the activation token, so requesting is the real "next action".
const REQUEST_ACTIVATION_URL: &str = "https://aomi.dev/request-activation";

/// Print a Next-steps block after a successful push, before activation has
/// landed.
///
/// Two steps, mirroring the trust model: the contributor (1) watches the
/// publication land via `aomi-git status`, then (2) requests activation from
/// platform ops via a Discord deep link — because contributors don't hold the
/// activation token, *asking* is the next action, not running `activate`.
fn print_next_steps(outcome: &crate::plan::DeployOutcome, _state: &DeploymentState) {
    let release_tag = &outcome.deployment.publish.release_tag;
    let repo = &outcome.deployment.publish.source_repo;

    println!();
    println!("Next steps:");
    println!("  1. Track the build & release:");
    println!("       aomi-git status");
    println!("     (polls CI and tells you when the release is ready to activate)");
    println!();
    println!("  2. Request activation from platform ops:");
    println!(
        "       {REQUEST_ACTIVATION_URL}?release={release_tag}&repo={repo}"
    );
    println!(
        "     Opens #aomi-apps and pings @platform-ops with your release tag."
    );
    println!("     (Contributors don't hold the activation token — ops activate for you.)");
}
