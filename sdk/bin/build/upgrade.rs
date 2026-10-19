use anyhow::{Context, Result, bail};
use clap::Args;
use std::process::Command;

#[derive(Debug, Args)]
pub struct UpgradeArgs {
    /// Backend whose SDK ABI the CLI must match.
    #[arg(long)]
    pub backend: Option<String>,
    /// Install an explicit published SDK/CLI version.
    #[arg(long)]
    pub version: Option<String>,
    /// Print the exact install command without executing it.
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(args: UpgradeArgs) -> Result<()> {
    let backend = crate::deploy::cli::shared::resolve_backend(&args.backend);
    if backend.is_none() && args.version.is_none() {
        bail!(
            "upgrade needs --backend <url> or --version <version> so the installed CLI matches your target"
        );
    }
    let version =
        crate::sdk_guard::resolve_required_sdk_version(backend.as_deref(), args.version).await?;
    // Only a plain published version is accepted; never interpret flags or shell code.
    if version.split('.').count() != 3
        || !version
            .split('.')
            .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    {
        bail!("invalid published SDK version `{version}`");
    }
    let install = [
        "install",
        "aomi-sdk",
        "--version",
        &version,
        "--features",
        "cli",
        "--bin",
        "aomi-build",
        "--locked",
        "--force",
    ];
    eprintln!(
        "Installing aomi-build {version} (current {})",
        aomi_sdk::AOMI_SDK_VERSION
    );
    if args.dry_run {
        println!("cargo {}", install.join(" "));
        return Ok(());
    }
    let status = Command::new("cargo")
        .args(install)
        .status()
        .context("failed to start cargo install")?;
    if !status.success() {
        bail!("CLI installation failed ({status}); the previous CLI remains available");
    }
    Ok(())
}
