//! Hosted deployment CLI surface for `aomi-build`.
//!
//! These commands are thin backend relays: they read local git facts and post the
//! repo-scoped deploy/activate requests defined in `hosted/types.rs`. They
//! never handle a GitHub token, never clone or push a platform repo, and never
//! generate release tags or manifests — the backend owns all of that.
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
//! activate [APP]...          # activate release tags (default: deployment.json tags)
//!   --platform <NAME>        # default: deployment.json platform
//!   --release-tag <TAG>      # repeatable; overrides deployment.json tags
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
use clap::{Args, Subcommand};

use super::app::AomiAppFiles;
use super::backend::BackendClient;
use super::config::AomiConfig;
use super::discord;
use super::platform::{Platform, normalize_github_repo};
use super::status::StatusResult;
use super::types::{
    ActivateInput, CreateTemplateInput, DeployInput, LocalDeployment, MintTokenInput, ReleaseTags,
    SourceRef, SyncSourceInput,
};

pub(crate) const ACTIVATION_TOKEN_ENV: &str = "AOMI_APP_ACTIVATION_TOKEN";
pub(crate) const BACKEND_URL_ENV: &str = "AOMI_BACKEND_URL";
pub(crate) const APP_SOURCE_ID_ENV: &str = "AOMI_APP_SOURCE_ID";
pub(crate) const ADMIN_KEY_ENV: &str = "AOMI_ADMIN_KEY";
pub(crate) const ADMIN_KID_ENV: &str = "AOMI_ADMIN_KID";

fn env_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn bin_name() -> String {
    std::env::args()
        .next()
        .and_then(|arg| {
            Path::new(&arg)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "aomi-build".to_string())
}

/// `--backend` flag → `AOMI_BACKEND_URL` → saved `connect` config.
fn resolve_backend(flag: &Option<String>) -> Option<String> {
    flag.clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| env_value(BACKEND_URL_ENV))
        .or_else(|| AomiConfig::load().backend_url)
}

/// `--activation-token` flag → `AOMI_APP_ACTIVATION_TOKEN` → saved `connect`
/// config. Lets a connected user run deploy/activate with no env wiring.
fn resolve_activation_token(flag: &Option<String>) -> Option<String> {
    flag.clone()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| env_value(ACTIVATION_TOKEN_ENV))
        .or_else(|| AomiConfig::load().activation_token)
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
    pub async fn run(self) -> Result<()> {
        let (git_root, start_dir) = git_context(&self.path)?;
        let platform = self.platform(&git_root, &start_dir);
        let source_ref = self.source_ref(&git_root)?;
        let aomi_toml_paths = self.aomi_toml_paths(&git_root)?;
        let app_source_id = self.resolve_app_source_id()?;

        let request = DeployInput {
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

        let state = LocalDeployment::from_deploy(response, app_source_id);
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
            let bin = bin_name();
            println!("  {bin} status --path {}", self.path.display());
            println!("  {bin} activate --path {}", self.path.display());
        }
        Ok(())
    }

    /// Dry-run: POST with `dry_run: true` when a backend + token are available
    /// (the backend resolves the branch and validates scope); otherwise print
    /// the request we would send, fully offline.
    async fn run_dry_run(&self, platform: &Platform, request: &DeployInput) -> Result<()> {
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
        // Resolution order: flag → env → the id recorded by a prior deploy /
        // `source sync` in `.aomi/deployment.json`. The last step is what lets a
        // re-deploy run with no `--app-source-id` once the source is known.
        if let Some(id) = self.app_source_id.filter(|id| *id > 0) {
            return Ok(id);
        }
        if let Some(id) = env_value(APP_SOURCE_ID_ENV)
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|id| *id > 0)
        {
            return Ok(id);
        }
        if let Some(id) = LocalDeployment::read(&self.path)
            .ok()
            .flatten()
            .and_then(|state| state.app_source_id())
            .filter(|id| *id > 0)
        {
            return Ok(id);
        }
        Err(anyhow!(
            "deploy needs --app-source-id (or {APP_SOURCE_ID_ENV}): the connected GitHub \
             App install to deploy from. Install the Aomi GitHub App on your source repo, \
             then run `aomi-build source sync --repo <owner/repo>` (or use the app_source id \
             ops/the portal issued you)."
        ))
    }
}

fn activation_token() -> Result<String> {
    resolve_activation_token(&None).ok_or_else(|| {
        anyhow!(
            "deploy requires an activation token via {ACTIVATION_TOKEN_ENV} \
             (or run `aomi-build connect`)"
        )
    })
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

    /// Activate explicit release tags. Repeat for multi-app activation. App
    /// names are optional; when provided, their count must match the tags.
    /// Defaults to the release tags recorded in `.aomi/deployment.json`.
    #[arg(long = "release-tag", value_name = "TAG")]
    pub release_tags: Vec<String>,

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
    pub async fn run(self) -> Result<()> {
        let mut state = LocalDeployment::read(&self.path)?.ok_or_else(|| {
            anyhow!(
                "no .aomi/deployment.json at {} — run `{} deploy` first",
                self.path.display(),
                bin_name()
            )
        })?;
        let platform = self
            .platform
            .clone()
            .unwrap_or_else(|| Platform::new(&state.deployment.platform.platform));

        let request = self.activation_request(&state)?;

        if self.dry_run {
            let printable = serde_json::json!({
                "endpoint": format!("/api/platforms/{}/apps/activate", platform.as_str()),
                "request": request,
            });
            println!("{}", serde_json::to_string_pretty(&printable)?);
            return Ok(());
        }

        let backend_url = resolve_backend(&self.backend).ok_or_else(|| {
            anyhow!("activate needs a backend URL — set --backend or {BACKEND_URL_ENV}")
        })?;
        let token = resolve_activation_token(&self.activation_token).ok_or_else(|| {
            anyhow!(
                "activate requires a token via --activation-token or {ACTIVATION_TOKEN_ENV} \
                 (or run `aomi-build connect`)"
            )
        })?;
        let client = BackendClient::new(backend_url, token)?;

        // One call activates every requested app; the response carries per-app
        // results with a partial-failure shape.
        let response = client.activate(&platform, &request).await?;
        state.apply_target_activation(&response);
        state.write(&self.path)?;

        if self.json {
            println!("{}", serde_json::to_string_pretty(&response)?);
        } else {
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

        let failures = response
            .activation
            .apps
            .iter()
            .filter(|a| a.error.is_some() || !a.loaded)
            .count();
        if failures > 0 {
            bail!("{failures} app(s) failed to activate");
        }
        Ok(())
    }

    pub(crate) fn activation_request(&self, state: &LocalDeployment) -> Result<ActivateInput> {
        let explicit_release_tags = clean_list(&self.release_tags);
        let has_explicit_release_tags = !explicit_release_tags.is_empty();
        let app_names = if has_explicit_release_tags && self.apps.is_empty() {
            Vec::new()
        } else if self.apps.is_empty() {
            state.app_names()
        } else {
            clean_list(&self.apps)
        };

        if has_explicit_release_tags
            && !app_names.is_empty()
            && app_names.len() != explicit_release_tags.len()
        {
            bail!("--release-tag activation requires the same number of apps and release tags");
        }

        let release_tags = if has_explicit_release_tags {
            explicit_release_tags
        } else {
            if app_names.is_empty() {
                bail!("no apps to activate — deploy first or pass --release-tag");
            }
            app_names
                .iter()
                .map(|app| {
                    state
                        .release_tag_for(app)
                        .map(str::to_string)
                        .ok_or_else(|| {
                            anyhow!("app `{app}` is not in .aomi/deployment.json; deploy it first")
                        })
                })
                .collect::<Result<Vec<_>>>()?
        };

        if release_tags.is_empty() {
            bail!("activation needs at least one release tag");
        }

        Ok(ActivateInput {
            target: ReleaseTags::new(release_tags),
            apps: app_names,
            target_tags: clean_list(&self.target_tags),
        })
    }
}

fn clean_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
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
    pub async fn run(self) -> Result<()> {
        let state = LocalDeployment::read(&self.path)?.ok_or_else(|| {
            anyhow!(
                "no .aomi/deployment.json at {} — run `{} deploy` first",
                self.path.display(),
                bin_name()
            )
        })?;

        // `--backend ''` explicitly opts out; otherwise flag/env.
        let backend_url = match &self.backend {
            Some(flag) if flag.trim().is_empty() => None,
            other => resolve_backend(other),
        };

        let report = StatusResult::collect(&state, backend_url).await;
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

// ---------------------------------------------------------------------------
// Bootstrap: token / source / scaffold / apps
//
// These wrap the platform-token + source-resolution endpoints that precede
// deploy. `token mint` signs a privileged admin AomiBearer locally (from
// `AOMI_ADMIN_KEY`); everything else runs on the activation token that mint
// produces.
// ---------------------------------------------------------------------------

/// `--backend`/env + activation token resolution shared by the activation-token
/// bootstrap commands (source/apps/token list+revoke).
fn resolve_activation(backend: &Option<String>, token: &Option<String>) -> Result<(String, String)> {
    let url = resolve_backend(backend)
        .ok_or_else(|| anyhow!("needs a backend URL — set --backend or {BACKEND_URL_ENV}"))?;
    let tok = resolve_activation_token(token).ok_or_else(|| {
        anyhow!(
            "needs an activation token via --activation-token or {ACTIVATION_TOKEN_ENV} \
             (or run `aomi-build connect`)"
        )
    })?;
    Ok((url, tok))
}

/// Record a resolved `app_source_id` into an existing `.aomi/deployment.json`
/// (if one exists) so the next deploy auto-resolves it. Returns the written
/// path, or `None` when there's no deployment record yet.
fn persist_app_source_id(path: &Path, app_source_id: i64) -> Option<PathBuf> {
    let mut state = LocalDeployment::read(path).ok().flatten()?;
    state.set_app_source_id(app_source_id);
    state.write(path).ok()
}

// ── token ───────────────────────────────────────────────────────────────────

#[derive(Debug, Args, Clone)]
pub struct TokenArgs {
    #[command(subcommand)]
    pub cmd: TokenCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum TokenCmd {
    /// Mint a platform/app activation token (signs an admin bearer from AOMI_ADMIN_KEY).
    Mint(TokenMintArgs),
    /// List a platform's activation tokens.
    List(TokenListArgs),
    /// Revoke a token by id.
    Revoke(TokenRevokeArgs),
}

impl TokenArgs {
    pub async fn run(self) -> Result<()> {
        match self.cmd {
            TokenCmd::Mint(a) => a.run().await,
            TokenCmd::List(a) => a.run().await,
            TokenCmd::Revoke(a) => a.run().await,
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct TokenMintArgs {
    /// Platform tag the token is scoped to.
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,

    /// Token scope: `platform` or `app`.
    #[arg(long, default_value = "platform")]
    pub scope: String,

    /// App id — required when `--scope app`.
    #[arg(long = "app-id", value_name = "ID")]
    pub app_id: Option<i64>,

    /// Backend base URL (default: `AOMI_BACKEND_URL`).
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// File holding the admin Ed25519 private key PEM. Falls back to
    /// `AOMI_ADMIN_KEY` (the PEM text itself, or a path to it).
    #[arg(long = "admin-key-file", value_name = "PATH")]
    pub admin_key_file: Option<PathBuf>,

    /// Admin issuer key id (e.g. `aomi-admin-staging-1`). Falls back to
    /// `AOMI_ADMIN_KID`.
    #[arg(long = "admin-kid", value_name = "KID")]
    pub admin_kid: Option<String>,

    /// Admin issuer name (the trusted `iss` on the backend).
    #[arg(long = "admin-iss", default_value = "aomi-admin")]
    pub admin_iss: String,

    /// Audience the backend accepts.
    #[arg(long = "admin-aud", default_value = "aomi-backend")]
    pub admin_aud: String,

    /// Subject recorded in the bearer.
    #[arg(long = "admin-sub", default_value = "aomi-build-cli")]
    pub admin_sub: String,

    /// Bearer lifetime, seconds.
    #[arg(long = "admin-ttl", value_name = "SECS", default_value_t = 900)]
    pub admin_ttl: i64,

    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

impl TokenMintArgs {
    pub async fn run(self) -> Result<()> {
        // Fail fast: minting needs the privileged signing key. Resolve it (and
        // the kid) before any network call so a missing credential is an
        // immediate, clear error rather than a 401 round-trip.
        let key = resolve_admin_key(&self.admin_key_file)?;
        let kid = self
            .admin_kid
            .clone()
            .or_else(|| env_value(ADMIN_KID_ENV))
            .ok_or_else(|| {
                anyhow!(
                    "`token mint` requires the admin issuer key id — set --admin-kid or \
                     {ADMIN_KID_ENV} (e.g. aomi-admin-staging-1)."
                )
            })?;

        let scope = self.scope.trim().to_string();
        if scope == "app" && self.app_id.is_none() {
            bail!("--scope app requires --app-id");
        }
        if scope != "app" && scope != "platform" {
            bail!("--scope must be `platform` or `app` (got `{scope}`)");
        }

        let backend_url = resolve_backend(&self.backend).ok_or_else(|| {
            anyhow!("token mint needs a backend URL — set --backend or {BACKEND_URL_ENV}")
        })?;

        let now = chrono::Utc::now().timestamp();
        let bearer = super::auth::AdminBearer {
            private_key_pem: &key,
            kid: &kid,
            iss: &self.admin_iss,
            aud: &self.admin_aud,
            sub: &self.admin_sub,
            ttl_secs: self.admin_ttl,
        }
        .sign(now)?;

        let request = MintTokenInput {
            scope: scope.clone(),
            app_id: self.app_id,
        };
        let result = BackendClient::new(backend_url, bearer)?
            .mint_token(&self.platform, &request)
            .await?;

        if self.json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "id": result.id,
                    "token": result.token,
                    "scope": result.scope,
                }))?
            );
        } else {
            println!(
                "Minted `{}` token id {} for platform `{}`",
                result.scope, result.id, self.platform
            );
            println!("  token: {}", result.token);
            println!(
                "  store this now — the backend keeps only the hash, the plaintext is shown once."
            );
            println!();
            println!("Use it for deploy/activate:");
            println!("  export {ACTIVATION_TOKEN_ENV}={}", result.token);
        }
        Ok(())
    }
}

/// Resolve the admin signing key: `--admin-key-file`, else `AOMI_ADMIN_KEY`
/// (PEM text, or a path to a PEM file). This is the privileged, out-of-band
/// signing key — not an activation token.
fn resolve_admin_key(file: &Option<PathBuf>) -> Result<Vec<u8>> {
    if let Some(path) = file {
        return std::fs::read(path)
            .with_context(|| format!("failed to read admin key file {}", path.display()));
    }
    let raw = env_value(ADMIN_KEY_ENV).ok_or_else(|| {
        anyhow!(
            "`token mint` requires a privileged admin signing key. Set {ADMIN_KEY_ENV} \
             (a PKCS#8 Ed25519 private key PEM) or pass --admin-key-file. This is an \
             out-of-band admin/service signing key, not an activation token."
        )
    })?;
    if raw.contains("BEGIN") {
        Ok(raw.into_bytes())
    } else {
        std::fs::read(&raw).with_context(|| {
            format!("{ADMIN_KEY_ENV} is neither a PEM nor a readable file path: {raw}")
        })
    }
}

#[derive(Debug, Args, Clone)]
pub struct TokenListArgs {
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
}

impl TokenListArgs {
    pub async fn run(self) -> Result<()> {
        let (url, token) = resolve_activation(&self.backend, &self.activation_token)?;
        let value = BackendClient::new(url, token)?
            .list_tokens(&self.platform)
            .await?;
        println!("{}", serde_json::to_string_pretty(&value)?);
        Ok(())
    }
}

#[derive(Debug, Args, Clone)]
pub struct TokenRevokeArgs {
    /// Token id to revoke.
    #[arg(value_name = "ID")]
    pub id: i64,
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
}

impl TokenRevokeArgs {
    pub async fn run(self) -> Result<()> {
        let (url, token) = resolve_activation(&self.backend, &self.activation_token)?;
        BackendClient::new(url, token)?
            .revoke_token(&self.platform, self.id)
            .await?;
        println!("Revoked token id {} on platform `{}`", self.id, self.platform);
        Ok(())
    }
}

// ── source ───────────────────────────────────────────────────────────────────

#[derive(Debug, Args, Clone)]
pub struct SourceArgs {
    #[command(subcommand)]
    pub cmd: SourceCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum SourceCmd {
    /// Resolve-or-bind an installed source repo and print its `app_source_id`.
    Sync(SourceSyncArgs),
}

impl SourceArgs {
    pub async fn run(self) -> Result<()> {
        match self.cmd {
            SourceCmd::Sync(a) => a.run().await,
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct SourceSyncArgs {
    /// Source repo, `owner/name`.
    #[arg(long, value_name = "OWNER/REPO")]
    pub repo: String,
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
    /// Source repo path for `.aomi/deployment.json` persistence.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub json: bool,
}

impl SourceSyncArgs {
    pub async fn run(self) -> Result<()> {
        let repo = normalize_github_repo(&self.repo)?;
        let (url, token) = resolve_activation(&self.backend, &self.activation_token)?;
        let result = BackendClient::new(url, token)?
            .sync_installed(&self.platform, &SyncSourceInput { repo: repo.clone() })
            .await?;
        report_source(&result, &self.path, self.json, "synced")
    }
}

/// Shared output + persistence for `source sync` / `scaffold` — both return a
/// resolved `app_source` whose id deploy needs.
fn report_source(
    result: &super::types::SourceResult,
    path: &Path,
    json: bool,
    verb: &str,
) -> Result<()> {
    let id = result.source.id;
    let persisted = persist_app_source_id(path, id);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "app_source_id": id,
                "repository_link": result.source.repository_link,
                "installation_id": result.source.installation_id,
                "bound_platform_id": result.source.bound_platform_id,
                "persisted": persisted.is_some(),
            }))?
        );
    } else {
        println!(
            "{verb} source `{}` (installation {})",
            result.source.repository_link, result.source.installation_id
        );
        println!("  app_source_id: {id}");
        match persisted {
            Some(p) => println!("  recorded in {} — deploy will auto-resolve it", p.display()),
            None => {
                println!("  no .aomi/deployment.json yet; pass it to the first deploy:");
                println!("    aomi-build deploy --app-source-id {id}   (or export {APP_SOURCE_ID_ENV}={id})");
            }
        }
    }
    Ok(())
}

// ── scaffold ─────────────────────────────────────────────────────────────────

#[derive(Debug, Args, Clone)]
pub struct ScaffoldArgs {
    /// Name of the new repo to create in the connected account.
    #[arg(long = "repo-name", value_name = "NAME")]
    pub repo_name: String,

    /// The one-shot GitHub App installation to create the repo under.
    #[arg(long = "installation-id", value_name = "ID")]
    pub installation_id: i64,

    #[arg(long, value_name = "NAME")]
    pub platform: Platform,

    /// Template repo to fork from.
    #[arg(long, default_value = "aomi-labs/playground-example")]
    pub template: String,

    /// Create the new repo as private.
    #[arg(long)]
    pub private: bool,

    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
    /// Source repo path for `.aomi/deployment.json` persistence.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub json: bool,
}

impl ScaffoldArgs {
    pub async fn run(self) -> Result<()> {
        let (url, token) = resolve_activation(&self.backend, &self.activation_token)?;
        let request = CreateTemplateInput {
            installation_id: self.installation_id,
            template_repo: self.template.clone(),
            repo_name: self.repo_name.clone(),
            private: self.private,
        };
        let result = BackendClient::new(url, token)?
            .create_from_template(&self.platform, &request)
            .await?;
        report_source(&result, &self.path, self.json, "created")
    }
}

// ── apps ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Args, Clone)]
pub struct AppsArgs {
    #[command(subcommand)]
    pub cmd: AppsCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum AppsCmd {
    /// List a platform's apps.
    List(AppsListArgs),
}

impl AppsArgs {
    pub async fn run(self) -> Result<()> {
        match self.cmd {
            AppsCmd::List(a) => a.run().await,
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct AppsListArgs {
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
}

impl AppsListArgs {
    pub async fn run(self) -> Result<()> {
        let (url, token) = resolve_activation(&self.backend, &self.activation_token)?;
        let value = BackendClient::new(url, token)?
            .list_apps(&self.platform)
            .await?;
        println!("{}", serde_json::to_string_pretty(&value)?);
        Ok(())
    }
}

// ── connect ───────────────────────────────────────────────────────────────────

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

    /// Activation token to store (issued by your Aomi admin). Prompted if omitted.
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// Print the install URL without opening a browser.
    #[arg(long)]
    pub no_browser: bool,

    /// Skip polling and paste the installation_id manually.
    #[arg(long)]
    pub manual: bool,

    /// How long to wait for the browser install before falling back to manual.
    #[arg(long = "timeout-secs", value_name = "SECS", default_value_t = 300)]
    pub timeout_secs: u64,
}

fn prompt_installation_id() -> Result<i64> {
    inquire::Text::new("After installing, paste your installation_id:")
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

        // 1. GitHub App install URL.
        let start =
            super::flow::oauth_install_url(&backend_url, self.platform.as_str(), repo.as_deref())
                .await?;
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
        println!();

        // 2. Capture the installation: explicit flag → poll-by-state (wait for
        //    the browser install) → manual paste fallback.
        let installation_id: i64 = if let Some(id) = self.installation_id {
            id
        } else if !self.manual {
            match start.state.as_deref() {
                Some(state) => {
                    println!(
                        "Waiting for the install to finish in your browser (up to {}s, Ctrl-C to cancel)…",
                        self.timeout_secs
                    );
                    match super::flow::poll_installation(
                        &backend_url,
                        state,
                        std::time::Duration::from_secs(self.timeout_secs),
                    )
                    .await?
                    {
                        Some(id) => {
                            println!("  detected installation_id {id}");
                            id
                        }
                        None => {
                            println!("  timed out waiting — enter it manually.");
                            prompt_installation_id()?
                        }
                    }
                }
                None => prompt_installation_id()?,
            }
        } else {
            prompt_installation_id()?
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
        if super::flow::validate_activation_token(&backend_url, &token, self.platform.as_str()).await
        {
            println!("  token verified for platform `{}`", self.platform);
        } else {
            println!(
                "  warning: could not verify the token against `{}` — saving anyway",
                self.platform
            );
        }

        // 4. Persist account identity.
        let path = AomiConfig {
            backend_url: Some(backend_url),
            platform: Some(self.platform.to_string()),
            installation_id: Some(installation_id),
            activation_token: Some(token),
        }
        .save()?;

        println!();
        println!("Connected. Saved to {}", path.display());
        println!("  installation_id: {installation_id}");
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
