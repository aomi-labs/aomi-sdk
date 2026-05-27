use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand};

use crate::activate::{ACTIVATION_TOKEN_ENV, ActivationPlan, BACKEND_URL_ENV, Visibility};
use crate::deployment_state::{read as read_deployment_state, write as write_deployment_state};
use crate::plan::Deployment;
use crate::platform::Platform;

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
            Command::Deploy(args) => {
                if args.dry_run {
                    let deployment =
                        Deployment::dry_run(&args.path, args.platform.clone(), args.allow_dirty)?;
                    let mut state = deployment.to_state();

                    if args.preflight {
                        let backend = args
                            .backend_url
                            .clone()
                            .or_else(|| std::env::var(BACKEND_URL_ENV).ok())
                            .ok_or_else(|| {
                                anyhow::anyhow!(
                                    "--preflight requires --backend-url or {BACKEND_URL_ENV}"
                                )
                            })?;
                        crate::preflight::run(&mut state, &backend).await?;
                    }

                    let state_path = write_deployment_state(&deployment.source.git_root, &state)?;
                    if args.json {
                        println!("{}", serde_json::to_string_pretty(&state)?);
                    } else {
                        println!("{}", deployment.render());
                        println!("  deployment_state    : {}", state_path.display());
                    }
                } else {
                    match (&args.stage_dir, &args.platform_repo_dir) {
                        (Some(_), Some(_)) => {
                            bail!("use either --stage-dir or --platform-repo-dir, not both");
                        }
                        (Some(stage_dir), None) => {
                            if args.no_push {
                                bail!("--no-push only applies with --platform-repo-dir");
                            }
                            let outcome = Deployment::stage(&args.path, args.platform.clone(), stage_dir)?;
                            if args.json {
                                println!("{}", serde_json::to_string_pretty(&outcome)?);
                            } else {
                                println!("{}", outcome.render());
                            }
                        }
                        (None, Some(platform_repo_dir)) => {
                            let outcome = Deployment::git_transport(
                                &args.path,
                                args.platform.clone(),
                                platform_repo_dir,
                                !args.no_push,
                            )?;

                            // Refresh .aomi/deployment.json with the post-push state.
                            // Start from any preflight-resolved state on disk; if none
                            // exists, derive a fresh state from the outcome.
                            let git_root = &outcome.deployment.source.git_root;
                            let mut state = match read_deployment_state(git_root) {
                                Ok(Some(prior)) => prior,
                                _ => outcome.deployment.to_state(),
                            };
                            state.state.pushed = outcome.pushed;
                            state.recompute_deployed();

                            // Auto-activate when an activation token is set and the
                            // push actually went through. Per ADR 0009 we *attempt*
                            // even when state.deployed = false; the backend is the
                            // authority on whether to reject.
                            let activation_token = std::env::var(ACTIVATION_TOKEN_ENV).ok();
                            let backend_url = args
                                .backend_url
                                .clone()
                                .or_else(|| std::env::var(BACKEND_URL_ENV).ok());
                            if outcome.pushed
                                && let (Some(token), Some(url)) = (activation_token, backend_url)
                            {
                                let visibility = match outcome.deployment.app.public {
                                    Some(true) => Visibility::Public,
                                    _ => Visibility::Private,
                                };
                                // Resolve the GitHub read token from
                                // aomi.toml's `[app].access_token` reference.
                                // Public-release platforms can omit this and
                                // we send no token (backend tries
                                // unauthenticated GitHub).
                                let github_token =
                                    outcome.deployment.app.resolved_access_token()?;
                                let plan = ActivationPlan::new(
                                    &outcome.deployment.publish.release_tag,
                                    args.platform.clone(),
                                    url,
                                    token,
                                    visibility,
                                    outcome.deployment.publish.source_repo.clone(),
                                    github_token,
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
                                        state
                                            .errors
                                            .push(format!("auto-activation failed: {e}"));
                                    }
                                }
                            }

                            state.touch();
                            let state_path = write_deployment_state(git_root, &state)?;

                            if args.json {
                                println!("{}", serde_json::to_string_pretty(&outcome)?);
                            } else {
                                println!("{}", outcome.render());
                                println!("  deployment_state    : {}", state_path.display());
                                println!(
                                    "  state               : pushed={} deployed={} activated={}",
                                    state.state.pushed,
                                    state.state.deployed,
                                    state.state.activated
                                );
                            }
                        }
                        (None, None) => {
                            bail!(
                                "deploy requires --dry-run, --stage-dir <DIR>, or --platform-repo-dir <DIR>"
                            );
                        }
                    }
                }
                Ok(())
            }
            Command::Activate(args) => {
                let plan = args.plan()?;
                let response = plan.execute().await?;
                if args.json {
                    println!("{}", serde_json::to_string_pretty(&response)?);
                } else {
                    println!("activated {}", plan.request.name);
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Prepare an Aomi app source publication.
    Deploy(DeployArgs),
    /// Activate a published Aomi app release in the backend.
    Activate(ActivateArgs),
}

#[derive(Debug, Args, Clone)]
pub struct DeployArgs {
    /// Platform app source to target. Any string accepted — must match a row
    /// in the backend `platforms` table. Defaults to `community`.
    #[arg(long, default_value_t = Platform::community())]
    pub platform: Platform,

    /// Print the publish plan without staging, pushing, or calling the backend.
    #[arg(long)]
    pub dry_run: bool,

    /// Allow a dirty working tree in the printed plan.
    #[arg(long)]
    pub allow_dirty: bool,

    /// App directory to inspect. Defaults to the current directory.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Local platform app repo checkout root to stage into.
    #[arg(long, value_name = "DIR")]
    pub stage_dir: Option<PathBuf>,

    /// Local platform app repo checkout root to commit and push from.
    #[arg(long, value_name = "DIR")]
    pub platform_repo_dir: Option<PathBuf>,

    /// Commit into --platform-repo-dir but do not push.
    #[arg(long)]
    pub no_push: bool,

    /// Print the publish plan as JSON.
    #[arg(long)]
    pub json: bool,

    /// Run online preflight against the backend during dry-run. Fills in
    /// `platform.resolved_deploy_branch` and `state.deployed` based on
    /// `GET /api/control/platforms`.
    #[arg(long)]
    pub preflight: bool,

    /// Backend base URL for preflight. Defaults to `AOMI_BACKEND_URL`.
    #[arg(long, value_name = "URL")]
    pub backend_url: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct ActivateArgs {
    /// Release tag to activate, e.g. apps-zora-abc1234.
    pub release_tag: String,

    /// Platform app source that produced the release. Defaults to `community`.
    #[arg(long, default_value_t = Platform::community())]
    pub platform: Platform,

    /// Backend base URL. Defaults to AOMI_BACKEND_URL.
    #[arg(long, value_name = "URL")]
    pub backend_url: Option<String>,

    /// App activation token. Defaults to AOMI_APP_ACTIVATION_TOKEN.
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,

    /// Activation visibility.
    #[arg(long, value_enum, default_value_t = Visibility::Private)]
    pub visibility: Visibility,

    /// Optional display label for the app registry row.
    #[arg(long)]
    pub label: Option<String>,

    /// Source repo to record on the application row, e.g.
    /// `aomi-labs/community-apps`. Required — the deployment.json artifact
    /// next to your aomi.toml has it under `publish.source_repo` if you've
    /// already run `aomi-git deploy`.
    #[arg(long, value_name = "REPO")]
    pub source_repo: String,

    /// Env-var **name** holding the GitHub read token the backend uses to
    /// download this release. Required for private platform repos; omit for
    /// public ones. e.g. `--access-token-env KREXA_GITHUB_TOKEN`. The token
    /// itself never appears on the CLI — only its env-var name.
    #[arg(long, value_name = "ENV_NAME")]
    pub access_token_env: Option<String>,

    /// Optional full source commit for provenance.
    #[arg(long)]
    pub source_commit: Option<String>,

    /// Optional source tree hash for provenance.
    #[arg(long)]
    pub source_tree: Option<String>,

    /// Optional source digest for provenance.
    #[arg(long)]
    pub source_digest: Option<String>,

    /// Print the backend response as JSON.
    #[arg(long)]
    pub json: bool,
}

impl ActivateArgs {
    pub fn plan(&self) -> Result<ActivationPlan> {
        let github_token = match self.access_token_env.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(env_name) => match std::env::var(env_name) {
                Ok(value) if !value.is_empty() => Some(value),
                Ok(_) => anyhow::bail!("env var `{env_name}` (from --access-token-env) is empty"),
                Err(_) => anyhow::bail!("env var `{env_name}` (from --access-token-env) is not set"),
            },
            None => None,
        };
        ActivationPlan::new(
            &self.release_tag,
            self.platform.clone(),
            self.backend_url
                .clone()
                .or_else(|| std::env::var(BACKEND_URL_ENV).ok())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "backend URL is required via --backend-url or {BACKEND_URL_ENV}"
                    )
                })?,
            self.activation_token
                .clone()
                .or_else(|| std::env::var(ACTIVATION_TOKEN_ENV).ok())
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "activation token is required via --activation-token or {ACTIVATION_TOKEN_ENV}"
                    )
                })?,
            self.visibility,
            self.source_repo.clone(),
            github_token,
            self.label.clone(),
            self.source_commit.clone(),
            self.source_tree.clone(),
            self.source_digest.clone(),
        )
    }
}
