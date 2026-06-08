//! Integration-style tests for the repo-scoped relay CLI. Contract round-trips
//! live in `types.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use serde_json::json;
use tempfile::TempDir;

use crate::cli::{Cli, DeployArgs};
use crate::platform::Platform;
use crate::types::{ActivateRequest, ActivateResponse, LocalRecord, SourceRef};

// ── deploy: arg parsing ─────────────────────────────────────────────────────

#[test]
fn deploy_rejects_branch_and_commit_together() {
    let parsed = Cli::try_parse_from([
        "aomi-git", "deploy", "--branch", "main", "--commit", "abc1234",
    ]);
    assert!(parsed.is_err(), "--branch and --commit must conflict");
}

#[test]
fn deploy_parses_repeated_aomi_toml() {
    let cli = Cli::try_parse_from([
        "aomi-git",
        "deploy",
        "--aomi-toml",
        "apps/a/aomi.toml",
        "--aomi-toml",
        "apps/b/aomi.toml",
    ])
    .expect("parse");
    match cli.command {
        crate::cli::Command::Deploy(args) => {
            assert_eq!(args.aomi_toml.len(), 2);
        }
        _ => panic!("expected deploy"),
    }
}

// ── deploy: source ref resolution ───────────────────────────────────────────

#[test]
fn source_ref_defaults_to_head_commit() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("", "root");
    repo.commit("init");
    let head = repo.head();

    let args = deploy_args(repo.root());
    assert_eq!(
        args.source_ref(repo.root()).unwrap(),
        SourceRef::commit(head)
    );
}

#[test]
fn source_ref_honors_branch_and_commit_flags() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("", "root");
    repo.commit("init");

    let mut args = deploy_args(repo.root());
    args.branch = Some("release".into());
    assert_eq!(
        args.source_ref(repo.root()).unwrap(),
        SourceRef::branch("release")
    );

    let mut args = deploy_args(repo.root());
    args.commit = Some("0badc0de".into());
    assert_eq!(
        args.source_ref(repo.root()).unwrap(),
        SourceRef::commit("0badc0de")
    );
}

// ── deploy: aomi.toml discovery + normalization ─────────────────────────────

#[test]
fn aomi_toml_paths_default_to_all_tracked() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("", "root");
    repo.write_aomi_toml("apps/bot", "bot");
    repo.commit("init");

    let paths = deploy_args(repo.root())
        .aomi_toml_paths(repo.root())
        .unwrap();
    assert_eq!(paths, vec!["aomi.toml", "apps/bot/aomi.toml"]);
}

#[test]
fn aomi_toml_paths_normalize_explicit_and_reject_traversal() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("", "root");
    repo.commit("init");

    let mut args = deploy_args(repo.root());
    args.aomi_toml = vec!["./apps/bot/aomi.toml".into(), "apps/bot/aomi.toml".into()];
    let paths = args.aomi_toml_paths(repo.root()).unwrap();
    assert_eq!(paths, vec!["apps/bot/aomi.toml"]); // normalized + deduped

    let mut bad = deploy_args(repo.root());
    bad.aomi_toml = vec!["../escape/aomi.toml".into()];
    assert!(bad.aomi_toml_paths(repo.root()).is_err());
}

// ── deploy: platform resolution ─────────────────────────────────────────────

#[test]
fn platform_defaults_from_aomi_toml_then_community() {
    let repo = TestRepo::new();
    repo.write("aomi.toml", "[app]\nname = \"x\"\nplatform = \"krexa\"\n");
    repo.commit("init");
    assert_eq!(
        deploy_args(repo.root())
            .platform(repo.root(), repo.root())
            .as_str(),
        "krexa"
    );

    let bare = TestRepo::new();
    bare.write("README.md", "no aomi.toml\n");
    bare.commit("init");
    assert_eq!(
        deploy_args(bare.root()).platform(bare.root(), bare.root()),
        Platform::community()
    );
}

// ── activate: request building ──────────────────────────────────────────────

fn sample_state() -> LocalRecord {
    serde_json::from_value(json!({
        "id": "dep_1",
        "status": "pr_created",
        "source": {
            "installation_id": 1, "repository_id": 2,
            "repository_link": "https://github.com/a/b.git",
            "ref": { "kind": "branch", "value": "main" },
            "commit_hash": "abc1234", "aomi_toml_paths": ["aomi.toml", "apps/b2/aomi.toml"]
        },
        "platform": {
            "platform": "krexa", "repository": "aomi-labs/krexa-apps",
            "source_branch": "main", "deploy_branch": "deploy/1/abc1234",
            "commit_hash": "def5678", "pr_number": 9,
            "pr_url": "https://github.com/aomi-labs/krexa-apps/pull/9",
            "apps": [
                { "name": "bot", "path": "apps/1/bot", "aomi_toml_path": "aomi.toml", "release_tag": "apps-bot-abc1234", "activated": false },
                { "name": "bot2", "path": "apps/1/bot2", "aomi_toml_path": "apps/b2/aomi.toml", "release_tag": "apps-bot2-abc1234", "activated": false }
            ]
        },
        "state": { "deployed": true, "ci_passed": false, "activated": false }
    }))
    .unwrap()
}

#[test]
fn release_tag_and_app_names_from_state() {
    let state = sample_state();
    assert_eq!(state.app_names(), vec!["bot", "bot2"]);
    assert_eq!(state.release_tag_for("bot"), Some("apps-bot-abc1234"));
    assert_eq!(state.release_tag_for("bot2"), Some("apps-bot2-abc1234"));
    assert_eq!(state.release_tag_for("nope"), None);
}

#[test]
fn activate_request_serializes_single_app_body() {
    let req = ActivateRequest {
        app_release_tag: "apps-bot-abc1234".into(),
        target_tags: vec!["staging".into()],
    };
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        json!({ "app_release_tag": "apps-bot-abc1234", "target_tags": ["staging"] })
    );
    // empty target_tags is omitted
    let bare = ActivateRequest {
        app_release_tag: "t".into(),
        target_tags: vec![],
    };
    assert_eq!(
        serde_json::to_value(&bare).unwrap(),
        json!({ "app_release_tag": "t" })
    );
}

fn activation(name: &str) -> ActivateResponse {
    ActivateResponse {
        id: 1,
        name: name.to_string(),
        label: name.to_string(),
        platform: "krexa".into(),
        is_active: true,
        is_public: false,
        app_source_id: Some(1),
        app_release_tag: Some(format!("apps-{name}-abc1234")),
        status: "activated".into(),
    }
}

#[test]
fn apply_activation_marks_app_then_overall_state() {
    let mut state = sample_state();
    state.apply_activation(&activation("bot"));
    assert_eq!(state.deployment.platform.apps[0].activated, Some(true));
    assert!(!state.state.activated, "bot2 still inactive");

    state.apply_activation(&activation("bot2"));
    assert!(state.state.activated, "all apps active");
}

// ── deployment.json round-trip via the state file ───────────────────────────

#[test]
fn local_deployment_write_then_read_round_trips() {
    let repo = TestRepo::new();
    let state = sample_state();
    state.write(repo.root()).unwrap();
    let read = LocalRecord::read(repo.root()).unwrap().unwrap();
    assert_eq!(read, state);
    // a repo with no .aomi/deployment.json reads as None, not an error
    let empty = TestRepo::new();
    assert!(LocalRecord::read(empty.root()).unwrap().is_none());
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn deploy_args(path: &Path) -> DeployArgs {
    DeployArgs {
        platform: None,
        app_source_id: None,
        branch: None,
        commit: None,
        aomi_toml: vec![],
        backend: None,
        path: path.to_path_buf(),
        dry_run: false,
        json: false,
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

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}
