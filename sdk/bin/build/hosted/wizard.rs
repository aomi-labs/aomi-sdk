//! Guided interactive flow for `aomi-build` with no subcommand — a Claude-Code
//! style first-run wizard that walks a user Connect → Source → Deploy →
//! Activate. It's built on `inquire` prompts over the same command structs the
//! CLI exposes (`DeployArgs`, `ActivateArgs`, …) and the shared `flow` core, so
//! nothing the wizard does is unavailable to scripting.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use inquire::{Confirm, Select, Text};

use super::cli::{ActivateArgs, DeployArgs, ScaffoldArgs};
use super::config::AomiConfig;
use super::flow;
use super::platform::{normalize_github_repo, Platform};

const STAGING_URL: &str = "https://api-staging.aomi.dev";
const PROD_URL: &str = "https://api.aomi.dev";

pub async fn run() -> Result<()> {
    println!("aomi-build — let's get your app deployed.\n");
    let mut config = AomiConfig::load();

    let backend_url = pick_backend(&config)?;
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

    let token = ensure_token(&mut config, &backend_url, &platform).await?;
    // Persist what we have so a re-run resumes with backend/token/platform set.
    let _ = config.save();

    // Main loop: a failed step prints its error and returns here instead of
    // tearing down the wizard. Only an explicit Quit (or Ctrl-C at a prompt)
    // exits.
    loop {
        let choice = Select::new(
            "What do you want to do?",
            vec![
                "Deploy an app from a local directory",
                "Scaffold a new app from a template",
                "Quit",
            ],
        )
        .prompt()
        .context("wizard cancelled")?;

        let outcome = match choice {
            c if c.starts_with("Deploy") => {
                existing_dir_flow(&backend_url, &platform, &token).await
            }
            c if c.starts_with("Scaffold") => {
                scaffold_flow(&config, &backend_url, &platform, &token).await
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
async fn ensure_token(config: &mut AomiConfig, backend_url: &str, platform: &str) -> Result<String> {
    if let Some(saved) = config.activation_token.clone() {
        if flow::validate_activation_token(backend_url, &saved, platform).await {
            println!("Using your saved activation token (verified).\n");
            return Ok(saved);
        }
        println!("Saved token didn't verify for `{platform}` — let's set a new one.");
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
        if flow::validate_activation_token(backend_url, &token, platform).await {
            println!("  token verified.\n");
        } else {
            let keep = Confirm::new("Couldn't verify that token — use it anyway?")
                .with_default(false)
                .prompt()
                .context("wizard cancelled")?;
            if !keep {
                continue;
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

    println!("Resolving the connected source…");
    let app_source_id = flow::sync_source(backend_url, token, platform, &repo)
        .await
        .context("source sync failed — is the Aomi GitHub App installed on this repo? (try `aomi-build connect --repo <owner/repo>`)")?;
    println!("  app_source_id: {app_source_id}\n");

    deploy_then_activate(platform, backend_url, token, &dir, Some(app_source_id)).await
}

async fn scaffold_flow(
    config: &AomiConfig,
    backend_url: &str,
    platform: &str,
    token: &str,
) -> Result<()> {
    let repo_name = Text::new("New repo name:")
        .prompt()
        .context("wizard cancelled")?
        .trim()
        .to_string();
    let installation_id = match config.installation_id {
        Some(id) => id,
        None => Text::new("GitHub App installation id (run `aomi-build connect` if unknown):")
            .prompt()
            .context("wizard cancelled")?
            .trim()
            .parse()
            .context("installation_id must be a number")?,
    };

    ScaffoldArgs {
        repo_name,
        installation_id,
        platform: Platform::new(platform),
        template: "aomi-labs/playground-example".to_string(),
        private: false,
        backend: Some(backend_url.to_string()),
        activation_token: Some(token.to_string()),
        path: PathBuf::from("."),
        json: false,
    }
    .run()
    .await?;

    println!(
        "\nScaffolded. Clone the new repo, then re-run `aomi-build` inside it to deploy:\n  \
         git clone <the repo URL above> && cd <repo> && aomi-build"
    );
    Ok(())
}

async fn deploy_then_activate(
    platform: &str,
    backend_url: &str,
    token: &str,
    dir: &PathBuf,
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
        let result = DeployArgs {
            platform: Some(Platform::new(platform)),
            app_source_id,
            branch: None,
            commit: None,
            aomi_toml: vec![],
            backend: Some(backend_url.to_string()),
            path: dir.clone(),
            dry_run: false,
            json: false,
        }
        .run()
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

    let activate = Confirm::new("CI builds the release (a few minutes). Activate now?")
        .with_default(false)
        .prompt()
        .context("wizard cancelled")?;
    if !activate {
        println!(
            "When CI is green:\n  aomi-build activate --path {} --target-tag staging",
            dir.display()
        );
        return Ok(());
    }

    loop {
        let result = ActivateArgs {
            apps: vec![],
            platform: Some(Platform::new(platform)),
            release_tags: vec![],
            backend: Some(backend_url.to_string()),
            activation_token: Some(token.to_string()),
            target_tags: vec!["staging".to_string()],
            path: dir.clone(),
            dry_run: false,
            json: false,
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
