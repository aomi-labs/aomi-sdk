//! `deploy` — deploy tracked `aomi.toml` apps from a source ref through the
//! backend.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow, bail};
use clap::Args;

use super::shared::{
    ACTIVATION_TOKEN_ENV, APP_SOURCE_ID_ENV, BACKEND_URL_ENV, CredentialSource, bin_name,
    env_value, git_context, head_commit, resolve_activation_token_with_source, resolve_backend,
    tracked_aomi_tomls,
};
use crate::deploy::app::AomiAppFiles;
use crate::deploy::backend::BackendClient;
use crate::deploy::platform::Platform;
use crate::deploy::types::{DeployInput, LocalDeployment};

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

impl DeployArgs {
    pub async fn run(self) -> Result<()> {
        let (git_root, start_dir) = git_context(&self.path)?;
        let platform = self.platform(&git_root, &start_dir);
        let source_ref = self.source_ref(&git_root)?;
        let aomi_toml_paths = self.aomi_toml_paths(&git_root)?;
        let app_source_id = self.resolve_app_source_id(&git_root)?;

        let request = DeployInput {
            app_source_id,
            source_ref: source_ref.clone(),
            aomi_toml_paths,
            preflight: self.preflight,
        };

        if self.preflight {
            return self.run_preflight(&platform, &request).await;
        }

        let backend_url = self.backend_url()?;
        crate::sdk_guard::ensure_project_sdk(&git_root, Some(&backend_url), self.fix_sdk).await?;
        let (token, token_source) = activation_token_with_source()?;
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
               - the source ref pushed to GitHub ({})\n\
             {}\n\
             To refresh the source id, run:\n\
               aomi-build source sync --repo <owner/repo>\n\
             To refresh credentials, run:\n\
               aomi-build connect --platform {platform}",
            token_source.label(),
            source_ref_label(source_ref),
            token_source.stale_hint()
        )
    }

    /// Preflight: POST with `preflight: true` when a backend + token are
    /// available (the backend validates source commit scope and renders the
    /// deployment record); otherwise print the request we would send, fully
    /// offline.
    async fn run_preflight(&self, platform: &Platform, request: &DeployInput) -> Result<()> {
        match (self.backend_url().ok(), activation_token().ok()) {
            (Some(url), Some(token)) => {
                let response = BackendClient::new(url, token)?
                    .deploy(platform, request)
                    .await?;
                println!("{}", serde_json::to_string_pretty(&response)?);
            }
            _ => {
                println!("{}", serde_json::to_string_pretty(request)?);
                println!("\n(preflight: no backend/token; printed the request only)");
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

    pub(crate) fn resolve_app_source_id(&self, git_root: &Path) -> Result<i64> {
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
        Err(anyhow!(
            "deploy needs --app-source-id (or {APP_SOURCE_ID_ENV}): the connected GitHub \
             App install to deploy from. Install the Aomi GitHub App on your source repo, \
             then run `aomi-build source sync --repo <owner/repo>` (or use the app_source id \
             ops/the portal issued you)."
        ))
    }

    pub(crate) fn recorded_app_source_id(&self, git_root: &Path) -> Option<i64> {
        LocalDeployment::read(git_root)
            .ok()
            .flatten()
            .and_then(|state| state.app_source_id())
            .filter(|id| *id > 0)
    }
}

fn activation_token() -> Result<String> {
    activation_token_with_source().map(|(token, _)| token)
}

fn activation_token_with_source() -> Result<(String, CredentialSource)> {
    resolve_activation_token_with_source(&None).ok_or_else(|| {
        anyhow!(
            "deploy requires an activation token via {ACTIVATION_TOKEN_ENV} \
             (or run `aomi-build connect`)"
        )
    })
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
