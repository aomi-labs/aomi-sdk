//! Guided interactive flow for `aomi-build` with no subcommand — a Claude-Code
//! style first-run wizard that walks a user Connect → Source → Deploy →
//! Activate. It's built on `inquire` prompts over the same command structs the
//! CLI exposes (`DeployArgs`, `ActivateArgs`, …) and the shared `flow` core, so
//! nothing the wizard does is unavailable to scripting.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use inquire::{Confirm, Select, Text};

use crate::new_app::NewAppArgs;
use crate::specs::workspace_root;

use super::cli::{ActivateArgs, DeployStepArgs};
use super::config::AomiConfig;
use super::flow;
use super::platform::{Platform, normalize_github_repo};
use super::types::LocalDeployment;

const STAGING_URL: &str = "https://api-staging.aomi.dev";
const PROD_URL: &str = "https://api.aomi.dev";

pub async fn run() -> Result<()> {
    println!("aomi-build — let's get your app deployed.\n");
    let mut config = AomiConfig::load();

    // Main loop: a failed step prints its error and returns here instead of
    // tearing down the wizard. Only an explicit Quit (or Ctrl-C at a prompt)
    // exits.
    loop {
        let choice = Select::new(
            "What do you want to do?",
            vec![
                "Deploy the app in a local directory",
                "Create an OpenAPI-driven app locally",
                "Scaffold a new app from the example template",
                "Quit",
            ],
        )
        .prompt()
        .context("wizard cancelled")?;

        let outcome = match choice {
            c if c.starts_with("Deploy") => {
                let (backend_url, platform, token) = deployment_context(&mut config).await?;
                existing_dir_flow(&backend_url, &platform, &token).await
            }
            c if c.starts_with("Create") => openapi_app_flow().await,
            c if c.starts_with("Scaffold") => {
                let (backend_url, platform, token) = deployment_context(&mut config).await?;
                scaffold_flow(&backend_url, &platform, &token).await
            }
            _ => return Ok(()),
        };

        if let Err(e) = outcome {
            print_step_error(&e);
            // fall through — stay in the loop
        } else if !Confirm::new("Do something else?")
            .with_default(false)
            .prompt()
            .unwrap_or(false)
        {
            return Ok(());
        }
    }
}

async fn deployment_context(config: &mut AomiConfig) -> Result<(String, String, String)> {
    let backend_url = pick_backend(config)?;
    config.backend_url = Some(backend_url.clone());

    // The deploy *destination* platform (a `DbPlatform`, e.g. `community`) —
    // not the source/template repo it's copied from.
    let platform = Text::new("Deploy to which platform?")
        .with_default(config.platform.as_deref().unwrap_or("community"))
        .prompt()
        .context("wizard cancelled")?
        .trim()
        .to_string();
    config.platform = Some(platform.clone());

    let token = ensure_token(config, &backend_url, &platform).await?;
    // Persist what we have so a re-run resumes with backend/token/platform set.
    // This must succeed: deploy/activate re-read the token from this file (the
    // `*Args` structs take no token), so a silent failure here would resurface
    // later as a confusing "no activation token" error.
    config
        .save()
        .context("couldn't save your Aomi config — deploy needs the token on disk")?;

    Ok((backend_url, platform, token))
}

/// Print a step error exactly (the full anyhow chain) without leaving the
/// wizard, so a transient backend failure doesn't kill the session.
fn print_step_error(e: &anyhow::Error) {
    eprintln!("\n  ✗ that step failed:");
    eprintln!("      {e:#}");
    eprintln!("  (you're still in the wizard — pick another option or try again)\n");
}

fn pick_backend(config: &AomiConfig) -> Result<String> {
    if let Some(url) = config.backend_url.as_deref().filter(|u| !u.is_empty()) {
        let reuse = Confirm::new(&format!("Use saved backend {url}?"))
            .with_default(true)
            .prompt()
            .context("wizard cancelled")?;
        if reuse {
            return Ok(url.to_string());
        }
    }
    let choice = Select::new("Which backend?", vec!["staging", "production", "custom"])
        .prompt()
        .context("wizard cancelled")?;
    Ok(match choice {
        "staging" => STAGING_URL.to_string(),
        "production" => PROD_URL.to_string(),
        _ => Text::new("Backend URL:")
            .prompt()
            .context("wizard cancelled")?
            .trim()
            .to_string(),
    })
}

/// Reuse a saved token if it still validates; otherwise prompt for one and
/// (optionally) point the user at `connect` to install the GitHub App.
async fn ensure_token(
    config: &mut AomiConfig,
    backend_url: &str,
    platform: &str,
) -> Result<String> {
    if let Some(saved) = config.activation_token.clone() {
        match flow::validate_activation_token(backend_url, &saved, platform).await {
            flow::TokenCheck::Valid => {
                println!("Using your saved activation token (verified).\n");
                return Ok(saved);
            }
            flow::TokenCheck::Unreachable => {
                // Don't force re-entry over a network blip — the token was fine
                // last time and we couldn't reach the backend to say otherwise.
                println!("Couldn't reach the backend to verify your saved token — using it.\n");
                return Ok(saved);
            }
            flow::TokenCheck::Invalid => {
                println!("Saved token was rejected for `{platform}` — let's set a new one.");
            }
        }
    } else {
        println!(
            "You'll need an activation token from your Aomi admin. If you haven't installed\n\
             the GitHub App yet, run `aomi-build connect` first (or paste the token below).\n"
        );
    }
    loop {
        let token = inquire::Password::new("Activation token (from your Aomi admin):")
            .without_confirmation()
            .prompt()
            .context("wizard cancelled")?
            .trim()
            .to_string();
        if token.is_empty() {
            println!("  a token is required to deploy.");
            continue;
        }
        match flow::validate_activation_token(backend_url, &token, platform).await {
            flow::TokenCheck::Valid => println!("  token verified.\n"),
            flow::TokenCheck::Unreachable => {
                println!("  couldn't reach the backend to verify — using it anyway.\n")
            }
            flow::TokenCheck::Invalid => {
                let keep = Confirm::new("That token was rejected — use it anyway?")
                    .with_default(false)
                    .prompt()
                    .context("wizard cancelled")?;
                if !keep {
                    continue;
                }
            }
        }
        config.activation_token = Some(token.clone());
        return Ok(token);
    }
}

async fn existing_dir_flow(backend_url: &str, platform: &str, token: &str) -> Result<()> {
    let dir = Text::new("Path to the app's source repo:")
        .with_default(".")
        .prompt()
        .context("wizard cancelled")?;
    let dir = PathBuf::from(dir.trim());

    let repo = match git_origin_slug(&dir) {
        Some(slug) => Text::new("Source repo (owner/name):")
            .with_default(&slug)
            .prompt()
            .context("wizard cancelled")?,
        None => Text::new("Source repo (owner/name):")
            .prompt()
            .context("wizard cancelled")?,
    };
    let repo = normalize_github_repo(repo.trim())?;

    resolve_and_deploy(backend_url, platform, token, &dir, &repo).await
}

/// Create an app inside the current Aomi workspace using the OpenAPI-driven
/// codegen pipeline, then optionally hand off the generated stubs to Codex or
/// Claude for skill-guided curation.
async fn openapi_app_flow() -> Result<()> {
    let platform = Text::new("New app/platform name:")
        .prompt()
        .context("wizard cancelled")?
        .trim()
        .to_string();
    if platform.is_empty() {
        bail!("app/platform name is required");
    }

    let source = Select::new(
        "OpenAPI source:",
        vec![
            "Discover automatically",
            "Fetch from a known OpenAPI URL",
            "Use existing apps/<name>/openapi.yaml",
        ],
    )
    .prompt()
    .context("wizard cancelled")?;

    let from_url = if source.starts_with("Fetch") {
        Some(
            Text::new("OpenAPI URL:")
                .prompt()
                .context("wizard cancelled")?
                .trim()
                .to_string(),
        )
    } else {
        None
    };

    let shared = Confirm::new("Generate as a shared provider under ext/?")
        .with_default(false)
        .prompt()
        .context("wizard cancelled")?;
    let all = Confirm::new("Expose every OpenAPI operation as a stub tool?")
        .with_default(false)
        .prompt()
        .context("wizard cancelled")?;
    let force = Confirm::new("Overwrite existing generated files if present?")
        .with_default(false)
        .prompt()
        .context("wizard cancelled")?;
    let build = Confirm::new("Run cargo build after generation?")
        .with_default(true)
        .prompt()
        .context("wizard cancelled")?;

    if source.starts_with("Use existing") {
        let platform_for_codegen = platform.clone();
        tokio::task::spawn_blocking(move || {
            run_existing_spec_app_flow(&platform_for_codegen, shared, all, force, build)
        })
        .await
        .context("OpenAPI generation task failed")??;
    } else {
        let platform_for_codegen = platform.clone();
        let result = tokio::task::spawn_blocking(move || {
            crate::new_app::run(NewAppArgs {
                platform: platform_for_codegen,
                from_url,
                all,
                force,
                no_tool: false,
                no_build: !build,
                shared,
            })
            .map_err(|e| anyhow!("{e:#}"))
        })
        .await
        .context("OpenAPI generation task failed")?;

        if let Err(e) = result {
            print_step_error(&anyhow!("{e:#}"));
            print_workbench_guidance(&platform, "draft or repair the OpenAPI spec");
            return Err(anyhow!("{e:#}"));
        }
    }

    print_workbench_guidance(&platform, "curate the generated app tools");
    println!();
    println!(
        "Next: commit and push the generated app, then choose \"Deploy the app in a local directory\" from this wizard."
    );
    Ok(())
}

fn run_existing_spec_app_flow(
    platform: &str,
    shared: bool,
    all: bool,
    force: bool,
    build: bool,
) -> Result<()> {
    println!("=== [1/2] gen-client {platform} ===");
    crate::client::run(crate::client::GenClientArgs {
        platform: platform.to_string(),
        spec: None,
        out: None,
        force,
        shared,
    })
    .map_err(|e| anyhow!("{e:#}"))
    .with_context(|| "gen-client failed")?;

    println!();
    println!("=== [2/2] gen-tool {platform} ===");
    crate::tool::run(crate::tool::GenToolArgs {
        platform: platform.to_string(),
        spec: None,
        out: None,
        all,
        force,
        shared,
    })
    .map_err(|e| anyhow!("{e:#}"))
    .with_context(|| "gen-tool failed")?;

    if build {
        println!();
        println!("=== [verify] cargo build -p {platform} ===");
        let root = workspace_root().map_err(|e| anyhow!("{e:#}"))?;
        let status = Command::new("cargo")
            .args(["build", "-p", platform])
            .current_dir(&root)
            .status()
            .with_context(|| "failed to spawn cargo")?;
        if !status.success() {
            bail!("cargo build -p {platform} failed");
        }
    }

    println!();
    println!("✓ OpenAPI app generation complete");
    Ok(())
}

fn print_workbench_guidance(platform: &str, task: &str) {
    println!();
    println!("For agent-assisted app creation, run the Smithers workbench:");
    println!("  aomi-workbench --sdk-root . --app {platform}");
    println!(
        "Use it to {task} with Codex or Claude while preserving this CLI's deterministic build path."
    );
}

/// Resolve the connected source for `repo` at `dir` — installing the aomi-build
/// App and retrying if it isn't connected yet — then deploy + activate. Shared
/// by the deploy-local and scaffold flows.
async fn resolve_and_deploy(
    backend_url: &str,
    platform: &str,
    token: &str,
    dir: &Path,
    repo: &str,
) -> Result<()> {
    println!("Resolving the connected source…");
    let app_source_id = match flow::sync_source(backend_url, token, platform, repo).await {
        Ok(id) => id,
        Err(_) => {
            // The repo isn't connected yet — the aomi-build App isn't installed
            // on it. Offer to install it inline, then retry the sync once.
            println!(
                "That repo isn't connected yet — the aomi-build GitHub App isn't installed on it."
            );
            let install = Confirm::new("Install the aomi-build GitHub App now?")
                .with_default(true)
                .prompt()
                .context("wizard cancelled")?;
            if !install {
                bail!(
                    "source sync needs the aomi-build GitHub App installed on {repo} — \
                     run `aomi-build connect --repo {repo}`, then try again."
                );
            }
            ensure_app_installed(backend_url, platform, Some(repo)).await?;
            flow::sync_source(backend_url, token, platform, repo)
                .await
                .context(
                    "still couldn't resolve the source after install — \
                 give GitHub a moment to propagate, then try again",
                )?
        }
    };
    println!("  app_source_id: {app_source_id}\n");
    deploy_then_activate(platform, backend_url, token, dir, Some(app_source_id)).await
}

/// Install the aomi-build GitHub App on `repo` from inside the wizard: open the
/// install URL and wait for the user to finish in their browser. We don't need
/// the resulting installation id — deploy resolves the source by repo via
/// sync-installed — so this is just "open and wait".
async fn ensure_app_installed(backend_url: &str, platform: &str, repo: Option<&str>) -> Result<()> {
    let start = flow::oauth_install_url(backend_url, platform, repo, "install").await?;
    println!("Install the aomi-build GitHub App:");
    println!("  {}", start.install_url);
    if let Err(e) = open::that(&start.install_url) {
        eprintln!("  (couldn't open a browser automatically: {e})");
        println!("  open the URL above to install.");
    } else {
        println!("  (opened in your browser)");
    }
    Confirm::new("Press enter once the install finishes")
        .with_default(true)
        .prompt()
        .context("wizard cancelled")?;
    Ok(())
}

/// Scaffold-from-example (bootstrap flow): open GitHub's "Use this template"
/// page so the user creates their own repo from the example, clone it, then run
/// the same resolve-and-deploy path as a local repo. Uses only the aomi-build
/// App — the repo is created by GitHub's template UI, not a backend call.
async fn scaffold_flow(backend_url: &str, platform: &str, token: &str) -> Result<()> {
    const GENERATE_URL: &str = "https://github.com/aomi-labs/playground-example/generate";
    println!("Create your repo from the example template:");
    println!("  {GENERATE_URL}");
    if let Err(e) = open::that(GENERATE_URL) {
        eprintln!("  (couldn't open a browser automatically: {e})");
        println!("  open the URL above.");
    } else {
        println!("  (opened in your browser)");
    }
    println!("  Name it whatever you like, create it in your account, then come back.\n");

    // Ask for the repo link and clone exactly that — preserving case (GitHub
    // owners are case-sensitive in the URL) and not reconstructing it. A pasted
    // URL is used verbatim; a bare `owner/name` becomes an https URL.
    let (clone_url, slug) = loop {
        let entered = Text::new("The repo you just created (link):")
            .prompt()
            .context("wizard cancelled")?;
        let entered = entered.trim();
        match normalize_github_repo(entered) {
            Ok(slug) => {
                let clone_url = if entered.contains("://") || entered.starts_with("git@") {
                    entered.trim_end_matches('/').to_string()
                } else {
                    format!("https://github.com/{}", entered.trim_end_matches('/'))
                };
                break (clone_url, slug);
            }
            Err(_) => {
                println!("  paste the repo link, e.g. https://github.com/you/your-repo")
            }
        }
    };

    // The CLI deploys from a local checkout (it derives the source ref from
    // git), so clone the new repo, then run the same path as a local repo.
    // Clone to an absolute path so the target never depends on a relative cwd.
    let name = slug.rsplit('/').next().unwrap_or("app").to_string();
    let cwd = std::env::current_dir().map_err(|_| {
        anyhow!(
            "this shell's working directory no longer exists (it was deleted or \
             recreated, so git can't run here). Open a fresh terminal — or `cd` into a \
             directory that exists — then re-run `aomi-build`."
        )
    })?;
    let target = cwd.join(&name);
    if target.exists() {
        bail!(
            "`{}` already exists — remove it or clone elsewhere, then pick \
             \"Deploy the app in a local directory\"",
            target.display()
        );
    }
    println!("Cloning {clone_url} → {} …", target.display());
    clone_repo(&clone_url, &target)?;

    resolve_and_deploy(backend_url, platform, token, &target, &slug).await
}

/// `git clone <repo_link> <dir>`. The example repo is public, so no auth.
fn clone_repo(repo_link: &str, dir: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["clone", repo_link])
        .arg(dir)
        .status()
        .context("failed to run git clone")?;
    if !status.success() {
        bail!(
            "git clone {repo_link} failed — if the repo is private make it public \
             (or clone it yourself); if it was just created, wait a moment and retry."
        );
    }
    Ok(())
}

async fn deploy_then_activate(
    platform: &str,
    backend_url: &str,
    token: &str,
    dir: &Path,
    app_source_id: Option<i64>,
) -> Result<()> {
    let go = Confirm::new(&format!("Deploy `{platform}` from {}?", dir.display()))
        .with_default(true)
        .prompt()
        .context("wizard cancelled")?;
    if !go {
        println!("Stopped before deploy. Re-run `aomi-build` anytime.");
        return Ok(());
    }

    // Deploy with inline retry — a transient backend failure prints and offers
    // a retry instead of unwinding the whole wizard.
    loop {
        let result = DeployStepArgs {
            platform: Some(Platform::new(platform)),
            app_source_id,
            backend: Some(backend_url.to_string()),
            activation_token: Some(token.to_string()),
            path: dir.to_path_buf(),
            fix_sdk: true,
            ..Default::default()
        }
        .run_deploy_command()
        .await;
        match result {
            Ok(()) => break,
            Err(e) => {
                print_step_error(&e);
                if !retry("Retry the deploy?")? {
                    return Ok(());
                }
            }
        }
    }

    let wait = Confirm::new("Wait for CI to build the release, then activate?")
        .with_default(true)
        .prompt()
        .context("wizard cancelled")?;
    if !wait {
        // Omit --target-tag: the backend defaults to the deployment's own
        // server_tags, matching the portal (which sends no target_tags).
        println!(
            "When CI is green:\n  aomi-build activate --path {}",
            dir.display()
        );
        return Ok(());
    }

    // Gate activation on the release build, like the portal: poll the
    // deployment's status until it's `ready` before promoting. The deploy step
    // recorded the id in `.aomi/deployment.json`.
    if let Some(id) = deployment_id(dir) {
        println!("Waiting for the release build (up to 30 min, Ctrl-C to stop)…");
        match flow::poll_deployment_ready(
            backend_url,
            token,
            platform,
            &id,
            Duration::from_secs(30 * 60),
            |state| println!("  build: {state}"),
        )
        .await?
        {
            flow::DeployReady::Ready => println!("  release is ready — activating.\n"),
            flow::DeployReady::Failed(msg) => {
                bail!("the release build failed: {msg}");
            }
            flow::DeployReady::TimedOut => {
                println!(
                    "Still building after 30 min. Activate once CI is green:\n  \
                     aomi-build activate --path {}",
                    dir.display()
                );
                return Ok(());
            }
        }
    }

    loop {
        let result = ActivateArgs {
            platform: Some(Platform::new(platform)),
            backend: Some(backend_url.to_string()),
            activation_token: Some(token.to_string()),
            path: dir.to_path_buf(),
            fix_sdk: true,
            ..Default::default()
        }
        .run()
        .await;
        match result {
            Ok(()) => return Ok(()),
            Err(e) => {
                print_step_error(&e);
                if !retry("Retry activation?")? {
                    return Ok(());
                }
            }
        }
    }
}

/// The deployment id the last deploy recorded in `.aomi/deployment.json`, if
/// readable. Used to poll the release build before activating.
fn deployment_id(dir: &Path) -> Option<String> {
    LocalDeployment::read(dir)
        .ok()
        .flatten()
        .map(|d| d.deployment.id)
}

/// Small yes/no retry prompt; a cancel (Ctrl-C) propagates out to exit.
fn retry(question: &str) -> Result<bool> {
    Confirm::new(question)
        .with_default(true)
        .prompt()
        .context("wizard cancelled")
}

/// Best-effort `owner/name` from the repo's `origin` remote.
fn git_origin_slug(dir: &PathBuf) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8(out.stdout).ok()?;
    normalize_github_repo(url.trim()).ok()
}
