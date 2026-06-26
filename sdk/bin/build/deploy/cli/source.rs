//! `source` — resolve a connected source repo to its `app_source_id`.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Args, Subcommand};

use super::shared::{APP_SOURCE_ID_ENV, git_context, resolve_activation};
use crate::deploy::backend::BackendClient;
use crate::deploy::platform::{Platform, normalize_github_repo};
use crate::deploy::types::{LocalDeployment, SourceResult, SyncSourceInput};

pub async fn run(args: SourceArgs) -> eyre::Result<()> {
    args.run().await.map_err(crate::git_error)
}

#[derive(Debug, Args, Clone)]
pub struct SourceArgs {
    #[command(subcommand)]
    pub cmd: SourceCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum SourceCmd {
    /// Resolve-or-bind an installed source repo and print its `app_source_id`.
    Sync(SourceSyncArgs),
}

impl SourceArgs {
    pub async fn run(self) -> Result<()> {
        match self.cmd {
            SourceCmd::Sync(a) => a.run().await,
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct SourceSyncArgs {
    /// Source repo, `owner/name`.
    #[arg(long, value_name = "OWNER/REPO")]
    pub repo: String,
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
    /// Source repo path for `.aomi/deployment.json` persistence.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub json: bool,
}

impl SourceSyncArgs {
    pub async fn run(self) -> Result<()> {
        let repo = normalize_github_repo(&self.repo)?;
        let (url, token) = resolve_activation(&self.backend, &self.activation_token)?;
        let result = BackendClient::new(url, token)?
            .sync_installed(&self.platform, &SyncSourceInput { repo: repo.clone() })
            .await?;
        report_source(&result, &self.path, self.json, "synced")
    }
}

/// Shared output + persistence for `source sync` / `scaffold` — both return a
/// resolved `app_source` whose id deploy needs.
fn report_source(result: &SourceResult, path: &Path, json: bool, verb: &str) -> Result<()> {
    let id = result.source.id;
    let persisted = persist_app_source_id(path, id);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "app_source_id": id,
                "repository_link": result.source.repository_link,
                "installation_id": result.source.installation_id,
                "bound_platform_id": result.source.bound_platform_id,
                "persisted": persisted.is_some(),
            }))?
        );
    } else {
        println!(
            "{verb} source `{}` (installation {})",
            result.source.repository_link, result.source.installation_id
        );
        println!("  app_source_id: {id}");
        match persisted {
            Some(p) => println!(
                "  recorded in {} — deploy will auto-resolve it",
                p.display()
            ),
            None => {
                println!("  no .aomi/deployment.json yet; pass it to the first deploy:");
                println!(
                    "    aomi-build deploy --app-source-id {id}   (or export {APP_SOURCE_ID_ENV}={id})"
                );
            }
        }
    }
    Ok(())
}

/// Record a resolved `app_source_id` into an existing `.aomi/deployment.json`
/// (if one exists) so the next deploy auto-resolves it. Returns the written
/// path, or `None` when there's no deployment record yet.
fn persist_app_source_id(path: &Path, app_source_id: i64) -> Option<PathBuf> {
    let repo_root = git_context(path)
        .map(|(root, _)| root)
        .unwrap_or_else(|_| path.to_path_buf());
    let mut state = LocalDeployment::read(&repo_root).ok().flatten()?;
    state.set_app_source_id(app_source_id);
    state.write(&repo_root).ok()
}
