//! `apps` — list a platform's apps.

use anyhow::Result;
use clap::{Args, Subcommand};

use super::shared::resolve_activation;
use crate::deploy::backend::BackendClient;
use crate::deploy::platform::Platform;

#[derive(Debug, Args, Clone)]
pub struct AppsArgs {
    #[command(subcommand)]
    pub cmd: AppsCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum AppsCmd {
    /// List a platform's apps.
    List(AppsListArgs),
}

impl AppsArgs {
    pub async fn run(self) -> Result<()> {
        match self.cmd {
            AppsCmd::List(a) => a.run().await,
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct AppsListArgs {
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
}

impl AppsListArgs {
    pub async fn run(self) -> Result<()> {
        let (url, token) = resolve_activation("apps list", &self.backend, &self.activation_token)?;
        let value = BackendClient::new(url, token)?
            .list_apps(&self.platform)
            .await?;
        println!("{}", serde_json::to_string_pretty(&value)?);
        Ok(())
    }
}
