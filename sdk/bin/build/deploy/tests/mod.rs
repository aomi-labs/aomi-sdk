//! Tests for the repo-scoped relay CLI, split by what they pin.
//!
//! Shared fixtures (`TestRepo`, `sample_state`, …) live here; each submodule
//! does `use super::*` to reach them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use serde_json::json;
use tempfile::TempDir;

use super::cli::{ActivateArgs, DeployStepArgs, StatusArgs};
use super::platform::Platform;
use super::state::LocalDeployment;
use super::types::{
    ActivateInput, ActivateResult, BuildDeployInput, DeployInput, DeployResult, ReleaseTags,
};

/// A two-app deployment record, as the backend would have produced it.
pub(super) fn sample_state() -> LocalDeployment {
    serde_json::from_value(json!({
        "id": "dep_1",
        "status": "pr_created",
        "source": {
            "installation_id": 1, "repository_id": 2,
            "repository_link": "https://github.com/a/b.git",
            "ref": "abc1234",
            "commit_hash": "abc1234", "aomi_toml_paths": ["aomi.toml", "apps/b2/aomi.toml"]
        },
        "platform": {
            "platform": "krexa", "repository": "aomi-labs/krexa-apps",
            "source_branch": "main", "deploy_branch": "deploy/1/abc1234",
            "commit_hash": "def5678", "pr_number": 9,
            "pr_url": "https://github.com/aomi-labs/krexa-apps/pull/9",
            "apps": [
                { "name": "bot", "path": "apps/1/r00a1b2c3d4/bot", "aomi_toml_path": "aomi.toml", "release_tag": "apps-1-r00a1b2c3d4-bot-abc1234", "target": "x86_64-unknown-linux-gnu", "activated": false },
                { "name": "bot2", "path": "apps/1/r00a1b2c3d4/bot2", "aomi_toml_path": "apps/b2/aomi.toml", "release_tag": "apps-1-r00a1b2c3d4-bot2-abc1234", "target": "x86_64-unknown-linux-gnu", "activated": false }
            ]
        },
        "state": { "deployed": true, "ci_passed": false, "activated": false }
    }))
    .unwrap()
}

pub(super) fn activate_args() -> ActivateArgs {
    ActivateArgs {
        path: ".".into(),
        ..Default::default()
    }
}

mod activate;
mod args;
mod inputs;
mod regressions;
mod state;

pub(super) fn deploy_args(path: &Path) -> DeployStepArgs {
    DeployStepArgs {
        path: path.to_path_buf(),
        ..Default::default()
    }
}

struct TestRepo {
    tmp: TempDir,
}

impl TestRepo {
    fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir");
        run_git(tmp.path(), ["init", "-q", "-b", "main"]);
        run_git(tmp.path(), ["config", "user.email", "t@example.test"]);
        run_git(tmp.path(), ["config", "user.name", "Test"]);
        Self { tmp }
    }

    fn root(&self) -> &Path {
        self.tmp.path()
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.tmp.path().join(relative)
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    fn write_aomi_toml(&self, dir: &str, name: &str) {
        let rel = if dir.is_empty() {
            "aomi.toml".to_string()
        } else {
            format!("{dir}/aomi.toml")
        };
        self.write(
            &rel,
            &format!("[app]\nname = \"{name}\"\nplatform = \"community\"\n"),
        );
    }

    fn commit(&self, message: &str) {
        run_git(self.root(), ["add", "."]);
        run_git(self.root(), ["commit", "-q", "-m", message]);
    }

    fn head(&self) -> String {
        String::from_utf8(
            Command::new("git")
                .current_dir(self.root())
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("rev-parse")
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string()
    }
}

pub(super) fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}
