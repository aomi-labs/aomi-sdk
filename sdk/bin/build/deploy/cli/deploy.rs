//! `deploy` — deploy tracked `aomi.toml` apps from a source ref through the
//! backend.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use clap::Args;

use super::login::ensure_logged_in;
use super::shared::{
    APP_SOURCE_ID_ENV, BUILD_TOKEN_ENV, BUILD_URL_ENV, bin_name, env_value, git_context,
    head_commit, remote_origin, resolve_backend, resolve_build_token, resolve_build_url,
    tracked_aomi_tomls,
};
use crate::deploy::app::AomiAppFiles;
use crate::deploy::build_client::BuildClient;
use crate::deploy::platform::Platform;
use crate::deploy::platform::normalize_github_repo;
use crate::deploy::types::{BuildDeployInput, LocalDeployment};

pub async fn run(args: DeployArgs) -> eyre::Result<()> {
    args.run().await.map_err(crate::git_error)
}

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

    /// App source directory. Defaults to the current directory.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Preview the deployment plan without opening a PR.
    #[arg(long, alias = "dry-run")]
    pub preflight: bool,

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
        let app_source_id = self.resolve_app_source_id(&git_root);
        let repo = normalize_github_repo(&remote_origin(&git_root)?)?;
        let backend_url = resolve_backend(&self.backend);
        let build_url =
            resolve_build_url(&self.build_url, backend_url.as_deref()).ok_or_else(|| {
                anyhow!("deploy needs an Aomi Build URL — set --build-url or {BUILD_URL_ENV}")
            })?;
        ensure_logged_in(&build_url).await?;
        let token = resolve_build_token().ok_or_else(|| {
            anyhow!("Builder login did not save a session; run `aomi-build login`")
        })?;
        let client = BuildClient::new(&build_url, token)?;
        let mut request = BuildDeployInput {
            platform: platform.to_string(),
            repo,
            source_ref: source_ref.clone(),
            aomi_toml_paths,
            app_source_id,
        };
        // The Builder-authenticated preflight is also the source-claim seam:
        // when no local app_source_id exists, Build resolves the installed repo
        // and links it to the signed-in Builder before any deployment write.
        let preflight = if self.preflight || request.app_source_id.is_none() {
            Some(client.deploy(&request, true).await.map_err(|error| {
                self.explain_deploy_error(error, &platform, &source_ref, &build_url)
            })?)
        } else {
            None
        };
        if self.preflight {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    preflight
                        .as_ref()
                        .expect("preflight response exists when requested")
                )?
            );
            return Ok(());
        }
        if let Some(preflight) = preflight {
            request.app_source_id = Some(preflight.app_source_id);
        }
        let response = client.deploy(&request, false).await.map_err(|error| {
            self.explain_deploy_error(error, &platform, &source_ref, &build_url)
        })?;

        let project_url = response.project_url.clone();
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
            println!("  project       : {project_url}");
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
        source_ref: &str,
        build_url: &str,
    ) -> anyhow::Error {
        let msg = err.to_string();
        if !(msg.contains("403 Forbidden") || msg.contains("401 Unauthorized")) {
            return err;
        }
        anyhow!(
            "{msg}\n\n\
             Deploy authorization needs a verified Builder login that owns this GitHub source.\n\
             Platform: `{platform}`\n\
             Source: {}\n\
             Log in again with:\n\
               aomi-build login --build-url {build_url}\n\
             Headless automation may set {BUILD_TOKEN_ENV}.",
            source_ref_label(source_ref),
        )
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

    pub(crate) fn resolve_app_source_id(&self, git_root: &Path) -> Option<i64> {
        // Resolution order: flag → env → the id recorded by a prior deploy /
        // `source sync` in `.aomi/deployment.json`. The last step is what lets a
        // re-deploy run with no `--app-source-id` once the source is known.
        if let Some(id) = self.app_source_id.filter(|id| *id > 0) {
            return Some(id);
        }
        if let Some(id) = env_value(APP_SOURCE_ID_ENV)
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|id| *id > 0)
        {
            return Some(id);
        }
        if let Some(id) = self.recorded_app_source_id(git_root) {
            return Some(id);
        }
        None
    }

    pub(crate) fn recorded_app_source_id(&self, git_root: &Path) -> Option<i64> {
        LocalDeployment::read(git_root)
            .ok()
            .flatten()
            .and_then(|state| state.app_source_id())
            .filter(|id| *id > 0)
    }
}

fn source_ref_label(source_ref: &str) -> String {
    format!("commit `{source_ref}`")
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
