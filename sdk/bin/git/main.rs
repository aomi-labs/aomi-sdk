//! `aomi-git` — backend-relayed deploy CLI for Aomi apps.
//!
//! See ADR 0004 (`aomi-git deploy contract`) for the user-facing contract.

mod activate;
mod app;
mod backend;
mod cli;
mod deployment_state;
mod discord;
mod git;
mod plan;
mod platform;
mod preflight;
mod status;

#[cfg(test)]
mod tests;

use anyhow::Result;
use clap::Parser;

use crate::cli::Cli;

#[tokio::main]
async fn main() -> Result<()> {
    Cli::parse().run().await
}
