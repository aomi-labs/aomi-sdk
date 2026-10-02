//! Resolve existing server deployment records; local state is an optional cache.
use super::shared::{git_context, remote_origin};
use crate::deploy::{session::Session, state::LocalDeployment, types::DeployPayload};
use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use std::path::{Path, PathBuf};

#[derive(Debug, Args, Clone, Default)]
pub struct DeploymentSelector {
    /// Owned project ID; works without a local deployment file.
    #[arg(long)]
    pub project_id: Option<i64>,
    /// GitHub source repository; defaults to git origin.
    #[arg(long)]
    pub repo: Option<String>,
    /// Exact source commit to inspect or activate. Defaults to the latest deployment.
    #[arg(long)]
    pub commit: Option<String>,
    /// Select an existing deployment directly.
    #[arg(long)]
    pub deployment_id: Option<String>,
}

impl DeploymentSelector {
    pub fn explicit(&self) -> bool {
        self.project_id.is_some()
            || self.repo.is_some()
            || self.commit.is_some()
            || self.deployment_id.is_some()
    }

    pub async fn resolve(
        &self,
        path: &Path,
        backend: &Option<String>,
        build_url: &Option<String>,
    ) -> Result<(PathBuf, LocalDeployment)> {
        let root = git_context(path)
            .map(|(root, _)| root)
            .unwrap_or(path.canonicalize()?);
        let session = Session::open(backend, build_url).await?;
        let repo = self
            .repo
            .clone()
            .or_else(|| {
                self.project_id
                    .is_none()
                    .then(|| remote_origin(&root).ok())
                    .flatten()
            })
            .map(|repo| crate::deploy::platform::normalize_github_repo(&repo))
            .transpose()?;
        if self.project_id.is_none() && repo.is_none() {
            bail!("select an owned deployment with --project-id <id> or --repo <owner/repo>");
        }
        let projects = session
            .client
            .get_json("/api/bff/deployments/projects", &[])
            .await?;
        let projects = projects
            .get("projects")
            .and_then(|value| value.as_array())
            .context("invalid projects response")?;
        let matches: Vec<_> = projects
            .iter()
            .filter(|project| {
                self.project_id
                    .is_none_or(|id| project["id"].as_i64() == Some(id))
                    && repo.as_ref().is_none_or(|repo| {
                        project["repositoryLink"]
                            .as_str()
                            .and_then(|link| {
                                crate::deploy::platform::normalize_github_repo(link).ok()
                            })
                            .as_ref()
                            == Some(repo)
                    })
            })
            .collect();
        if matches.len() != 1 {
            bail!(
                "expected one owned project for this source, found {}; pass --project-id",
                matches.len()
            );
        }
        let project = matches[0];
        let project_id = project["id"]
            .as_i64()
            .context("project response omitted id")?;
        let platform = project["platformName"]
            .as_str()
            .context("project response omitted platformName")?;
        let ids: Vec<String> = if let Some(id) = &self.deployment_id {
            vec![id.clone()]
        } else {
            let history = session
                .client
                .get_json(
                    "/api/bff/deployments/history",
                    &[
                        ("projectId", project_id.to_string()),
                        ("limit", "100".into()),
                    ],
                )
                .await?;
            let records = history["deployments"]
                .as_array()
                .context("invalid deployment history")?;
            records
                .iter()
                .filter_map(|record| record["deploymentId"].as_str().map(str::to_string))
                .collect()
        };
        let expected_repo = crate::deploy::platform::normalize_github_repo(
            project["repositoryLink"]
                .as_str()
                .context("project response omitted repositoryLink")?,
        )?;
        for id in ids {
            let status = session
                .client
                .get_json(
                    "/api/bff/deployments/status",
                    &[("platform", platform.into()), ("deploymentId", id.clone())],
                )
                .await?;
            let deployment: DeployPayload = serde_json::from_value(status["deployment"].clone())
                .context("deployment status omitted its manifest")?;
            if crate::deploy::platform::normalize_github_repo(&deployment.source.repository_link)?
                != expected_repo
            {
                bail!(
                    "deployment {id} belongs to a different source repository than the selected project"
                );
            }
            if self.commit.as_ref().is_some_and(|commit| {
                deployment.source.commit_hash != commit.trim().to_ascii_lowercase()
            }) {
                continue;
            }
            let mut state = LocalDeployment::from_deploy(
                crate::deploy::types::DeployResult {
                    ok: true,
                    deployment,
                },
                project_id,
            );
            state.state.ci_passed = status["state"] == "ready";
            return Ok((root, state));
        }
        Err(anyhow!(
            "no matching deployment in this project's recent history; verify --commit/--deployment-id and the selected environment"
        ))
    }
}
