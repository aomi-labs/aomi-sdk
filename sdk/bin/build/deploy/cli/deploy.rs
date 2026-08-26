//! `deploy` — full hosted app deploy lifecycle plus explicit deploy steps.

use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::{Args, Subcommand};

use super::release;
use super::shared::{
    ACTIVATION_TOKEN_ENV, BUILD_TOKEN_ENV, bin_name, commit_on_remote, env_value, git_context,
    head_branch, resolve_backend, worktree_dirty,
};
use super::{ActivateArgs, StatusArgs};
use crate::deploy::platform::Platform;
use crate::deploy::session::Session;
use crate::deploy::state::LocalDeployment;
use crate::deploy::types::BuildDeployInput;

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
        release::wait_via_backend(
            &backend_url,
            &token,
            &platform,
            &state,
            "Release build",
            timeout_message,
        )
        .await?;
    } else {
        let session = Session::open(&args.backend, &args.build_url).await?;
        release::wait_via_build(
            &session.client,
            &platform,
            &state,
            "Release build",
            timeout_message,
        )
        .await?;
    }
    args.run().await
}

#[derive(Debug, Args, Clone, Default)]
pub struct DeployStepArgs {
    /// Source repository used to resolve its existing Project.
    #[arg(long, value_name = "OWNER/REPO")]
    pub repo: Option<String>,

    /// Deploy this exact source commit. Defaults to local HEAD.
    #[arg(long, value_name = "SHA")]
    pub commit: Option<String>,

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

/// A deploy step with the local source and authenticated request resolved.
/// The preflight response supplies the persisted Project and platform.
///
/// Both entry points below resolved this same set of five values in the same
/// order before they could do anything, so it is built once and passed whole.
struct Prepared {
    git_root: PathBuf,
    session: Session,
    request: BuildDeployInput,
}

impl Prepared {
    /// Send the deploy request, annotating an auth rejection with what it took.
    async fn deploy(&self, preflight: bool) -> Result<crate::deploy::types::BuildDeployResult> {
        self.session
            .client
            .deploy(&self.request, preflight)
            .await
            .map_err(|error| self.explain(error))
    }

    fn explain(&self, err: anyhow::Error) -> anyhow::Error {
        let message = err.to_string();
        if !(message.contains("403 Forbidden") || message.contains("401 Unauthorized")) {
            return err;
        }
        anyhow!(
            "{message}\n\n\
             Deploy authorization needs a verified Builder login that owns this GitHub source.\n\
             Source: commit `{}`\n\
             Log in again with:\n\
               aomi-build login --build-url {}\n\
             Headless automation may set {BUILD_TOKEN_ENV}.",
            self.request.source_ref,
            self.session.build_url()
        )
    }
}

impl DeployStepArgs {
    /// Resolve the git root, SDK pin, session, and request body, then
    /// show what is about to be deployed and block the deploys that cannot
    /// work: a source commit the backend can't fetch, or an SDK pin that isn't
    /// in the commit being shipped.
    async fn prepare(&self, announce: bool) -> Result<Prepared> {
        let (git_root, _) = git_context(&self.path)?;
        let backend_url = resolve_backend(&self.backend);
        let sdk =
            crate::sdk_guard::ensure_project_sdk(&git_root, backend_url.as_deref(), self.fix_sdk)
                .await?;
        let session = Session::open(&self.backend, &self.build_url).await?;
        let request = self.build_request(&git_root)?;
        let configured_platform = crate::deploy::project_config::ProjectConfig::load(&git_root)?
            .platform()
            .clone();

        let branch = head_branch(&git_root);
        let dirty = worktree_dirty(&git_root);
        let pushed = commit_on_remote(&git_root, &request.source_ref);

        if announce {
            let short = &request.source_ref[..request.source_ref.len().min(7)];
            println!(
                "  Project    {} (platform `{configured_platform}`)",
                request.repo
            );
            let mut source = format!("{} @ {short}", request.repo);
            source.push_str(&format!(" · {}", branch.as_deref().unwrap_or("detached")));
            match pushed {
                Some(true) => source.push_str(" · pushed ✓"),
                Some(false) => source.push_str(" · pushed ✗"),
                None => {}
            }
            match dirty {
                Some(false) => source.push_str(" · clean ✓"),
                Some(true) => source.push_str(" · uncommitted changes !"),
                None => {}
            }
            println!("  Source     {source}");
            let sdk_line = if sdk.from_backend {
                format!("aomi-sdk ={} · matches backend ✓", sdk.required)
            } else {
                format!(
                    "aomi-sdk ={} · matches this CLI (no backend URL to ask) ✓",
                    sdk.required
                )
            };
            println!("  SDK        {sdk_line}");
        }

        if dirty == Some(true) {
            eprintln!(
                "  ! the working tree has uncommitted changes — the deploy ships commit \
                 {}, not your local edits.",
                &request.source_ref[..request.source_ref.len().min(7)]
            );
        }
        crate::sdk_guard::ensure_committed_pin(&git_root, &request.source_ref, &sdk)?;
        if pushed == Some(false) {
            bail!(
                "commit {} is not on any remote — the backend syncs the source from GitHub \
                 and cannot see unpushed commits. Push it, then re-deploy:\n  git push",
                &request.source_ref[..request.source_ref.len().min(7)]
            );
        }

        Ok(Prepared {
            git_root,
            session,
            request,
        })
    }

    pub async fn run_full_lifecycle(self) -> Result<()> {
        let mut prepared = self.prepare(!self.json).await?;
        let quiet = self.json;

        let preflight = prepared.deploy(true).await?;
        let platform = Platform::new(&preflight.deployment.platform.platform);
        if !quiet {
            println!();
            println!(
                "[1/4] Preflight       ✓ {} app(s) on `{}`",
                preflight.deployment.platform.apps.len(),
                platform
            );
            for app in &preflight.deployment.platform.apps {
                println!("        {} → {}", app.name, app.release_tag);
            }
        }

        prepared.request.project_id = Some(preflight.project_id);
        let deploy = prepared.deploy(false).await?;
        let mut state = LocalDeployment::from_build_deploy(deploy);
        let path = state.write(&prepared.git_root)?;
        if !quiet {
            println!(
                "[2/4] Deploy          ✓ PR {}",
                state
                    .deployment
                    .platform
                    .pr_url
                    .as_deref()
                    .unwrap_or("(pending CI)")
            );
            println!(
                "        id {} · recorded in {}",
                state.deployment.id,
                path.display()
            );
        }

        release::wait_via_build(
            &prepared.session.client,
            &platform,
            &state,
            "[3/4] Release build",
            format!(
                "deployment did not become ready within 30 minutes; resume with `{} deploy activate --path {}`",
                bin_name(),
                self.path.display()
            ),
        )
        .await?;

        if !quiet {
            println!("[4/4] Activate        …");
        }
        let activate_args = ActivateArgs {
            platform: Some(platform.clone()),
            backend: prepared.session.backend_url().map(str::to_string),
            build_url: Some(prepared.session.build_url().to_string()),
            activation_token: self.activation_token.clone(),
            target_tags: self.target_tags.clone(),
            path: self.path.clone(),
            json: self.json,
            fix_sdk: self.fix_sdk,
            ..Default::default()
        };
        let response = activate_args
            .activate_with_state(&prepared.git_root, &mut state)
            .await?;
        state.write(&prepared.git_root)?;
        ActivateArgs::print_activation(&response, self.json)?;
        if !quiet {
            let apps = state.app_names().join(", ");
            let live = if state.app_names().len() == 1 {
                "is live"
            } else {
                "are live"
            };
            println!();
            println!(
                "✓ {apps} {live} on {} ({})",
                platform,
                prepared.session.env_label()
            );
            for app in &state.deployment.platform.apps {
                println!("    Release    {}", app.release_tag);
            }
            if let Some(project_url) = &state.project_url {
                println!("    Project    {project_url}");
            }
            println!(
                "    Status     {} status --path {}",
                bin_name(),
                self.path.display()
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
        // Preflight's stdout is the JSON plan; keep the human card off it.
        let mut prepared = self.prepare(!self.json && !preflight).await?;
        // A first deploy has no project id yet; preflight resolves it.
        let response = if preflight || prepared.request.project_id.is_none() {
            let resolved = prepared.deploy(true).await?;
            if preflight {
                println!("{}", serde_json::to_string_pretty(&resolved)?);
                return Ok(());
            }
            prepared.request.project_id = Some(resolved.project_id);
            prepared.deploy(false).await?
        } else {
            prepared.deploy(false).await?
        };

        let git_root = prepared.git_root;
        let platform = Platform::new(&response.deployment.platform.platform);
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
}
