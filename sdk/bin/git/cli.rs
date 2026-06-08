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
//!   --app-source-id <ID>     # connected GitHub App install (AOMI_APP_SOURCE_ID)
//!   --branch <NAME>          # deploy this source branch (backend resolves it)
//!   --commit <SHA>           # deploy this source commit (default: HEAD)
//!   --aomi-toml <PATH>       # repeatable; default: all tracked aomi.toml
//!   --backend <URL>          # AOMI_BACKEND_URL
//!   --dry-run                # resolve + print the plan; deploy nothing
//!   --json
//!
//! activate [APP]...          # apps to activate (default: all from deployment.json)
//!   --platform <NAME>        # default: deployment.json platform
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

use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Parser, Subcommand};

use crate::app::AomiAppFiles;
use crate::backend::BackendClient;
use crate::platform::{Platform, normalize_github_repo};
use crate::status::StatusReport;
use crate::types::{ActivateRequest, DeployRequest, LocalRecord, SourceRef};

pub(crate) const ACTIVATION_TOKEN_ENV: &str = "AOMI_APP_ACTIVATION_TOKEN";
pub(crate) const BACKEND_URL_ENV: &str = "AOMI_BACKEND_URL";
pub(crate) const APP_SOURCE_ID_ENV: &str = "AOMI_APP_SOURCE_ID";

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

    /// The connected GitHub App install (`app_source`) to deploy from. The
    /// backend resolves the source repo from it. Falls back to
    /// `AOMI_APP_SOURCE_ID`.
    #[arg(long = "app-source-id", value_name = "ID")]
    pub app_source_id: Option<i64>,

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
        let (git_root, start_dir) = git_context(&self.path)?;
        let platform = self.platform(&git_root, &start_dir);
        let source_ref = self.source_ref(&git_root)?;
        let aomi_toml_paths = self.aomi_toml_paths(&git_root)?;
        let app_source_id = self.resolve_app_source_id()?;

        let request = DeployRequest {
            app_source_id,
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

        let state = LocalRecord::from_deploy(response);
        let path = state.write(&git_root)?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&state)?);
        } else {
            println!(
                "Deployed {} app(s) to platform `{platform}`",
                state.deployment.platform.apps.len()
            );
            println!(
                "  pr            : {}",
                state
                    .deployment
                    .platform
                    .pr_url
                    .as_deref()
                    .unwrap_or("(pending CI)")
            );
            println!(
                "  deploy_branch : {}",
                state.deployment.platform.deploy_branch
            );
            for app in &state.deployment.platform.apps {
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

    pub(crate) fn platform(&self, git_root: &Path, start_dir: &Path) -> Platform {
        if let Some(p) = &self.platform {
            return p.clone();
        }
        AomiAppFiles::discover(start_dir, git_root)
            .ok()
            .and_then(|a| a.platform)
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .map(Platform::new)
            .unwrap_or_else(Platform::community)
    }

    pub(crate) fn source_ref(&self, git_root: &Path) -> Result<SourceRef> {
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
        Ok(SourceRef::commit(head_commit(git_root)?))
    }

    pub(crate) fn aomi_toml_paths(&self, git_root: &Path) -> Result<Vec<String>> {
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
        let found = tracked_aomi_tomls(git_root)?;
        if found.is_empty() {
            bail!(
                "no tracked aomi.toml found under {} — add and commit one, or pass --aomi-toml",
                git_root.display()
            );
        }
        Ok(found)
    }

    fn backend_url(&self) -> Result<String> {
        resolve_backend(&self.backend).ok_or_else(|| {
            anyhow!("deploy needs a backend URL — set --backend or {BACKEND_URL_ENV}")
        })
    }

    fn resolve_app_source_id(&self) -> Result<i64> {
        self.app_source_id
            .or_else(|| env_value(APP_SOURCE_ID_ENV).and_then(|v| v.parse::<i64>().ok()))
            .filter(|id| *id > 0)
            .ok_or_else(|| {
                anyhow!(
                    "deploy needs --app-source-id (or {APP_SOURCE_ID_ENV}): the connected GitHub \
                     App install to deploy from. Install the Aomi GitHub App on your source repo, \
                     then use the app_source id ops/the portal issued you."
                )
            })
    }
}

fn activation_token() -> Result<String> {
    env_value(ACTIVATION_TOKEN_ENV)
        .ok_or_else(|| anyhow!("deploy requires an activation token via {ACTIVATION_TOKEN_ENV}"))
}

fn git_context(start: impl AsRef<Path>) -> Result<(PathBuf, PathBuf)> {
    let start_dir = normalize_start_dir(start.as_ref())?;
    let root = git_output_at(&start_dir, ["rev-parse", "--show-toplevel"])
        .with_context(|| format!("failed to find git root from {}", start_dir.display()))?;
    let root = PathBuf::from(root.trim());
    let root = root.canonicalize().unwrap_or(root);
    Ok((root, start_dir))
}

fn head_commit(git_root: &Path) -> Result<String> {
    Ok(git_output_at(git_root, ["rev-parse", "HEAD"])?
        .trim()
        .to_string())
}

fn remote_origin(git_root: &Path) -> Result<String> {
    Ok(git_output_at(git_root, ["remote", "get-url", "origin"])?
        .trim()
        .to_string())
}

fn tracked_aomi_tomls(git_root: &Path) -> Result<Vec<String>> {
    let raw = git_output_at(
        git_root,
        ["ls-files", "-z", "--", "*aomi.toml", "aomi.toml"],
    )
    .with_context(|| format!("failed to list tracked files in {}", git_root.display()))?;
    let mut paths: Vec<String> = raw
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .filter(|entry| entry.rsplit('/').next() == Some("aomi.toml"))
        .map(str::to_string)
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn normalize_start_dir(path: &Path) -> Result<PathBuf> {
    let path = if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    };
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    Ok(if path.is_file() {
        path.parent()
            .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?
            .to_path_buf()
    } else {
        path
    })
}

fn git_output_at<const N: usize>(dir: &Path, args: [&str; N]) -> Result<String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", dir.display()))?;
    if !output.status.success() {
        bail!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("git output was not valid UTF-8")
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
        let mut state = LocalRecord::read(&self.path)?.ok_or_else(|| {
            anyhow!(
                "no .aomi/deployment.json at {} — run `aomi-git deploy` first",
                self.path.display()
            )
        })?;
        let platform = self
            .platform
            .clone()
            .unwrap_or_else(|| Platform::new(&state.deployment.platform.platform));

        // Resolve (app, release_tag) for each requested app (or all deployed).
        let app_names = if self.apps.is_empty() {
            state.app_names()
        } else {
            self.apps.clone()
        };
        if app_names.is_empty() {
            bail!("no apps to activate — deploy first or name them positionally");
        }
        let mut targets: Vec<(String, String)> = Vec::with_capacity(app_names.len());
        for app in &app_names {
            let tag = state.release_tag_for(app).ok_or_else(|| {
                anyhow!("app `{app}` is not in .aomi/deployment.json; deploy it first")
            })?;
            targets.push((app.clone(), tag.to_string()));
        }

        if self.dry_run {
            for (app, tag) in &targets {
                let request = ActivateRequest {
                    app_release_tag: tag.clone(),
                    target_tags: self.target_tags.clone(),
                };
                let printable = serde_json::json!({
                    "endpoint": format!("/api/platforms/{}/apps/{}/activate", platform.as_str(), app),
                    "request": request,
                });
                println!("{}", serde_json::to_string_pretty(&printable)?);
            }
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
        let client = BackendClient::new(backend_url, token)?;

        // Activate each app independently (the endpoint is single-app); collect
        // results, persist after each so a mid-run failure still records progress.
        let mut failures = 0usize;
        for (app, tag) in &targets {
            let request = ActivateRequest {
                app_release_tag: tag.clone(),
                target_tags: self.target_tags.clone(),
            };
            match client.activate(&platform, app, &request).await {
                Ok(response) => {
                    state.apply_activation(&response);
                    if !self.json {
                        println!(
                            "  - {} : active={} ({})",
                            response.name, response.is_active, response.status
                        );
                    }
                }
                Err(e) => {
                    failures += 1;
                    if !self.json {
                        println!("  - {app} : FAILED ({e})");
                    }
                }
            }
        }
        state.write(&self.path)?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&state)?);
        }
        if failures > 0 {
            bail!("{failures} app(s) failed to activate");
        }
        Ok(())
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
        let state = LocalRecord::read(&self.path)?.ok_or_else(|| {
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
