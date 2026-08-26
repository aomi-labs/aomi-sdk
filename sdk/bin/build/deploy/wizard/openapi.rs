//! Wizard branch that creates an app from an OpenAPI spec.
//!
//! Pure local codegen — it never touches Build, which is why it needs no
//! session and sits apart from the deploy branch.

use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use inquire::{Confirm, Select, Text};

use super::print_step_error;
use crate::new_app::NewAppArgs;
use crate::specs::workspace_root;

/// Create an app inside the current Aomi workspace using the OpenAPI-driven
/// codegen pipeline, then optionally hand off the generated stubs to Codex or
/// Claude for skill-guided curation.
pub(super) async fn app_flow() -> Result<()> {
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
