//! Guided interactive flow for `aomi-build` with no subcommand — a Claude-Code
//! style first-run wizard that walks a user Login → Source → Deploy →
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

use super::cli::shared::resolve_build_url;
use super::cli::{ActivateArgs, DeployStepArgs, login};
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
                let (backend_url, build_url, platform) = deployment_context(&mut config).await?;
                existing_dir_flow(&backend_url, &build_url, &platform).await
            }
            c if c.starts_with("Create") => openapi_app_flow().await,
            c if c.starts_with("Scaffold") => {
                let (backend_url, build_url, platform) = deployment_context(&mut config).await?;
                scaffold_flow(&backend_url, &build_url, &platform).await
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
    let build_url = resolve_build_url(&None, Some(&backend_url)).ok_or_else(|| {
        anyhow!(
            "custom backend `{backend_url}` needs AOMI_BUILD_URL or a saved `aomi-build login --build-url ...`"
        )
    })?;
    // May run a browser login, which persists the CLI bearer to the config file.
    login::ensure_logged_in(&build_url).await?;

    // The deploy *destination* platform (a `DbPlatform`, e.g. `community`) —
    // not the source/template repo it's copied from.
    let platform = Text::new("Deploy to which platform?")
        .with_default(config.platform.as_deref().unwrap_or("community"))
        .prompt()
        .context("wizard cancelled")?
        .trim()
        .to_string();

    // Merge onto whatever is on disk *now* rather than saving the snapshot this
    // function started from: `ensure_logged_in` may have written a fresh bearer
    // in between, and a wholesale save of the stale snapshot would erase it.
    AomiConfig::update(|saved| {
        saved.backend_url = Some(backend_url.clone());
        saved.build_url = Some(build_url.clone());
        saved.platform = Some(platform.clone());
    })
    .context("couldn't save your Aomi Builder config")?;

    // Keep the caller's in-memory copy in step for later loop iterations.
    config.backend_url = Some(backend_url.clone());
    config.build_url = Some(build_url.clone());
    config.platform = Some(platform.clone());

    Ok((backend_url, build_url, platform))
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

async fn existing_dir_flow(backend_url: &str, build_url: &str, platform: &str) -> Result<()> {
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

    deploy_then_activate(platform, backend_url, build_url, &dir, &repo).await
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

/// Scaffold-from-example (bootstrap flow): open GitHub's "Use this template"
/// page so the user creates their own repo from the example, clone it, then run
/// the same resolve-and-deploy path as a local repo. Uses only the aomi-build
/// App — the repo is created by GitHub's template UI, not a backend call.
async fn scaffold_flow(backend_url: &str, build_url: &str, platform: &str) -> Result<()> {
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

    deploy_then_activate(platform, backend_url, build_url, &target, &slug).await
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
    build_url: &str,
    dir: &Path,
    repo: &str,
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
            repo: Some(repo.to_string()),
            backend: Some(backend_url.to_string()),
            build_url: Some(build_url.to_string()),
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
    // recorded the id in `.aomi/deployment.json`. A read failure here used to be
    // swallowed, so the wizard skipped the wait it had just promised and went
    // straight to an activation that could not succeed either — the same file is
    // what `activate` reads its release tags from.
    let deployment = local_deployment(dir)?;
    let client = login::ensure_logged_in(build_url).await?.client;
    println!("Waiting for the release build (up to 30 min, Ctrl-C to stop)…");
    match flow::poll_build_deployment_ready(
        &client,
        platform,
        &deployment.deployment.id,
        Duration::from_secs(30 * 60),
        |state| println!("  build: {state}"),
    )
    .await?
    {
        flow::DeployReady::Ready => println!("  release is ready — activating.\n"),
        flow::DeployReady::Failed(msg) => {
            bail!("the release build failed: {msg}");
        }
        flow::DeployReady::NoCi(msg) => {
            bail!(
                "no CI ran for this deployment, so there is no release to activate: {msg}\n\
                 The deploy PR opens on the platform repo — check that it has a release \
                 workflow and that the PR was picked up:\n  {}",
                deployment
                    .deployment
                    .platform
                    .pr_url
                    .as_deref()
                    .unwrap_or("(no PR URL recorded)")
            );
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

    loop {
        let result = ActivateArgs {
            platform: Some(Platform::new(platform)),
            backend: Some(backend_url.to_string()),
            build_url: Some(build_url.to_string()),
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

fn local_deployment(dir: &Path) -> Result<LocalDeployment> {
    let (git_root, _) = super::cli::shared::git_context(dir)?;
    LocalDeployment::read(&git_root)?.ok_or_else(|| {
        anyhow!(
            "the deploy did not leave a .aomi/deployment.json at {} — \
             without it there is no deployment id to wait on and no release tag to \
             activate. Re-run the deploy, or activate manually once CI is green.",
            git_root.display()
        )
    })
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
