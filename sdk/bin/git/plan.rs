use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::app::App;
use crate::deployment_state::{Check, DeploymentState, StateFlags, TargetSpec};
use crate::git::{GitRepo, Source};
use crate::platform::{
    Platform, PublishTarget, commit_message, ensure_dirty_scope, verify_remote_origin,
};
use crate::stage::{StagedFile, manifest_path_in, write_manifest, write_source_tree};

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    DryRun,
    Stage,
    GitTransport,
}

impl Mode {
    pub fn stages_files(self) -> bool {
        matches!(self, Mode::Stage | Mode::GitTransport)
    }

    pub fn pushes(self) -> bool {
        matches!(self, Mode::GitTransport)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub struct Deployment {
    pub mode: Mode,
    pub platform: Platform,
    pub app: App,
    pub source: Source,
    pub publish: PublishTarget,
    #[serde(default)]
    pub files: Vec<StagedFile>,
}

impl Deployment {
    pub fn dry_run(start: impl AsRef<Path>, platform: Platform, allow_dirty: bool) -> Result<Self> {
        let repo = GitRepo::discover(start)?;
        let app = App::discover(&repo)?;
        let source = repo.snapshot(&app.source_path, allow_dirty)?;
        Self::build(Mode::DryRun, platform, app, source, Vec::new())
    }

    pub fn stage(
        start: impl AsRef<Path>,
        platform: Platform,
        stage_root: impl AsRef<Path>,
    ) -> Result<DeployOutcome> {
        let repo = GitRepo::discover(start)?;
        let app = App::discover(&repo)?;
        let source = repo.snapshot(&app.source_path, false)?;
        let mut deployment = Self::build(Mode::Stage, platform, app, source, Vec::new())?;

        let stage_root = stage_root.as_ref();
        let stage_root = stage_root
            .canonicalize()
            .unwrap_or_else(|_| stage_root.to_path_buf());
        let app_dir = stage_root.join(&deployment.publish.app_path);
        guard_overlap(&app_dir, &deployment.source)?;

        deployment.files = write_source_tree(&repo, &deployment.source.source_path, &app_dir)?;
        let manifest_path = manifest_path_in(&app_dir);
        write_manifest(&manifest_path, &deployment)?;

        Ok(DeployOutcome {
            deployment,
            app_dir,
            manifest_path,
            platform_repo_root: None,
            branch: None,
            commit: None,
            pushed: false,
        })
    }

    pub fn git_transport(
        start: impl AsRef<Path>,
        platform: Platform,
        platform_repo_root: impl AsRef<Path>,
        push: bool,
    ) -> Result<DeployOutcome> {
        let repo = GitRepo::discover(start)?;
        let app = App::discover(&repo)?;
        let source = repo.snapshot(&app.source_path, false)?;
        let mut deployment = Self::build(Mode::GitTransport, platform, app, source, Vec::new())?;

        let platform_repo = GitRepo::discover(platform_repo_root.as_ref())?;
        verify_remote_origin(&platform_repo, &deployment.publish.source_repo)?;
        ensure_dirty_scope(&platform_repo, &deployment.publish.app_path)?;
        platform_repo.checkout_or_create_branch(&deployment.publish.publish_branch)?;
        ensure_dirty_scope(&platform_repo, &deployment.publish.app_path)?;

        let app_dir = platform_repo.root().join(&deployment.publish.app_path);
        guard_overlap(&app_dir, &deployment.source)?;

        deployment.files = write_source_tree(&repo, &deployment.source.source_path, &app_dir)?;
        let manifest_path = manifest_path_in(&app_dir);
        write_manifest(&manifest_path, &deployment)?;

        platform_repo.add_path(&deployment.publish.app_path)?;
        let commit = if platform_repo.has_staged_changes_under(&deployment.publish.app_path)? {
            let (subject, body) = commit_message(&deployment);
            Some(platform_repo.commit_with_message(&subject, &body)?)
        } else {
            None
        };
        let pushed = if push && commit.is_some() {
            platform_repo.push_branch(&deployment.publish.publish_branch)?;
            true
        } else {
            false
        };

        Ok(DeployOutcome {
            platform_repo_root: Some(platform_repo.root().to_path_buf()),
            branch: Some(platform_repo.current_branch()?),
            commit,
            pushed,
            deployment,
            app_dir,
            manifest_path,
        })
    }

    fn build(
        mode: Mode,
        platform: Platform,
        app: App,
        source: Source,
        files: Vec<StagedFile>,
    ) -> Result<Self> {
        let publish = PublishTarget::resolve(&platform, &app, &source)?;
        Ok(Self {
            mode,
            platform,
            app,
            source,
            publish,
            files,
        })
    }

    /// Build the `.aomi/deployment.json` artifact from this in-memory plan.
    ///
    /// `aomi.toml`'s `[app].branch` (if present) takes precedence as the
    /// user's chosen target branch; otherwise we default to the platform's
    /// `publish_branch` from policy. Preflight may later overwrite
    /// `platform.resolved_deploy_branch` and `state.deployed` once it has
    /// fetched the DB-backed contract.
    pub fn to_state(&self) -> DeploymentState {
        let user_branch = self
            .app
            .branch
            .clone()
            .unwrap_or_else(|| self.publish.publish_branch.clone());
        let target = TargetSpec {
            branch: user_branch,
            app_path: self.publish.app_path.clone(),
            release_tag: self.publish.release_tag.clone(),
            server_tags: self.app.server_tags.clone(),
        };
        let mut state = DeploymentState::new(self.app.clone(), self.source.clone(), target);

        // Offline checks: things we can verify without network.
        state.checks.push(Check {
            name: "git_clean".to_string(),
            passed: !self.source.dirty,
            detail: if self.source.dirty {
                Some("git tree is dirty".to_string())
            } else {
                None
            },
        });
        state.checks.push(if self.app.platform.is_some() {
            Check::pass("platform_declared", self.app.platform.clone().unwrap())
        } else {
            Check::fail(
                "platform_declared",
                "aomi.toml is missing [app].platform — preflight cannot resolve target",
            )
        });
        state.checks.push(if self.app.git.is_some() {
            Check::pass("git_declared", self.app.git.clone().unwrap())
        } else {
            Check::fail(
                "git_declared",
                "aomi.toml is missing [app].git — preflight cannot verify push access",
            )
        });

        // State flags start false; preflight + deploy flip them.
        state.state = StateFlags::default();
        state
    }

    pub fn render(&self) -> String {
        format!(
            "\
Publish plan ({mode:?})
  platform        : {platform}
  app_slug        : {slug}
  display_name    : {display_name}
  app_config      : {config_path}
  git_root        : {git_root}
  source_path     : {source_path}
  source_branch   : {branch}
  source_commit   : {commit}
  source_tree     : {tree}
  source_digest   : {digest}
  dirty           : {dirty}
  publish_repo    : {source_repo}
  publish_branch  : {publish_branch}
  publish_path    : {app_path}
  release_tag     : {release_tag}
  server_tags     : {server_tags}
  stages_files    : {stages_files}
  pushes          : {pushes}
  staged_files    : {file_count}",
            mode = self.mode,
            platform = self.platform,
            slug = self.app.name,
            display_name = self.app.display_name,
            config_path = self.app.config_path.display(),
            git_root = self.source.git_root.display(),
            source_path = self.source.source_path.display(),
            branch = self.source.branch,
            commit = self.source.commit,
            tree = self.source.tree,
            digest = self.source.digest,
            dirty = self.source.dirty,
            source_repo = self.publish.source_repo,
            publish_branch = self.publish.publish_branch,
            app_path = self.publish.app_path,
            release_tag = self.publish.release_tag,
            server_tags = if self.app.server_tags.is_empty() {
                "<none>".to_string()
            } else {
                self.app.server_tags.join(",")
            },
            stages_files = self.mode.stages_files(),
            pushes = self.mode.pushes(),
            file_count = self.files.len(),
        )
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct DeployOutcome {
    pub deployment: Deployment,
    pub app_dir: PathBuf,
    pub manifest_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_repo_root: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub pushed: bool,
}

impl DeployOutcome {
    pub fn render(&self) -> String {
        let platform_repo_root = self
            .platform_repo_root
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "(n/a)".to_string());
        let branch = self.branch.as_deref().unwrap_or("(n/a)");
        let commit = self.commit.as_deref().unwrap_or("(no changes)");
        format!(
            "{}\n  staged_app_dir      : {}\n  manifest_path       : {}\n  platform_repo_root  : {}\n  publish_branch      : {}\n  publish_commit      : {}\n  pushed              : {}",
            self.deployment.render(),
            self.app_dir.display(),
            self.manifest_path.display(),
            platform_repo_root,
            branch,
            commit,
            self.pushed
        )
    }
}

fn guard_overlap(app_dir: &Path, source: &Source) -> Result<()> {
    if app_dir == source.git_root.join(&source.source_path) {
        anyhow::bail!("refusing to stage over the source app directory");
    }
    Ok(())
}

pub(crate) fn short_hash(commit: &str) -> String {
    commit.chars().take(12).collect()
}
