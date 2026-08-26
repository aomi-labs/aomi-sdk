//! Guided interactive flow for `aomi-build` with no subcommand — a Claude-Code
//! style first-run wizard that walks a user Login → Source → Deploy →
//! Activate. It's built on `inquire` prompts over the same command structs the
//! CLI exposes (`DeployArgs`, `ActivateArgs`, …) and the shared `flow` core, so
//! nothing the wizard does is unavailable to scripting.

mod deploy;
mod openapi;

use anyhow::{Context, Result, anyhow};
use inquire::{Confirm, Select, Text};

use super::cli::shared::resolve_build_url;
use super::config::AomiConfig;
use super::session::Session;

const STAGING_URL: &str = "https://api-staging.aomi.dev";
const PROD_URL: &str = "https://api.aomi.dev";

pub async fn run() -> Result<()> {
    println!(
        "aomi-build {} — let's get your app deployed.\n",
        aomi_sdk::AOMI_SDK_VERSION
    );
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
            c if c.starts_with("Deploy") => match deployment_context(&mut config).await {
                Ok((session, platform)) => deploy::existing_dir_flow(&session, &platform).await,
                Err(e) => Err(e),
            },
            c if c.starts_with("Create") => openapi::app_flow().await,
            c if c.starts_with("Scaffold") => match deployment_context(&mut config).await {
                Ok((session, platform)) => deploy::scaffold_flow(&session, &platform).await,
                Err(e) => Err(e),
            },
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

/// Establish the session and destination platform every deploy flow needs.
async fn deployment_context(config: &mut AomiConfig) -> Result<(Session, String)> {
    let backend_url = pick_backend(config)?;
    let build_url = resolve_build_url(&None, Some(&backend_url)).ok_or_else(|| {
        anyhow!(
            "custom backend `{backend_url}` needs AOMI_BUILD_URL or a saved `aomi-build login --build-url ...`"
        )
    })?;
    // May run a browser login, which persists the CLI bearer to the config file.
    let session = Session::at(&build_url, Some(backend_url.clone())).await?;

    // The deploy *destination* platform (a `DbPlatform`, e.g. `community`) —
    // not the source/template repo it's copied from.
    let platform = Text::new("Deploy to which platform?")
        .with_default(config.platform.as_deref().unwrap_or("community"))
        .prompt()
        .context("wizard cancelled")?
        .trim()
        .to_string();

    // Merge onto whatever is on disk *now* rather than saving the snapshot this
    // function started from: opening the session may have written a fresh bearer
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

    Ok((session, platform))
}

/// Print a step error exactly (the full anyhow chain) without leaving the
/// wizard, so a transient backend failure doesn't kill the session.
pub(super) fn print_step_error(e: &anyhow::Error) {
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
