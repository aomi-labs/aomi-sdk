use crate::deploy::session::Session;
use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use std::{io::Read, path::PathBuf};

#[derive(Debug, Args, Clone)]
pub struct Connection {
    #[arg(long)]
    backend: Option<String>,
    #[arg(long)]
    build_url: Option<String>,
}

#[derive(Debug, Args)]
pub struct LogsArgs {
    #[command(flatten)]
    connection: Connection,
    /// Restrict logs to an owned project.
    #[arg(long)]
    project_id: Option<i64>,
    #[arg(long)]
    platform: Option<String>,
    #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u16).range(1..=200))]
    limit: u16,
    /// Pagination cursor returned by the previous response, as JSON.
    #[arg(long)]
    cursor: Option<String>,
}

impl LogsArgs {
    pub async fn run(self) -> Result<()> {
        let session = Session::open(&self.connection.backend, &self.connection.build_url).await?;
        let mut query = vec![("limit", self.limit.to_string())];
        if let Some(id) = self.project_id {
            query.push(("projectId", id.to_string()));
        }
        if let Some(platform) = self.platform {
            query.push(("platform", platform));
        }
        if let Some(cursor) = self.cursor {
            serde_json::from_str::<serde_json::Value>(&cursor).context("--cursor must be JSON")?;
            query.push(("cursor", cursor));
        }
        let result = session
            .client
            .get_json("/api/bff/operate/logs", &query)
            .await?;
        println!("{}", serde_json::to_string_pretty(&result)?);
        Ok(())
    }
}

#[derive(Debug, Args)]
pub struct EnvArgs {
    #[command(subcommand)]
    cmd: EnvCmd,
}

#[derive(Debug, Args)]
pub struct EnvTarget {
    #[command(flatten)]
    connection: Connection,
    /// Stable application ID from activation/status, checked against your ownership.
    #[arg(long = "app-id", alias = "application-id")]
    app_id: i64,
}

#[derive(Debug, Subcommand)]
enum EnvCmd {
    /// List configured key names; values are never returned.
    List(EnvTarget),
    /// Set any key, including keys not declared in the app manifest.
    Set {
        #[command(flatten)]
        target: EnvTarget,
        key: String,
        /// Read the value from this file. Otherwise reads stdin.
        #[arg(long)]
        file: Option<PathBuf>,
    },
    Unset {
        #[command(flatten)]
        target: EnvTarget,
        key: String,
    },
}

impl EnvArgs {
    pub async fn run(self) -> Result<()> {
        let target = match &self.cmd {
            EnvCmd::List(target) | EnvCmd::Set { target, .. } | EnvCmd::Unset { target, .. } => {
                target
            }
        };
        if target.app_id <= 0 {
            bail!("--app-id must be positive");
        }
        let session =
            Session::open(&target.connection.backend, &target.connection.build_url).await?;
        let result = match self.cmd {
            EnvCmd::List(target) => {
                session
                    .client
                    .get_json(
                        "/api/bff/deployments/secrets",
                        &[("applicationId", target.app_id.to_string())],
                    )
                    .await?
            }
            EnvCmd::Set { target, key, file } => {
                validate_key(&key)?;
                let mut value = String::new();
                match file {
                    Some(path) => {
                        value =
                            std::fs::read_to_string(path).context("failed to read secret file")?
                    }
                    None => {
                        std::io::stdin()
                            .take(1024 * 1024 + 1)
                            .read_to_string(&mut value)
                            .context("failed to read secret from stdin")?;
                    }
                }
                if value.is_empty() || value.len() > 1024 * 1024 {
                    bail!("secret must contain 1 byte to 1 MiB");
                }
                let secrets = serde_json::json!({ key: value.trim_end_matches(['\r', '\n']) });
                session
                    .client
                    .post_json(
                        "/api/bff/deployments/secrets",
                        &serde_json::json!({"applicationId": target.app_id, "secrets": secrets}),
                    )
                    .await?
            }
            EnvCmd::Unset { target, key } => {
                validate_key(&key)?;
                session
                    .client
                    .delete_json(
                        "/api/bff/deployments/secrets",
                        &serde_json::json!({"applicationId": target.app_id, "name": key}),
                    )
                    .await?
            }
        };
        println!("{}", serde_json::to_string_pretty(&result)?);
        Ok(())
    }
}

fn validate_key(key: &str) -> Result<()> {
    if key.is_empty()
        || !key.chars().enumerate().all(|(index, ch)| {
            ch == '_' || ch.is_ascii_alphabetic() || (index > 0 && ch.is_ascii_digit())
        })
    {
        bail!(
            "environment key must use letters, digits and underscores and must not start with a digit"
        );
    }
    Ok(())
}
