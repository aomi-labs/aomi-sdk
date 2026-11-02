//! `project` — connect a GitHub repository to one Aomi platform.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};

use super::login::{authenticate_with_options, saved_builder_github_identity};
use super::shared::{
    git_context, missing_backend, remote_origin, resolve_activation_token, resolve_backend,
    resolve_build_url,
};
use crate::deploy::backend::{BackendClient, oauth_result, oauth_start};
use crate::deploy::platform::{Platform, normalize_github_repo};
use crate::deploy::project_config::ProjectConfig;
use crate::deploy::types::{CreateProjectInput, ProjectResult};

const INSTALL_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const INSTALL_POLL: Duration = Duration::from_secs(2);

#[derive(Debug, Args, Clone)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub cmd: ProjectCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum ProjectCmd {
    /// Connect this repository and print its platform-bound Project id.
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
    /// Source repo, `owner/name`. Defaults to this checkout's origin.
    #[arg(long, value_name = "OWNER/REPO")]
    pub repo: Option<String>,
    #[arg(long, value_name = "NAME", default_value_t = Platform::default())]
    pub platform: Platform,
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,
    /// Aomi Build URL for the Builder login. Inferred from known backends.
    #[arg(long = "build-url", value_name = "URL")]
    pub build_url: Option<String>,
    /// Admin/headless path. Without it, the project is connected through your
    /// Builder login and a GitHub App approval in the browser.
    #[arg(long, value_name = "TOKEN")]
    pub activation_token: Option<String>,
    /// Print the GitHub URL instead of opening a browser.
    #[arg(long)]
    pub no_browser: bool,
    /// Source repo path where `.aomi/config.json` is created.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,
    #[arg(long)]
    pub json: bool,
}

impl ProjectCreateArgs {
    pub async fn run(self) -> Result<()> {
        let (repo_root, _) = git_context(&self.path)?;
        let local_repo = normalize_github_repo(&remote_origin(&repo_root)?)?;
        let repo = match &self.repo {
            Some(repo) => normalize_github_repo(repo)?,
            None => local_repo.clone(),
        };
        if repo != local_repo {
            bail!("--repo `{repo}` does not match this checkout's origin `{local_repo}`");
        }
        let backend_url =
            resolve_backend(&self.backend).ok_or_else(|| missing_backend("project create"))?;
        let (config, config_path) = ProjectConfig::create(&repo_root, &self.platform)?;
        let applications = config.applications().len();

        match resolve_activation_token(&self.activation_token) {
            Some(token) => {
                let result = self.create_with_token(&repo, backend_url, token).await?;
                report_project(&result, &config_path, applications, self.json)
            }
            None => {
                let (login, installation_id) = self.connect_with_login(&repo, &backend_url).await?;
                report_connected(
                    &repo,
                    &login,
                    installation_id,
                    &config_path,
                    applications,
                    self.json,
                )
            }
        }
    }

    async fn create_with_token(
        &self,
        repo: &str,
        backend_url: String,
        token: String,
    ) -> Result<ProjectResult> {
        let github_user_id = match saved_builder_github_identity() {
            Some((id, _)) => Some(id),
            None => {
                if !self.json {
                    println!(
                        "  no Builder login found — creating the project on the activation \
                         token alone (run `aomi-build login` first to claim it for your \
                         GitHub account)"
                    );
                }
                None
            }
        };
        BackendClient::new(backend_url, token)?
            .create_project(
                &self.platform,
                &CreateProjectInput {
                    repo: repo.to_string(),
                    github_user_id,
                },
            )
            .await
    }

    /// The Build site's "import a repository" flow: the GitHub App approval
    /// names the repo, and the backend's install callback creates the Project
    /// for the GitHub account that approved it. We wait on `oauth/result`.
    async fn connect_with_login(&self, repo: &str, backend_url: &str) -> Result<(String, i64)> {
        let build_url = resolve_build_url(&self.build_url, Some(backend_url))
            .context("no Aomi Build URL known for this backend; pass --build-url")?;
        let login = authenticate_with_options(&build_url, self.no_browser)
            .await?
            .identity
            .github_login;

        let start = oauth_start(backend_url, self.platform.as_str(), Some(repo), "install").await?;
        let state = start
            .state
            .context("the backend did not return an install state; upgrade aomi-build")?;
        if !self.json {
            println!("Approve the Aomi GitHub App for {repo} as @{login}:");
            println!("  {}", start.install_url);
        }
        if !self.no_browser && open::that(&start.install_url).is_ok() && !self.json {
            println!("  (opened in your browser)");
        }

        let started = Instant::now();
        loop {
            let result = oauth_result(backend_url, &state).await?;
            if let Some(installation_id) = result.installation_id {
                return Ok((login, installation_id));
            }
            if result.status == "failed" {
                let reason = result
                    .error
                    .as_deref()
                    .unwrap_or("GitHub App approval failed");
                bail!("couldn't connect {repo}: {reason}");
            }
            if started.elapsed() >= INSTALL_TIMEOUT {
                bail!(
                    "timed out waiting for the GitHub App approval for {repo}. If GitHub showed \
                     an error, fix it there (you need admin rights on the repo) and run this again"
                );
            }
            tokio::time::sleep(INSTALL_POLL).await;
        }
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

fn report_connected(
    repo: &str,
    login: &str,
    installation_id: i64,
    config_path: &std::path::Path,
    application_count: usize,
    json: bool,
) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "repository_link": repo,
                "github_login": login,
                "installation_id": installation_id,
                "config_path": config_path,
                "applications": application_count,
            }))?
        );
    } else {
        println!("connected `{repo}` to @{login} (installation {installation_id})");
        println!("  config: {}", config_path.display());
        println!("  applications: {application_count}");
        println!("  commit and push the config, then run `aomi-build deploy`");
    }
    Ok(())
}
