use clap::Args;
use eyre::{Context, Result};
use std::process::Command;

use crate::client::GenClientArgs;
use crate::specs::workspace_root;
use crate::specs::{GenSpecsArgs, Source};
use crate::tool::GenToolArgs;

#[derive(Args, Debug)]
pub struct NewAppArgs {
    /// Platform name.
    pub platform: String,

    /// Skip discovery and fetch directly from this URL.
    #[arg(long)]
    pub from_url: Option<String>,

    /// Non-interactive: include every operation as a tool.
    #[arg(long)]
    pub all: bool,

    /// Overwrite existing spec, generated client, and app sources.
    #[arg(long)]
    pub force: bool,

    /// Stop after gen-client (don't scaffold the app).
    #[arg(long)]
    pub no_tool: bool,

    /// Skip the final `cargo build -p <platform>` verification.
    #[arg(long)]
    pub no_build: bool,

    /// Treat as a shared library (lives under ext/). Default is app-local.
    #[arg(long)]
    pub shared: bool,
}

pub fn run(args: NewAppArgs) -> Result<()> {
    let platform = args.platform.clone();

    println!("=== [1/3] gen-specs {platform} ===");
    crate::specs::run(GenSpecsArgs {
        platform: platform.clone(),
        source: Source::All,
        out: None,
        force: args.force,
        from_url: args.from_url.clone(),
        shared: args.shared,
    })
    .with_context(|| "gen-specs failed")?;

    println!();
    println!("=== [2/3] gen-client {platform} ===");
    crate::client::run(GenClientArgs {
        platform: platform.clone(),
        spec: None,
        out: None,
        force: args.force,
        shared: args.shared,
    })
    .with_context(|| "gen-client failed")?;

    if args.no_tool {
        println!();
        println!(
            "Stopped before gen-tool (--no-tool). Run `aomi-build gen-tool {platform}` to scaffold."
        );
        return Ok(());
    }

    println!();
    println!("=== [3/3] gen-tool {platform} ===");
    crate::tool::run(GenToolArgs {
        platform: platform.clone(),
        spec: None,
        out: None,
        all: args.all,
        force: args.force,
        shared: args.shared,
    })
    .with_context(|| "gen-tool failed")?;

    if !args.no_build {
        println!();
        println!("=== [verify] cargo build -p {platform} ===");
        let root = workspace_root()?;
        let status = Command::new("cargo")
            .args(["build", "-p", &platform])
            .current_dir(&root)
            .status()
            .with_context(|| "failed to spawn cargo")?;
        if !status.success() {
            eyre::bail!("cargo build -p {platform} failed");
        }
    }

    println!();
    println!("✓ new-app {platform} complete");
    Ok(())
}
