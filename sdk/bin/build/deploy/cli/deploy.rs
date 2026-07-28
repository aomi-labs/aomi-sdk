//! `deploy` — full hosted app deploy lifecycle plus explicit deploy steps.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};

use super::login::ensure_logged_in;
use super::shared::{
    ACTIVATION_TOKEN_ENV, APP_SOURCE_ID_ENV, BUILD_TOKEN_ENV, BUILD_URL_ENV, bin_name, env_value,
    git_context, head_commit, remote_origin, resolve_backend, resolve_build_url,
    tracked_aomi_tomls,
};
use super::{ActivateArgs, StatusArgs};
use crate::deploy::app::AomiAppFiles;
use crate::deploy::build_client::BuildClient;
use crate::deploy::config::AomiConfig;
use crate::deploy::flow;
use crate::deploy::platform::{Platform, normalize_github_repo};
use crate::deploy::types::{BuildDeployInput, LocalDeployment};

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
    let timeout_message = format!(
        "deployment did not become ready within 30 minutes; rerun `{} deploy activate --path {}` later",
        bin_name(),
        args.path.display()
    );
    if args.activation_token.is_some() || env_value(ACTIVATION_TOKEN_ENV).is_some() {
        let backend_url = resolve_backend(&args.backend)
            .ok_or_else(|| anyhow!("deploy activate needs a backend URL"))?;
        let token = args
            .activation_token
            .clone()
            .or_else(|| env_value(ACTIVATION_TOKEN_ENV))
            .ok_or_else(|| anyhow!("deploy activate needs an activation token"))?;
        wait_for_backend_release(
            &backend_url,
            &token,
            &platform,
            &state.deployment.id,
            state.deployment.platform.pr_url.as_deref(),
            timeout_message,
        )
        .await?;
    } else {
        let backend_url = resolve_backend(&args.backend);
        let build_url = resolve_build_url(&args.build_url, backend_url.as_deref())
            .ok_or_else(|| anyhow!("deploy activate needs --build-url or {BUILD_URL_ENV}"))?;
        let client = ensure_logged_in(&build_url).await?.client;
        wait_for_build_release(
            &client,
            &platform,
            &state.deployment.id,
            state.deployment.platform.pr_url.as_deref(),
            timeout_message,
        )
        .await?;
    }
    args.run().await
}

async fn wait_for_backend_release(
    backend_url: &str,
    token: &str,
    platform: &Platform,
    deployment_id: &str,
    pr_url: Option<&str>,
    timeout_message: String,
) -> Result<()> {
    println!("Waiting for release readiness...");
    match flow::poll_deployment_ready_with_pr(
        backend_url,
        token,
        platform.as_str(),
        deployment_id,
        pr_url,
        Duration::from_secs(30 * 60),
        |status| println!("  build         : {status}"),
    )
    .await?
    {
        flow::DeployReady::Ready => println!("Release is ready."),
        flow::DeployReady::Failed(msg) => bail!("deployment failed before activation: {msg}"),
        flow::DeployReady::NoCi(msg) => bail!("{}", no_ci_message(&msg, pr_url)),
        flow::DeployReady::TimedOut => bail!("{timeout_message}"),
    }
    Ok(())
}

/// `no_ci` is not a build failure — nothing broke, nothing ran. Point at the
/// deploy PR, since a missing or unpicked-up release workflow on the platform
/// repo is the usual cause.
fn no_ci_message(detail: &str, pr_url: Option<&str>) -> String {
    format!(
        "no CI ran for this deployment, so there is no release to activate: {detail}\n\
         Check that the platform repo's deploy PR has a release workflow: {}",
        pr_url.unwrap_or("(no PR URL recorded)")
    )
}

async fn wait_for_build_release(
    client: &BuildClient,
    platform: &Platform,
    deployment_id: &str,
    pr_url: Option<&str>,
    timeout_message: String,
) -> Result<()> {
    println!("Waiting for release readiness...");
    match flow::poll_build_deployment_ready(
        client,
        platform.as_str(),
        deployment_id,
        Duration::from_secs(30 * 60),
        |status| println!("  build         : {status}"),
    )
    .await?
    {
        flow::DeployReady::Ready => println!("Release is ready."),
        flow::DeployReady::Failed(msg) => bail!("deployment failed before activation: {msg}"),
        flow::DeployReady::NoCi(msg) => bail!("{}", no_ci_message(&msg, pr_url)),
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

    /// Aomi Build URL (default: `AOMI_BUILD_URL`, saved login, or inferred from
    /// the backend environment).
    #[arg(long = "build-url", value_name = "URL")]
    pub build_url: Option<String>,

    /// Explicit admin/headless activation token for the lifecycle's activation
    /// step. Human deploys always use the Builder login.
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// Backend server tag for explicit admin/headless activation.
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
        let platform = self.platform(&git_root, &start_dir)?;
        let backend_url = resolve_backend(&self.backend);
        crate::sdk_guard::ensure_project_sdk(&git_root, backend_url.as_deref(), self.fix_sdk)
            .await?;
        let build_url = self.build_url(backend_url.as_deref())?;
        let client = ensure_logged_in(&build_url).await?.client;
        let mut request = self.build_request(&git_root, &platform)?;

        let preflight = client.deploy(&request, true).await.map_err(|error| {
            explain_deploy_error(error, &platform, &request.source_ref, &build_url)
        })?;
        println!("Preflight passed for platform `{platform}`.");
        println!(
            "  source_commit : {}",
            preflight.deployment.source.commit_hash
        );
        for app in &preflight.deployment.platform.apps {
            println!("  - {} -> {}", app.name, app.release_tag);
        }

        request.app_source_id = Some(preflight.app_source_id);
        let deploy = client.deploy(&request, false).await.map_err(|error| {
            explain_deploy_error(error, &platform, &request.source_ref, &build_url)
        })?;
        let mut state = LocalDeployment::from_build_deploy(deploy);
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
        if let Some(project_url) = &state.project_url {
            println!("  project       : {project_url}");
        }

        wait_for_build_release(
            &client,
            &platform,
            &state.deployment.id,
            state.deployment.platform.pr_url.as_deref(),
            format!(
                "deployment did not become ready within 30 minutes; resume with `{} deploy activate --path {}`",
                bin_name(),
                self.path.display()
            ),
        )
        .await?;

        let activate_args = ActivateArgs {
            platform: Some(platform),
            backend: backend_url,
            build_url: Some(build_url),
            activation_token: self.activation_token.clone(),
            target_tags: self.target_tags.clone(),
            path: self.path.clone(),
            json: self.json,
            fix_sdk: self.fix_sdk,
            ..Default::default()
        };
        let response = activate_args
            .activate_with_state(&git_root, &mut state)
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
        let platform = self.platform(&git_root, &start_dir)?;
        let backend_url = resolve_backend(&self.backend);
        crate::sdk_guard::ensure_project_sdk(&git_root, backend_url.as_deref(), self.fix_sdk)
            .await?;
        let build_url = self.build_url(backend_url.as_deref())?;
        let client = ensure_logged_in(&build_url).await?.client;
        let mut request = self.build_request(&git_root, &platform)?;
        let response = if preflight || request.app_source_id.is_none() {
            let preflight_response = client.deploy(&request, true).await.map_err(|error| {
                explain_deploy_error(error, &platform, &request.source_ref, &build_url)
            })?;
            if preflight {
                println!("{}", serde_json::to_string_pretty(&preflight_response)?);
                return Ok(());
            }
            request.app_source_id = Some(preflight_response.app_source_id);
            client.deploy(&request, false).await
        } else {
            client.deploy(&request, false).await
        }
        .map_err(|error| explain_deploy_error(error, &platform, &request.source_ref, &build_url))?;

        let state = LocalDeployment::from_build_deploy(response);
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
            if let Some(project_url) = &state.project_url {
                println!("  project       : {project_url}");
            }
            println!();
            println!("Next: track CI, then activate once it is green:");
            let bin = bin_name();
            println!("  {bin} status --path {}", self.path.display());
            println!("  {bin} activate --path {}", self.path.display());
        }
        Ok(())
    }

    fn build_request(&self, git_root: &Path, platform: &Platform) -> Result<BuildDeployInput> {
        let repo = match self.repo.as_deref() {
            Some(repo) => normalize_github_repo(repo)?,
            None => normalize_github_repo(&remote_origin(git_root)?)?,
        };
        Ok(BuildDeployInput {
            platform: platform.to_string(),
            source_ref: self.source_ref(git_root)?,
            aomi_toml_paths: self.aomi_toml_paths(git_root)?,
            app_source_id: self.resolve_app_source_id(git_root, &repo),
            repo,
        })
    }

    fn build_url(&self, backend_url: Option<&str>) -> Result<String> {
        resolve_build_url(&self.build_url, backend_url)
            .ok_or_else(|| anyhow!("deploy needs --build-url or {BUILD_URL_ENV}"))
    }

    /// Documented precedence: `--platform` flag, then the platform declared by
    /// the deployed `aomi.toml` manifests, then saved config, then `community`.
    pub(crate) fn platform(&self, git_root: &Path, start_dir: &Path) -> Result<Platform> {
        self.resolve_platform(git_root, start_dir, AomiConfig::load().platform)
    }

    /// `platform` with the saved-config value injected so tests don't depend
    /// on the machine's `~/.config/aomi/config.toml`.
    pub(crate) fn resolve_platform(
        &self,
        git_root: &Path,
        start_dir: &Path,
        saved_platform: Option<String>,
    ) -> Result<Platform> {
        if let Some(p) = &self.platform {
            return Ok(p.clone());
        }
        if let Some(platform) = self.manifest_platform(git_root)? {
            return Ok(platform);
        }
        Ok(AomiAppFiles::discover(start_dir, git_root)
            .ok()
            .and_then(|a| a.platform)
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
            .map(Platform::new)
            .or_else(|| saved_platform.map(Platform::new))
            .unwrap_or_else(Platform::community))
    }

    /// Platform declared by the manifests this deploy actually ships — the
    /// `--aomi-toml` set, or every tracked `aomi.toml` when the flag is absent.
    /// `None` when no manifest in the set declares one; an error when the set
    /// disagrees, since one deploy targets exactly one platform.
    fn manifest_platform(&self, git_root: &Path) -> Result<Option<Platform>> {
        let Ok(paths) = self.aomi_toml_paths(git_root) else {
            // No deployable manifest set (e.g. nothing tracked yet); the deploy
            // steps surface that error where it matters.
            return Ok(None);
        };
        let mut declared: Vec<(String, Platform)> = Vec::new();
        for path in paths {
            let Some(platform) = AomiAppFiles::from_aomi_toml(&git_root.join(&path), git_root)
                .ok()
                .and_then(|app| app.platform)
                .map(Platform::new)
            else {
                continue;
            };
            if !declared.iter().any(|(_, seen)| *seen == platform) {
                declared.push((path, platform));
            }
        }
        if declared.len() > 1 {
            let listing = declared
                .iter()
                .map(|(path, platform)| format!("  {path} -> {platform}"))
                .collect::<Vec<_>>()
                .join("\n");
            bail!(
                "the aomi.toml manifests in this deploy declare conflicting platforms:\n{listing}\n\
                 A deploy targets one platform; align `[app].platform`, scope the deploy with --aomi-toml, or pass --platform."
            );
        }
        Ok(declared.pop().map(|(_, platform)| platform))
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

    /// Resolution order: flag → env → the id recorded by a prior deploy /
    /// `source sync` in `.aomi/deployment.json`. The last step is what lets a
    /// re-deploy run with no `--app-source-id` once the source is known.
    ///
    /// `repo` is the source the caller actually asked to deploy. The backend
    /// resolves the repo *from* `app_source_id`, so a recorded id belonging to a
    /// different repo would silently win over that request — making the wizard's
    /// "Source repo (owner/name)" answer a no-op. When they disagree, the
    /// recorded id is dropped so preflight re-resolves the source from `repo`.
    pub(crate) fn resolve_app_source_id(&self, git_root: &Path, repo: &str) -> Option<i64> {
        if let Some(id) = self.app_source_id.filter(|id| *id > 0) {
            return Some(id);
        }
        if let Some(id) = env_value(APP_SOURCE_ID_ENV)
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|id| *id > 0)
        {
            return Some(id);
        }
        let state = LocalDeployment::read(git_root).ok().flatten()?;
        let id = state.app_source_id().filter(|id| *id > 0)?;
        let recorded_repo = state
            .source_repo_hint()
            .and_then(|hint| normalize_github_repo(hint).ok());
        match recorded_repo {
            // Recorded against a different repo — re-resolve rather than deploy
            // the wrong source under the user's nose.
            Some(recorded) if !recorded.eq_ignore_ascii_case(repo) => {
                eprintln!(
                    "  note: .aomi/deployment.json records app_source_id {id} for `{recorded}`, \
                     but this deploy targets `{repo}` — re-resolving the source."
                );
                None
            }
            _ => Some(id),
        }
    }
}

fn explain_deploy_error(
    err: anyhow::Error,
    platform: &Platform,
    source_ref: &str,
    build_url: &str,
) -> anyhow::Error {
    let message = err.to_string();
    if !(message.contains("403 Forbidden") || message.contains("401 Unauthorized")) {
        return err;
    }
    anyhow!(
        "{message}\n\n\
         Deploy authorization needs a verified Builder login that owns this GitHub source.\n\
         Platform: `{platform}`\n\
         Source: commit `{source_ref}`\n\
         Log in again with:\n\
           aomi-build login --build-url {build_url}\n\
         Headless automation may set {BUILD_TOKEN_ENV}."
    )
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
