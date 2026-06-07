//! `aomi-git` CLI surface.
//!
//! `aomi-git` is a thin backend relay: it reads local git facts and posts the
//! repo-scoped deploy/activate requests defined in CONTRACTS.md. It never
//! handles a GitHub token, never clones or pushes a platform repo, and never
//! generates release tags or manifests — the backend owns all of that.
//!
//! ```text
//! deploy                     # deploy tracked aomi.toml apps from a source ref
//!   --platform <NAME>        # aomi.toml [app].platform (default community)
//!   --branch <NAME>          # deploy this source branch (backend resolves it)
//!   --commit <SHA>           # deploy this source commit (default: HEAD)
//!   --aomi-toml <PATH>       # repeatable; default: all tracked aomi.toml
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --dry-run                # resolve + print the plan; deploy nothing
//!   --json
//!
//! activate [APP]...          # apps to activate (default: all from deployment.json)
//!   --target <PR_URL|BRANCH> # managed target (default: deployment.json PR)
//!   --platform <NAME>
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --activation-token <T>   # AOMI_APP_ACTIVATION_TOKEN
//!   --target-tag <TAG>       # repeatable
//!   --dry-run
//!   --json
//!
//! status                     # local deployment.json + backend per-app state
//!   --backend <URL>
//!   --json
//!
//! request                    # legacy ops onboarding request (Discord)
//! ```

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};

use crate::app::App;
use crate::backend::BackendClient;
use crate::git::GitRepo;
use crate::local;
use crate::platform::{Platform, normalize_github_repo};
use crate::status::StatusReport;
use crate::wire::{
    ActivateRequest, ActivateTarget, DeployRequest, DeploymentRecord, SourceRef, TargetValue,
};

pub(crate) const ACTIVATION_TOKEN_ENV: &str = "AOMI_APP_ACTIVATION_TOKEN";
pub(crate) const BACKEND_URL_ENV: &str = "AOMI_BACKEND_URL";

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
            Command::Deploy(args) => args.run().await,
            Command::Activate(args) => args.run().await,
            Command::Status(args) => args.run().await,
            Command::Request(args) => args.run().await,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Deploy tracked `aomi.toml` apps from a source ref through the backend.
    Deploy(DeployArgs),
    /// Activate built managed releases.
    Activate(ActivateArgs),
    /// Show local + backend deployment status.
    Status(StatusArgs),
    /// Ask platform ops for legacy onboarding details.
    Request(RequestArgs),
}

fn env_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// `--backend` flag, else `AOMI_BACKEND_URL`.
fn resolve_backend(flag: &Option<String>) -> Option<String> {
    flag.clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| env_value(BACKEND_URL_ENV))
}

// ---------------------------------------------------------------------------
// Deploy
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct DeployArgs {
    /// Platform tag (`aomi.toml [app].platform`). Defaults to aomi.toml, then
    /// `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// Deploy this source branch; the backend resolves it to a commit.
    #[arg(long, value_name = "NAME", conflicts_with = "commit")]
    pub branch: Option<String>,

    /// Deploy this exact source commit. Defaults to local HEAD.
    #[arg(long, value_name = "SHA")]
    pub commit: Option<String>,

    /// `aomi.toml` to deploy, repo-relative. Repeatable. Defaults to every
    /// tracked `aomi.toml` in the repo.
    #[arg(long = "aomi-toml", value_name = "PATH")]
    pub aomi_toml: Vec<String>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// App source directory. Defaults to the current directory.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Resolve and print the deploy plan without deploying.
    #[arg(long)]
    pub dry_run: bool,

    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

impl DeployArgs {
    async fn run(self) -> Result<()> {
        let repo = GitRepo::discover(&self.path)?;
        let platform = self.platform(&repo);
        let source_ref = self.source_ref(&repo)?;
        let aomi_toml_paths = self.aomi_toml_paths(&repo)?;

        // The backend fetches the commit from GitHub, not the local tree, so a
        // commit ref must already be pushed. Branch refs are resolved remotely.
        if let SourceRef::Commit { value } = &source_ref {
            local::ensure_pushed(repo.root(), value)?;
        }

        let request = DeployRequest {
            source_ref,
            aomi_toml_paths,
            dry_run: self.dry_run,
        };

        if self.dry_run {
            return self.run_dry_run(&platform, &request).await;
        }

        let backend_url = self.backend_url()?;
        let token = activation_token()?;
        let response = BackendClient::new(backend_url, token)?
            .deploy(&platform, &request)
            .await?;

        let state = DeploymentRecord::from_deploy(response);
        let path = state.write(repo.root())?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&state)?);
        } else {
            println!(
                "Deployed {} app(s) to platform `{platform}`",
                state.managed.apps.len()
            );
            println!("  pr            : {}", state.managed.pr_url);
            println!("  deploy_branch : {}", state.managed.deploy_branch);
            for app in &state.managed.apps {
                println!("  - {} -> {}", app.name, app.release_tag);
            }
            println!("  deployment    : {}", path.display());
            println!();
            println!("Next: track CI, then activate once it is green:");
            println!("  aomi-git status --path {}", self.path.display());
            println!("  aomi-git activate --path {}", self.path.display());
        }
        Ok(())
    }

    /// Dry-run: POST with `dry_run: true` when a backend + token are available
    /// (the backend resolves the branch and validates scope); otherwise print
    /// the request we would send, fully offline.
    async fn run_dry_run(&self, platform: &Platform, request: &DeployRequest) -> Result<()> {
        match (self.backend_url().ok(), activation_token().ok()) {
            (Some(url), Some(token)) => {
                let response = BackendClient::new(url, token)?
                    .deploy(platform, request)
                    .await?;
                println!("{}", serde_json::to_string_pretty(&response)?);
            }
            _ => {
                println!("{}", serde_json::to_string_pretty(request)?);
                println!("\n(dry-run: no backend/token; printed the request only)");
            }
        }
        Ok(())
    }

    pub(crate) fn platform(&self, repo: &GitRepo) -> Platform {
        if let Some(p) = &self.platform {
            return p.clone();
        }
        App::discover(repo)
            .ok()
            .and_then(|a| a.platform)
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .map(Platform::new)
            .unwrap_or_else(Platform::community)
    }

    pub(crate) fn source_ref(&self, repo: &GitRepo) -> Result<SourceRef> {
        if let Some(branch) = self
            .branch
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
        {
            return Ok(SourceRef::branch(branch));
        }
        if let Some(commit) = self
            .commit
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            return Ok(SourceRef::commit(commit));
        }
        Ok(SourceRef::commit(repo.head_commit()?))
    }

    pub(crate) fn aomi_toml_paths(&self, repo: &GitRepo) -> Result<Vec<String>> {
        if !self.aomi_toml.is_empty() {
            let mut paths: Vec<String> = self
                .aomi_toml
                .iter()
                .map(|p| normalize_rel_path(p))
                .collect::<Result<_>>()?;
            paths.sort();
            paths.dedup();
            return Ok(paths);
        }
        let found = local::discover_aomi_tomls(repo.root())?;
        if found.is_empty() {
            bail!(
                "no tracked aomi.toml found under {} — add and commit one, or pass --aomi-toml",
                repo.root().display()
            );
        }
        Ok(found)
    }

    fn backend_url(&self) -> Result<String> {
        resolve_backend(&self.backend).ok_or_else(|| {
            anyhow!("deploy needs a backend URL — set --backend or {BACKEND_URL_ENV}")
        })
    }
}

fn activation_token() -> Result<String> {
    env_value(ACTIVATION_TOKEN_ENV)
        .ok_or_else(|| anyhow!("deploy requires an activation token via {ACTIVATION_TOKEN_ENV}"))
}

/// Normalize a user-supplied path to a clean repo-relative POSIX path.
fn normalize_rel_path(value: &str) -> Result<String> {
    let path = value.trim().replace('\\', "/");
    let path = path.strip_prefix("./").unwrap_or(&path);
    let path = path.trim_matches('/');
    if path.is_empty() {
        bail!("empty --aomi-toml path");
    }
    if path.split('/').any(|seg| seg == "..") {
        bail!("--aomi-toml path may not contain '..': `{value}`");
    }
    Ok(path.to_string())
}

// ---------------------------------------------------------------------------
// Activate
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct ActivateArgs {
    /// Apps to activate. Defaults to every app from `.aomi/deployment.json`.
    #[arg(value_name = "APP")]
    pub apps: Vec<String>,

    /// Managed target: a PR URL (`…/pull/N`) or a managed branch. Defaults to
    /// the PR opened by the last deploy.
    #[arg(long, value_name = "PR_URL|BRANCH")]
    pub target: Option<String>,

    /// Platform tag. Defaults to `.aomi/deployment.json`, then `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

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
}

impl ActivateArgs {
    async fn run(self) -> Result<()> {
        let mut state = DeploymentRecord::read(&self.path)?;
        let request = self.build_request(state.as_ref())?;

        if self.dry_run {
            println!("{}", serde_json::to_string_pretty(&request)?);
            return Ok(());
        }

        let backend_url = resolve_backend(&self.backend).ok_or_else(|| {
            anyhow!("activate needs a backend URL — set --backend or {BACKEND_URL_ENV}")
        })?;
        let token = self
            .activation_token
            .clone()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .or_else(|| env_value(ACTIVATION_TOKEN_ENV))
            .ok_or_else(|| {
                anyhow!(
                    "activate requires a token via --activation-token or {ACTIVATION_TOKEN_ENV}"
                )
            })?;

        let response = BackendClient::new(backend_url, token)?
            .activate(&request)
            .await?;

        if let Some(state) = &mut state {
            state.apply_activation(&response.activation);
            state.write(&self.path)?;
        }

        if self.json {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
            println!("activation: {}", response.activation.status);
            for app in &response.activation.apps {
                match &app.error {
                    Some(err) => println!("  - {} : FAILED ({err})", app.name),
                    None => println!(
                        "  - {} : active={} loaded={}",
                        app.name, app.is_active, app.loaded
                    ),
                }
            }
        }
        Ok(())
    }

    pub(crate) fn build_request(&self, state: Option<&DeploymentRecord>) -> Result<ActivateRequest> {
        let platform = self
            .platform
            .clone()
            .or_else(|| state.map(|s| Platform::new(&s.managed.platform)))
            .unwrap_or_else(Platform::community);

        let target = match self.target.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
            Some(value) => infer_target(value),
            None => state
                .map(DeploymentRecord::default_target)
                .ok_or_else(|| {
                    anyhow!(
                        "no --target and no .aomi/deployment.json at {} — deploy first or pass --target",
                        self.path.display()
                    )
                })?,
        };

        let apps = if !self.apps.is_empty() {
            self.apps.clone()
        } else {
            state.map(DeploymentRecord::app_names).unwrap_or_default()
        };
        if apps.is_empty() {
            bail!(
                "no apps to activate — name them positionally or deploy first so \
                 .aomi/deployment.json records them"
            );
        }

        Ok(ActivateRequest {
            platform: platform.to_string(),
            target,
            apps,
            target_tags: self.target_tags.clone(),
        })
    }
}

/// Infer the managed-target kind from a CLI value: a GitHub PR URL becomes
/// `managed_pr`, anything else is treated as a `managed_branch`.
fn infer_target(value: &str) -> ActivateTarget {
    let kind = if value.contains("github.com") && value.contains("/pull/") {
        "managed_pr"
    } else {
        "managed_branch"
    };
    ActivateTarget {
        kind: kind.to_string(),
        value: TargetValue::One(value.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

#[derive(Debug, Args, Clone)]
pub struct StatusArgs {
    /// Backend base URL (default: `AOMI_BACKEND_URL`). Pass `--backend ''` to
    /// skip the backend probe.
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Source repo path for the `.aomi/deployment.json` lookup.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Print the status report as JSON.
    #[arg(long)]
    pub json: bool,
}

impl StatusArgs {
    async fn run(self) -> Result<()> {
        let state = DeploymentRecord::read(&self.path)?.ok_or_else(|| {
            anyhow!(
                "no .aomi/deployment.json at {} — run `aomi-git deploy` first",
                self.path.display()
            )
        })?;

        // `--backend ''` explicitly opts out; otherwise flag/env.
        let backend_url = match &self.backend {
            Some(flag) if flag.trim().is_empty() => None,
            other => resolve_backend(other),
        };

        let report = StatusReport::collect(&state, backend_url).await;
        if self.json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            print!("{}", report.render());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Request (legacy ops onboarding)
// ---------------------------------------------------------------------------

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
    async fn run(self) -> Result<()> {
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

        let repo = GitRepo::discover(&self.path).ok();
        let app_cfg = repo.as_ref().and_then(|r| App::discover(r).ok());

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
            .or_else(|| repo.as_ref().and_then(|r| r.remote_origin().ok()))
            .map(|raw| normalize_github_repo(&raw))
            .transpose()?
            .ok_or_else(|| {
                anyhow!("source repo is unknown - run from a source repo with a GitHub origin")
            })?;

        let request = crate::discord::ActivationRequest {
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
            crate::discord::DISCORD_INVITE
        );
        Ok(())
    }
}
