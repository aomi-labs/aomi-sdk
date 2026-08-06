//! `project` — connect a GitHub repository to one Aomi platform.

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::{Args, Subcommand};

use super::login::verified_builder_github_user_id;
use super::shared::{PROJECT_ID_ENV, git_context, resolve_activation};
use crate::deploy::backend::BackendClient;
use crate::deploy::platform::{Platform, normalize_github_repo};
use crate::deploy::session::Session;
use crate::deploy::state::LocalDeployment;
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
    /// Source repo path for `.aomi/deployment.json` persistence.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub json: bool,
}

impl ProjectCreateArgs {
    pub async fn run(self) -> Result<()> {
        let repo = normalize_github_repo(&self.repo)?;
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
        report_project(&result, &self.path, self.json)
    }
}

fn report_project(result: &ProjectResult, path: &Path, json: bool) -> Result<()> {
    let id = result.project.id;
    let persisted = persist_project_id(path, id);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "project_id": id,
                "repository_link": result.project.repository_link,
                "installation_id": result.project.installation_id,
                "platform_id": result.project.platform_id,
                "persisted": persisted.is_some(),
            }))?
        );
    } else {
        println!(
            "created project `{}` (installation {})",
            result.project.repository_link, result.project.installation_id
        );
        println!("  project_id: {id}");
        match persisted {
            Some(p) => println!(
                "  recorded in {} — deploy will auto-resolve it",
                p.display()
            ),
            None => {
                println!("  no .aomi/deployment.json yet; pass it to the first deploy:");
                println!(
                    "    aomi-build deploy --project-id {id}   (or export {PROJECT_ID_ENV}={id})"
                );
            }
        }
    }
    Ok(())
}

/// Record a resolved Project id into an existing `.aomi/deployment.json`.
fn persist_project_id(path: &Path, project_id: i64) -> Option<PathBuf> {
    let repo_root = git_context(path)
        .map(|(root, _)| root)
        .unwrap_or_else(|_| path.to_path_buf());
    let mut state = LocalDeployment::read(&repo_root).ok().flatten()?;
    state.set_project_id(project_id);
    state.write(&repo_root).ok()
}
