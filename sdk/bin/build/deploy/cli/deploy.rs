//! `deploy` — full hosted app deploy lifecycle plus explicit deploy steps.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};

use super::shared::{
    ACTIVATION_TOKEN_ENV, APP_SOURCE_ID_ENV, BACKEND_URL_ENV, CredentialSource, bin_name,
    clean_list, env_value, git_context, head_commit, resolve_activation_token,
    resolve_activation_token_with_source, resolve_backend, tracked_aomi_tomls,
};
use super::{ActivateArgs, StatusArgs};
use crate::deploy::app::AomiAppFiles;
use crate::deploy::backend::BackendClient;
use crate::deploy::config::AomiConfig;
use crate::deploy::flow;
use crate::deploy::platform::Platform;
use crate::deploy::types::{DeployInput, LocalDeployment};

#[derive(Debug, Args, Clone)]
pub struct DeployArgs {
    #[command(subcommand)]
    pub cmd: Option<DeployCmd>,

    #[command(flatten)]
    pub step: DeployStepArgs,
}

#[derive(Debug, Subcommand, Clone)]
pub enum DeployCmd {
    /// Validate backend/source/app inputs without writing the platform repo.
    Preflight(DeployStepArgs),
    /// Create or update the platform deployment and write `.aomi/deployment.json`.
    Run(DeployStepArgs),
    /// Wait for release readiness, activate release tags, and verify loaded state.
    Activate(ActivateArgs),
    /// Show local + backend deployment status.
    Status(StatusArgs),
}

impl DeployArgs {
    pub async fn run(self) -> Result<()> {
        match self.cmd {
            Some(DeployCmd::Preflight(args)) => args.run_preflight_command().await,
            Some(DeployCmd::Run(args)) => args.run_deploy_command().await,
            Some(DeployCmd::Activate(args)) => run_activate_step(args).await,
            Some(DeployCmd::Status(args)) => args.run().await,
            None if self.step.preflight => self.step.run_preflight_command().await,
            None => self.step.run_full_lifecycle().await,
        }
    }
}

pub(crate) async fn run_activate_step(args: ActivateArgs) -> Result<()> {
    if args.dry_run {
        return args.run().await;
    }
    let (git_root, _) = git_context(&args.path)?;
    let state = LocalDeployment::read(&git_root)?.ok_or_else(|| {
        anyhow!(
            "no .aomi/deployment.json at {} — run `{} deploy run` first",
            git_root.display(),
            bin_name()
        )
    })?;
    let platform = args
        .platform
        .clone()
        .unwrap_or_else(|| Platform::new(&state.deployment.platform.platform));
    let backend_url = resolve_backend(&args.backend).ok_or_else(|| {
        anyhow!("activate needs a backend URL — set --backend or {BACKEND_URL_ENV}")
    })?;
    let token = resolve_activation_token(&args.activation_token).ok_or_else(|| {
        anyhow!(
            "activate requires a token via --activation-token or {ACTIVATION_TOKEN_ENV} \
             (or run `aomi-build connect`)"
        )
    })?;
    wait_for_release(
        &backend_url,
        &token,
        &platform,
        &state.deployment.id,
        format!(
            "deployment did not become ready within 30 minutes; rerun `{} deploy activate --path {}` later",
            bin_name(),
            args.path.display()
        ),
    )
    .await?;
    args.run().await
}

async fn wait_for_release(
    backend_url: &str,
    token: &str,
    platform: &Platform,
    deployment_id: &str,
    timeout_message: String,
) -> Result<()> {
    println!("Waiting for release readiness...");
    match flow::poll_deployment_ready(
        backend_url,
        token,
        platform.as_str(),
        deployment_id,
        Duration::from_secs(30 * 60),
        |status| println!("  build         : {status}"),
    )
    .await?
    {
        flow::DeployReady::Ready => println!("Release is ready."),
        flow::DeployReady::Failed(msg) => bail!("deployment failed before activation: {msg}"),
        flow::DeployReady::TimedOut => bail!("{timeout_message}"),
    }
    Ok(())
}

#[derive(Debug, Args, Clone, Default)]
pub struct DeployStepArgs {
    /// Platform tag (`aomi.toml [app].platform`). Defaults to aomi.toml, then
    /// saved config, then `community`.
    #[arg(long, value_name = "NAME")]
    pub platform: Option<Platform>,

    /// Source repository to sync when no app_source_id is known, as `owner/repo`.
    #[arg(long, value_name = "OWNER/REPO")]
    pub repo: Option<String>,

    /// The connected GitHub App install (`app_source`) to deploy from. The
    /// backend resolves the source repo from it. Falls back to
    /// `AOMI_APP_SOURCE_ID`.
    #[arg(long = "app-source-id", value_name = "ID")]
    pub app_source_id: Option<i64>,

    /// Deprecated. Backend deploy accepts immutable commits only; checkout the branch locally.
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

    /// Activation token (default: `AOMI_APP_ACTIVATION_TOKEN`).
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// Backend server tag to activate onto. Defaults to `/api/platforms/server-tags`.
    #[arg(long = "target-tag", value_name = "TAG")]
    pub target_tags: Vec<String>,

    /// App source directory. Defaults to the current directory.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Preview the deployment plan without opening a PR.
    #[arg(long, alias = "dry-run")]
    pub preflight: bool,

    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,

    /// Rewrite Cargo.toml/Cargo.lock to the backend-required aomi-sdk version
    /// before deploying when a mismatch is detected.
    #[arg(long)]
    pub fix_sdk: bool,
}

impl DeployStepArgs {
    pub async fn run_full_lifecycle(self) -> Result<()> {
        let (git_root, start_dir) = git_context(&self.path)?;
        let platform = self.platform(&git_root, &start_dir);
        let backend_url = self.backend_url()?;
        crate::sdk_guard::ensure_project_sdk(&start_dir, Some(&backend_url), self.fix_sdk).await?;

        let source_ref = self.source_ref(&git_root)?;
        let aomi_toml_paths = self.aomi_toml_paths(&git_root)?;
        let app_source_id = self.resolve_app_source_id(&git_root, &platform).await?;
        let request = DeployInput {
            app_source_id,
            source_ref: source_ref.clone(),
            aomi_toml_paths,
            preflight: true,
        };

        let (token, token_source) = self.activation_token_with_source()?;
        let client = BackendClient::new(backend_url.clone(), token.clone())?;

        let preflight = client.deploy(&platform, &request).await.map_err(|e| {
            self.explain_deploy_error(e, &platform, app_source_id, &source_ref, token_source)
        })?;
        println!("Preflight passed for platform `{platform}`.");
        println!(
            "  source_commit : {}",
            preflight.deployment.source.commit_hash
        );
        for app in &preflight.deployment.platform.apps {
            println!("  - {} -> {}", app.name, app.release_tag);
        }

        let mut deploy_request = request;
        deploy_request.preflight = false;
        let deploy = client
            .deploy(&platform, &deploy_request)
            .await
            .map_err(|e| {
                self.explain_deploy_error(e, &platform, app_source_id, &source_ref, token_source)
            })?;
        let mut state = LocalDeployment::from_deploy(deploy, app_source_id);
        let path = state.write(&git_root)?;
        println!("Deployment started.");
        println!("  id            : {}", state.deployment.id);
        println!(
            "  pr            : {}",
            state
                .deployment
                .platform
                .pr_url
                .as_deref()
                .unwrap_or("(pending CI)")
        );
        println!("  deployment    : {}", path.display());

        wait_for_release(
            &backend_url,
            &token,
            &platform,
            &state.deployment.id,
            format!(
                "deployment did not become ready within 30 minutes; resume with `{} deploy activate --path {}`",
                bin_name(),
                self.path.display()
            ),
        )
        .await?;

        let target_tags = self.activation_target_tags(&backend_url).await?;
        let activate_args = ActivateArgs {
            platform: Some(platform),
            backend: Some(backend_url),
            activation_token: Some(token),
            target_tags,
            path: self.path.clone(),
            json: self.json,
            fix_sdk: self.fix_sdk,
            ..Default::default()
        };
        let response = activate_args
            .activate_with_state(&start_dir, &mut state)
            .await?;
        state.write(&git_root)?;
        ActivateArgs::print_activation(&response, self.json)?;
        if !self.json {
            println!(
                "Deployment verified: all activated apps are active, artifact-ready, and loaded."
            );
        }
        Ok(())
    }

    pub async fn run_preflight_command(self) -> Result<()> {
        self.run_step(true).await
    }

    pub async fn run_deploy_command(self) -> Result<()> {
        self.run_step(false).await
    }

    async fn run_step(self, preflight: bool) -> Result<()> {
        let (git_root, start_dir) = git_context(&self.path)?;
        let platform = self.platform(&git_root, &start_dir);
        let source_ref = self.source_ref(&git_root)?;
        let aomi_toml_paths = self.aomi_toml_paths(&git_root)?;
        let app_source_id = self.resolve_app_source_id(&git_root, &platform).await?;

        let request = DeployInput {
            app_source_id,
            source_ref: source_ref.clone(),
            aomi_toml_paths,
            preflight,
        };

        if preflight {
            return self.run_preflight(&platform, &request).await;
        }

        let backend_url = self.backend_url()?;
        crate::sdk_guard::ensure_project_sdk(&start_dir, Some(&backend_url), self.fix_sdk).await?;
        let (token, token_source) = self.activation_token_with_source()?;
        let response = BackendClient::new(backend_url, token)?
            .deploy(&platform, &request)
            .await
            .map_err(|e| {
                self.explain_deploy_error(e, &platform, app_source_id, &source_ref, token_source)
            })?;

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

    fn explain_deploy_error(
        &self,
        err: anyhow::Error,
        platform: &Platform,
        app_source_id: i64,
        source_ref: &str,
        token_source: CredentialSource,
    ) -> anyhow::Error {
        let msg = err.to_string();
        if !(msg.contains("403 Forbidden") || msg.contains("401 Unauthorized")) {
            return err;
        }
        anyhow!(
            "{msg}\n\n\
             Deploy authorization needs:\n\
               - a valid activation token for platform `{platform}` (using {})\n\
               - a connected GitHub App source id for this repo (`app_source_id: {app_source_id}`)\n\
               - the source ref pushed to GitHub (commit `{source_ref}`)\n\
             {}\n\
             To refresh the source id, run:\n\
               aomi-build source sync --repo <owner/repo>\n\
             To refresh credentials, run:\n\
               aomi-build connect --platform {platform}",
            token_source.label(),
            token_source.stale_hint()
        )
    }

    /// Preflight: POST with `preflight: true`; the backend validates source
    /// commit scope and renders the deployment record without platform writes.
    async fn run_preflight(&self, platform: &Platform, request: &DeployInput) -> Result<()> {
        let url = self.backend_url()?;
        let (token, _) = self.activation_token_with_source()?;
        crate::sdk_guard::ensure_project_sdk(&self.sdk_project_path()?, Some(&url), self.fix_sdk)
            .await?;
        let response = BackendClient::new(url, token)?
            .deploy(platform, request)
            .await?;
        println!("{}", serde_json::to_string_pretty(&response)?);
        Ok(())
    }

    pub(crate) fn sdk_project_path(&self) -> Result<PathBuf> {
        Ok(git_context(&self.path)?.1)
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
            .or_else(|| AomiConfig::load().platform.map(Platform::new))
            .unwrap_or_else(Platform::community)
    }

    pub(crate) fn source_ref(&self, git_root: &Path) -> Result<String> {
        if self
            .branch
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
            .is_some()
        {
            bail!(
                "--branch is not supported by the current backend deploy contract; checkout the branch locally or pass --commit with a resolved SHA"
            );
        }
        if let Some(commit) = self
            .commit
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            return validate_source_commit(commit);
        }
        validate_source_commit(&head_commit(git_root)?)
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

    async fn activation_target_tags(&self, backend_url: &str) -> Result<Vec<String>> {
        let explicit = clean_list(&self.target_tags);
        if !explicit.is_empty() {
            return Ok(explicit);
        }
        fetch_server_tags(backend_url).await
    }

    pub(crate) async fn resolve_app_source_id(
        &self,
        git_root: &Path,
        platform: &Platform,
    ) -> Result<i64> {
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
        if let Some(id) = self.recorded_app_source_id(git_root) {
            return Ok(id);
        }
        if let Some(repo) = self
            .repo
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
        {
            let repo = crate::deploy::platform::normalize_github_repo(repo)?;
            let url = self.backend_url()?;
            let (token, _) = self.activation_token_with_source()?;
            let result = BackendClient::new(url, token)?
                .sync_installed(
                    platform,
                    &crate::deploy::types::SyncSourceInput { repo: repo.clone() },
                )
                .await?;
            println!(
                "Resolved source `{}` to app_source_id {}.",
                result.source.repository_link, result.source.id
            );
            return Ok(result.source.id);
        }
        Err(anyhow!(
            "deploy needs --app-source-id (or {APP_SOURCE_ID_ENV}): the connected GitHub \
             App install to deploy from. Install the Aomi GitHub App on your source repo, \
             then run `aomi-build source sync --repo <owner/repo> --platform {platform}` \
             or pass --repo <owner/repo> to let deploy sync it."
        ))
    }

    pub(crate) fn recorded_app_source_id(&self, git_root: &Path) -> Option<i64> {
        LocalDeployment::read(git_root)
            .ok()
            .flatten()
            .and_then(|state| state.app_source_id())
            .filter(|id| *id > 0)
    }
    fn activation_token_with_source(&self) -> Result<(String, CredentialSource)> {
        resolve_activation_token_with_source(&self.activation_token).ok_or_else(|| {
            anyhow!(
                "deploy requires an activation token via --activation-token or {ACTIVATION_TOKEN_ENV} \
                 (or run `aomi-build connect`)"
            )
        })
    }
}

async fn fetch_server_tags(backend_url: &str) -> Result<Vec<String>> {
    let endpoint = format!(
        "{}/api/platforms/server-tags",
        backend_url.trim_end_matches('/')
    );
    let value: serde_json::Value = reqwest::Client::new()
        .get(&endpoint)
        .send()
        .await
        .map_err(|e| anyhow!("failed to call server-tags endpoint {endpoint}: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("server-tags endpoint {endpoint} returned invalid JSON: {e}"))?;
    Ok(value
        .get("server_tags")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect())
}

fn validate_source_commit(value: &str) -> Result<String> {
    let commit = value.trim().to_ascii_lowercase();
    if (7..=40).contains(&commit.len()) && commit.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(commit)
    } else {
        bail!("source commit must be a git commit SHA (7-40 hex chars), got `{value}`")
    }
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
