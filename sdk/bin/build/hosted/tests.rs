//! Integration-style tests for the repo-scoped relay CLI. Contract round-trips
//! live in `types.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use serde_json::json;
use tempfile::TempDir;

use super::cli::{ActivateArgs, DeployArgs};
use super::platform::Platform;
use super::types::{
    ActivateInput, ActivateResult, DeployInput, DeployResult, LocalDeployment, ReleaseTags,
    SourceRef,
};

// ── deploy: arg parsing ─────────────────────────────────────────────────────

#[test]
fn deploy_rejects_branch_and_commit_together() {
    let parsed = crate::Cli::try_parse_from([
        "aomi-build",
        "deploy",
        "--branch",
        "main",
        "--commit",
        "abc1234",
    ]);
    assert!(parsed.is_err(), "--branch and --commit must conflict");
}

#[test]
fn deploy_parses_repeated_aomi_toml() {
    let cli = crate::Cli::try_parse_from([
        "aomi-build",
        "deploy",
        "--aomi-toml",
        "apps/a/aomi.toml",
        "--aomi-toml",
        "apps/b/aomi.toml",
    ])
    .expect("parse");
    match cli.cmd {
        Some(crate::Cmd::Deploy(args)) => {
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

#[test]
fn deploy_input_serializes_widget_contract_body() {
    let input = DeployInput {
        app_source_id: 42,
        source_ref: SourceRef::branch("release"),
        aomi_toml_paths: vec!["aomi.toml".into(), "apps/bot/aomi.toml".into()],
        dry_run: true,
    };

    assert_eq!(
        serde_json::to_value(&input).unwrap(),
        json!({
            "app_source_id": 42,
            "source_ref": { "kind": "branch", "value": "release" },
            "aomi_toml_paths": ["aomi.toml", "apps/bot/aomi.toml"],
            "dry_run": true
        })
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

fn sample_state() -> LocalDeployment {
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
                { "name": "bot", "path": "apps/1/r00a1b2c3d4/bot", "aomi_toml_path": "aomi.toml", "release_tag": "apps-1-r00a1b2c3d4-bot-abc1234", "target": "x86_64-unknown-linux-gnu", "activated": false },
                { "name": "bot2", "path": "apps/1/r00a1b2c3d4/bot2", "aomi_toml_path": "apps/b2/aomi.toml", "release_tag": "apps-1-r00a1b2c3d4-bot2-abc1234", "target": "x86_64-unknown-linux-gnu", "activated": false }
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
    assert_eq!(
        state.release_tag_for("bot"),
        Some("apps-1-r00a1b2c3d4-bot-abc1234")
    );
    assert_eq!(
        state.release_tag_for("bot2"),
        Some("apps-1-r00a1b2c3d4-bot2-abc1234")
    );
    assert_eq!(state.release_tag_for("nope"), None);
}

#[test]
fn activate_request_serializes_target_based_body() {
    let req = ActivateInput {
        target: ReleaseTags::new(vec!["apps-1-r00a1b2c3d4-bot-abc1234".into()]),
        apps: vec!["bot".into()],
        target_tags: vec!["staging".into()],
    };
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        json!({
            "target": { "kind": "release_tags", "value": ["apps-1-r00a1b2c3d4-bot-abc1234"] },
            "apps": ["bot"],
            "target_tags": ["staging"]
        })
    );

    let release_tags = ActivateInput {
        target: ReleaseTags::new(vec!["apps-1-r00a1b2c3d4-bot-abc1234".into()]),
        apps: vec![],
        target_tags: vec![],
    };
    assert_eq!(
        serde_json::to_value(&release_tags).unwrap(),
        json!({
            "target": { "kind": "release_tags", "value": ["apps-1-r00a1b2c3d4-bot-abc1234"] }
        })
    );
}

fn release_tag_activation(apps: &[(&str, bool, bool)]) -> ActivateResult {
    let app_values: Vec<_> = apps
        .iter()
        .map(|(name, is_active, loaded)| {
            json!({
                "name": name,
                "path": format!("apps/1/r00a1b2c3d4/{name}"),
                "release_tag": format!("apps-1-r00a1b2c3d4-{name}-abc1234"),
                "is_active": is_active,
                "loaded": loaded,
                "error": if *loaded { serde_json::Value::Null } else { json!("post-activation hot-reload failed") }
            })
        })
        .collect();
    serde_json::from_value(json!({
        "ok": apps.iter().all(|(_, _, loaded)| *loaded),
        "activation": {
            "status": "activated",
            "platform": "krexa",
            "target": {
                "kind": "release_tags",
                "value": apps.iter().map(|(name, _, _)| format!("apps-1-r00a1b2c3d4-{name}-abc1234")).collect::<Vec<_>>(),
                "platform_repo": "aomi-labs/krexa-apps",
                "platform_branch": "publish",
                "promoted": apps.iter().map(|(name, _, _)| json!({
                    "name": name,
                    "release_tag": format!("apps-1-r00a1b2c3d4-{name}-abc1234"),
                    "source_branch": "a/b/1/abc1234",
                    "platform_commit_hash": "def5678",
                    "live_commit_hash": "fed7654",
                    "ci_status": "passed",
                    "ci_url": "https://github.com/aomi-labs/krexa-apps/actions/runs/1",
                    "release_assets": [
                        format!("aomi-plugins-apps-1-r00a1b2c3d4-{name}-abc1234-x86_64-unknown-linux-gnu.tar.gz"),
                        "manifest.json",
                        "aomi-release.json"
                    ]
                })).collect::<Vec<_>>()
            },
            "apps": app_values
        }
    }))
    .unwrap()
}

#[test]
fn apply_target_activation_marks_apps_ci_and_overall_state() {
    let mut state = sample_state();
    // First call activates only `bot`; CI passed should flip `ci_passed`.
    state.apply_target_activation(&release_tag_activation(&[("bot", true, true)]));
    assert_eq!(state.deployment.platform.apps[0].activated, Some(true));
    assert!(
        state.state.ci_passed,
        "release-tag promotions mirror passed CI into ci_passed"
    );
    assert!(!state.state.activated, "bot2 still inactive");
    assert_eq!(
        state
            .last_activation
            .as_ref()
            .map(|a| a.target.kind.as_str()),
        Some("release_tags")
    );

    // Second call activates `bot2`; now all apps are active.
    state.apply_target_activation(&release_tag_activation(&[("bot2", true, true)]));
    assert!(state.state.activated, "all apps active");
}

#[test]
fn release_tag_activation_promotions_mark_ci_and_sync_last_activation() {
    let mut state = sample_state();
    let response = release_tag_activation(&[("bot", true, true), ("bot2", true, true)]);

    state.apply_target_activation(&response);

    assert!(state.state.ci_passed, "promoted releases carry passed CI");
    assert!(state.state.activated, "all promoted apps are active");
    let last = state.last_activation.as_ref().expect("last activation");
    assert_eq!(last.target.kind, "release_tags");
    assert_eq!(last.target.promoted.len(), 2);
    assert_eq!(
        last.target.promoted[0].release_tag,
        "apps-1-r00a1b2c3d4-bot-abc1234"
    );
    assert_eq!(
        last.target.promoted[0].live_commit_hash.as_deref(),
        Some("fed7654")
    );
}

#[test]
fn apply_target_activation_keeps_failed_app_inactive() {
    let mut state = sample_state();
    state.apply_target_activation(&release_tag_activation(&[
        ("bot", true, true),
        ("bot2", false, false),
    ]));
    assert_eq!(state.deployment.platform.apps[0].activated, Some(true));
    assert_eq!(state.deployment.platform.apps[1].activated, Some(false));
    assert!(
        !state.state.activated,
        "a failed app blocks overall activation"
    );
}

#[test]
fn apply_target_activation_keeps_unloaded_active_row_inactive() {
    let mut state = sample_state();
    state.apply_target_activation(&release_tag_activation(&[("bot", true, false)]));
    assert_eq!(
        state.deployment.platform.apps[0].activated,
        Some(false),
        "hot-reload failure should not persist as a usable activation"
    );
    assert!(!state.state.activated);
}

fn activate_args() -> ActivateArgs {
    ActivateArgs {
        apps: Vec::new(),
        platform: None,
        release_tags: Vec::new(),
        backend: None,
        activation_token: None,
        target_tags: Vec::new(),
        path: ".".into(),
        dry_run: false,
        json: false,
    }
}

#[test]
fn activation_request_defaults_to_deployment_release_tags() {
    let state = sample_state();
    let request = activate_args().activation_request(&state).unwrap();
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({
            "target": {
                "kind": "release_tags",
                "value": ["apps-1-r00a1b2c3d4-bot-abc1234", "apps-1-r00a1b2c3d4-bot2-abc1234"]
            },
            "apps": ["bot", "bot2"]
        })
    );

    let mut subset = activate_args();
    subset.apps = vec!["bot".into()];
    subset.target_tags = vec!["staging".into()];
    let request = subset.activation_request(&state).unwrap();
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({
            "target": { "kind": "release_tags", "value": ["apps-1-r00a1b2c3d4-bot-abc1234"] },
            "apps": ["bot"],
            "target_tags": ["staging"]
        })
    );
}

#[test]
fn activation_request_supports_explicit_release_tags() {
    let state = sample_state();

    let mut release = activate_args();
    release.release_tags = vec!["apps-123-demo-abc1234".into()];
    let request = release.activation_request(&state).unwrap();
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({
            "target": { "kind": "release_tags", "value": ["apps-123-demo-abc1234"] }
        })
    );

    let mut mismatch = activate_args();
    mismatch.apps = vec!["bot".into(), "bot2".into()];
    mismatch.release_tags = vec!["apps-123-demo-abc1234".into()];
    assert!(mismatch.activation_request(&state).is_err());
}

// ── deployment.json round-trip via the state file ───────────────────────────

#[test]
fn local_deployment_write_then_read_round_trips() {
    let repo = TestRepo::new();
    let state = sample_state();
    state.write(repo.root()).unwrap();
    let read = LocalDeployment::read(repo.root()).unwrap().unwrap();
    assert_eq!(read, state);
    // a repo with no .aomi/deployment.json reads as None, not an error
    let empty = TestRepo::new();
    assert!(LocalDeployment::read(empty.root()).unwrap().is_none());
}

#[test]
fn deploy_records_app_source_id_and_round_trips() {
    // The backend deploy response omits app_source_id; the CLI stamps the id it
    // deployed from so re-deploys / activate can auto-resolve it.
    let resp: DeployResult = serde_json::from_value(json!({
        "ok": true,
        "deployment": {
            "id": "dep_1",
            "status": "pr_created",
            "source": {
                "installation_id": 1, "repository_id": 2,
                "repository_link": "https://github.com/a/b.git",
                "ref": { "kind": "branch", "value": "main" },
                "commit_hash": "abc1234", "aomi_toml_paths": ["aomi.toml"]
            },
            "platform": {
                "platform": "playground", "repository": "aomi-labs/aomi-playground",
                "source_branch": "main", "deploy_branch": "main",
                "apps": [
                    { "name": "bot", "path": "apps/1/r0/bot", "aomi_toml_path": "aomi.toml", "release_tag": "apps-1-r0-bot-abc1234" }
                ]
            }
        }
    }))
    .unwrap();

    let state = LocalDeployment::from_deploy(resp, 219);
    assert_eq!(state.app_source_id(), Some(219));

    // Persists into .aomi/deployment.json and reads back intact.
    let repo = TestRepo::new();
    state.write(repo.root()).unwrap();
    assert_eq!(
        LocalDeployment::read(repo.root())
            .unwrap()
            .unwrap()
            .app_source_id(),
        Some(219)
    );

    // `source sync` / `scaffold` patch path: set + persist on an existing record.
    let mut patched = LocalDeployment::read(repo.root()).unwrap().unwrap();
    patched.set_app_source_id(321);
    patched.write(repo.root()).unwrap();
    assert_eq!(
        LocalDeployment::read(repo.root())
            .unwrap()
            .unwrap()
            .app_source_id(),
        Some(321)
    );
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
