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

use super::backend;
use super::cli::login;
use super::cli::shared::{BUILD_URL_ENV, git_context, infer_build_url};
use super::cli::{ActivateArgs, DeployArgs};
use super::config::AomiConfig;
use super::flow;
use super::platform::{Platform, normalize_github_repo};
use super::types::LocalDeployment;

const STAGING_URL: &str = "https://api-staging.aomi.dev";
const PROD_URL: &str = "https://api.aomi.dev";

pub async fn run() -> Result<()> {
    println!("aomi-build — let's get your app deployed.\n");
    let mut config = AomiConfig::load();

    let backend_url = pick_backend(&config)?;
    config.backend_url = Some(backend_url.clone());
    let build_url = pick_build_url(&config, &backend_url)?;
    config.build_url = Some(build_url.clone());

    // Human deploys are always tied to a verified GitHub Builder. The saved
    // session is reused only after Build validates it.
    config = login::ensure_logged_in(&build_url, false).await?.config;
    config.backend_url = Some(backend_url.clone());
    config.build_url = Some(build_url.clone());

    // The deploy *destination* platform (a `DbPlatform`, e.g. `community`) —
    // not the source/template repo it's copied from.
    let platform = Text::new("Deploy to which platform?")
        .with_default(config.platform.as_deref().unwrap_or("community"))
        .prompt()
        .context("wizard cancelled")?
        .trim()
        .to_string();
    config.platform = Some(platform.clone());

    // Persist environment/platform selection. Login already persisted the
    // Builder session.
    config.save().context("couldn't save your Aomi config")?;

    // Main loop: a failed step prints its error and returns here instead of
    // tearing down the wizard. Only an explicit Quit (or Ctrl-C at a prompt)
    // exits.
    loop {
        let choice = Select::new(
            "What do you want to do?",
            vec![
                "Deploy the app in a local directory",
                "Scaffold a new app from the example template",
                "Quit",
            ],
        )
        .prompt()
        .context("wizard cancelled")?;

        let outcome = match choice {
            c if c.starts_with("Deploy") => {
                existing_dir_flow(&backend_url, &build_url, &platform).await
            }
            c if c.starts_with("Scaffold") => {
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

fn pick_build_url(config: &AomiConfig, backend_url: &str) -> Result<String> {
    if let Some(url) = config.build_url.as_deref().filter(|url| !url.is_empty()) {
        return Ok(url.trim_end_matches('/').to_string());
    }
    if let Some(url) = infer_build_url(backend_url) {
        return Ok(url);
    }
    let url = Text::new(&format!("Aomi Build URL ({BUILD_URL_ENV}):"))
        .prompt()
        .context("wizard cancelled")?;
    let url = url.trim().trim_end_matches('/');
    if url.is_empty() {
        bail!("Aomi Build URL is required");
    }
    Ok(url.to_string())
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

/// Install the aomi-build GitHub App on `repo` from inside the wizard: open the
/// install URL and wait for the user to finish in their browser. We don't need
/// the resulting installation id — deploy resolves the source by repo via
/// sync-installed — so this is just "open and wait".
async fn ensure_app_installed(backend_url: &str, platform: &str, repo: Option<&str>) -> Result<()> {
    let start = backend::oauth_start(backend_url, platform, repo, "install").await?;
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
    if !Confirm::new(&format!("Deploy `{platform}` from {}?", dir.display()))
        .with_default(true)
        .prompt()
        .context("wizard cancelled")?
    {
        println!("Stopped before deploy. Re-run `aomi-build` anytime.");
        return Ok(());
    }

    // Deploy with inline retry — a transient backend failure prints and offers
    // a retry instead of unwinding the whole wizard.
    let mut offered_install = false;
    loop {
        let result = DeployArgs {
            platform: Some(Platform::new(platform)),
            app_source_id: None,
            branch: None,
            commit: None,
            aomi_toml: vec![],
            backend: Some(backend_url.to_string()),
            build_url: Some(build_url.to_string()),
            path: dir.to_path_buf(),
            preflight: false,
            json: false,
        }
        .run()
        .await;
        match result {
            Ok(()) => break,
            Err(e) => {
                print_step_error(&e);
                let message = e.to_string().to_ascii_lowercase();
                if !offered_install
                    && (message.contains("source")
                        || message.contains("installation")
                        || message.contains("github"))
                {
                    offered_install = true;
                    if Confirm::new("Install or re-authorize the Aomi GitHub App for this repo?")
                        .with_default(true)
                        .prompt()
                        .context("wizard cancelled")?
                    {
                        ensure_app_installed(backend_url, platform, Some(repo)).await?;
                        continue;
                    }
                }
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
    let deployment = local_deployment(dir);
    if let Some(id) = deployment.as_ref().map(|state| &state.deployment.id) {
        let client = login::ensure_logged_in(build_url, false).await?.client;
        println!("Waiting for the release build (up to 30 min, Ctrl-C to stop)…");
        match flow::poll_build_deployment_ready(
            &client,
            platform,
            id,
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
            apps: vec![],
            platform: Some(Platform::new(platform)),
            release_tags: vec![],
            backend: Some(backend_url.to_string()),
            build_url: Some(build_url.to_string()),
            activation_token: None,
            // Empty: let the backend use the deployment's server_tags, as the
            // portal does (it sends no target_tags).
            target_tags: vec![],
            path: dir.to_path_buf(),
            dry_run: false,
            json: false,
        }
        .run()
        .await;
        match result {
            Ok(()) => {
                if let Some(url) = deployment.and_then(|state| state.project_url) {
                    println!("\nView your deployment:\n  {url}");
                }
                return Ok(());
            }
            Err(e) => {
                print_step_error(&e);
                if !retry("Retry activation?")? {
                    return Ok(());
                }
            }
        }
    }
}

fn local_deployment(dir: &Path) -> Option<LocalDeployment> {
    let git_root = git_context(dir).ok()?.0;
    LocalDeployment::read(&git_root).ok().flatten()
}

/// Small yes/no retry prompt; a cancel (Ctrl-C) propagates out to exit.
fn retry(question: &str) -> Result<bool> {
    Confirm::new(question)
        .with_default(true)
        .prompt()
        .context("wizard cancelled")
}

/// Best-effort `owner/name` from the repo's `origin` remote.
fn git_origin_slug(dir: &Path) -> Option<String> {
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
