//! `aomi-git` — backend-relayed deploy CLI for Aomi apps.
//!
//! See `docs/platform-ralph/CONTRACTS.md` (product-mono) for the repo-scoped
//! deploy/activate wire contract this CLI relays to.

mod app;
mod backend;
mod cli;
mod discord;
mod git;
mod local;
mod platform;
mod status;
/// Repo-scoped deploy/activate wire contract + local `.aomi/deployment.json`
/// state (CONTRACTS.md). Single canonical home for both.
mod wire;

#[cfg(test)]
mod tests;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    Cli::parse().run().await
}
