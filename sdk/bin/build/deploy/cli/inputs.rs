//! How a deploy step reads its inputs from the working tree.
//!
//! Everything here answers "what exactly are we deploying?" from git, the root
//! Project configuration: the source commit and manifest set. The backend
//! resolves the destination platform and Project id. Split from the lifecycle
//! in `deploy.rs`, which consumes the answers.

use std::path::Path;

use anyhow::{Result, bail};

use super::DeployStepArgs;
use super::shared::{head_commit, remote_origin};
use crate::deploy::platform::normalize_github_repo;
use crate::deploy::project_config::ProjectConfig;
use crate::deploy::types::BuildDeployInput;

impl DeployStepArgs {
    pub(super) fn build_request(&self, git_root: &Path) -> Result<BuildDeployInput> {
        let repo = match self.repo.as_deref() {
            Some(repo) => normalize_github_repo(repo)?,
            None => normalize_github_repo(&remote_origin(git_root)?)?,
        };
        ProjectConfig::load(git_root)?;
        Ok(BuildDeployInput {
            source_ref: self.source_ref(git_root)?,
            project_id: None,
            repo,
        })
    }

    pub(crate) fn source_ref(&self, git_root: &Path) -> Result<String> {
        if let Some(commit) = self
            .commit
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
        {
            return validate_source_commit(commit);
        }
        validate_source_commit(&head_commit(git_root)?)
    }
}

fn validate_source_commit(value: &str) -> Result<String> {
    let commit = value.trim().to_ascii_lowercase();
    if (7..=40).contains(&commit.len()) && commit.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(commit)
    } else {
        bail!("source commit must be a git commit SHA (7-40 hex chars), got `{value}`")
    }
}
