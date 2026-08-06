//! `project` — connect a GitHub repository to one Aomi platform.

use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use super::login::verified_builder_github_user_id;
use super::shared::{git_context, remote_origin, resolve_activation};
use crate::deploy::backend::BackendClient;
use crate::deploy::platform::{Platform, normalize_github_repo};
use crate::deploy::project_config::ProjectConfig;
use crate::deploy::session::Session;
use crate::deploy::types::{CreateProjectInput, ProjectResult};

#[derive(Debug, Args, Clone)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub cmd: ProjectCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum ProjectCmd {
    /// Connect an installed repository and print its platform-bound Project id.
    Create(ProjectCreateArgs),
}

impl ProjectArgs {
    pub async fn run(self) -> Result<()> {
        match self.cmd {
            ProjectCmd::Create(a) => a.run().await,
        }
    }
}

#[derive(Debug, Args, Clone)]
pub struct ProjectCreateArgs {
    /// Source repo, `owner/name`.
    #[arg(long, value_name = "OWNER/REPO")]
    pub repo: String,
    #[arg(long, value_name = "NAME")]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    /// Aomi Build URL used to verify the Builder identity that owns the source.
    #[arg(long = "build-url", value_name = "URL")]
    pub build_url: Option<String>,
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
    /// Source repo path where `.aomi/config.json` is created.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub json: bool,
}

impl ProjectCreateArgs {
    pub async fn run(self) -> Result<()> {
        let repo = normalize_github_repo(&self.repo)?;
        let (repo_root, _) = git_context(&self.path)?;
        let local_repo = normalize_github_repo(&remote_origin(&repo_root)?)?;
        if repo != local_repo {
            anyhow::bail!("--repo `{repo}` does not match this checkout's origin `{local_repo}`");
        }
        let (config, config_path) = ProjectConfig::create(&repo_root, &self.platform)?;
        let (url, token) =
            resolve_activation("project create", &self.backend, &self.activation_token)?;
        let session = Session::open(&self.backend, &self.build_url).await?;
        let github_user_id = verified_builder_github_user_id(&session.identity.github_user_id)?;
        let result = BackendClient::new(url, token)?
            .create_project(
                &self.platform,
                &CreateProjectInput {
                    repo: repo.clone(),
                    github_user_id,
                },
            )
            .await?;
        report_project(
            &result,
            &config_path,
            config.applications().len(),
            self.json,
        )
    }
}

fn report_project(
    result: &ProjectResult,
    config_path: &std::path::Path,
    application_count: usize,
    json: bool,
) -> Result<()> {
    let id = result.project.id;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project_id": id,
                "repository_link": result.project.repository_link,
                "installation_id": result.project.installation_id,
                "platform_id": result.project.platform_id,
                "config_path": config_path,
                "applications": application_count,
            }))?
        );
    } else {
        println!(
            "created project `{}` (installation {})",
            result.project.repository_link, result.project.installation_id
        );
        println!("  project_id: {id}");
        println!("  config: {}", config_path.display());
        println!("  applications: {application_count}");
        println!("  commit and push the config before deploying");
    }
    Ok(())
}
