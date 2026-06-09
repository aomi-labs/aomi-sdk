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
use super::types::{ActivateRequest, ActivateResponse, LocalRecord, SourceRef, TargetRef};

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
        crate::Cmd::Deploy(args) => {
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
                { "name": "bot", "path": "apps/1/bot", "aomi_toml_path": "aomi.toml", "release_tag": "apps-1-bot-abc1234", "activated": false },
                { "name": "bot2", "path": "apps/1/bot2", "aomi_toml_path": "apps/b2/aomi.toml", "release_tag": "apps-1-bot2-abc1234", "activated": false }
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
    assert_eq!(state.release_tag_for("bot"), Some("apps-1-bot-abc1234"));
    assert_eq!(state.release_tag_for("bot2"), Some("apps-1-bot2-abc1234"));
    assert_eq!(state.release_tag_for("nope"), None);
}

#[test]
fn activate_request_serializes_target_based_body() {
    let req = ActivateRequest {
        target: TargetRef::PlatformPr {
            value: "https://github.com/aomi-labs/krexa-apps/pull/9".into(),
        },
        apps: vec!["bot".into(), "bot2".into()],
        release_tags: vec![],
        target_tags: vec!["staging".into()],
    };
    assert_eq!(
        serde_json::to_value(&req).unwrap(),
        json!({
            "target": { "kind": "platform_pr", "value": "https://github.com/aomi-labs/krexa-apps/pull/9" },
            "apps": ["bot", "bot2"],
            "target_tags": ["staging"]
        })
    );
    // Empty apps/release_tags/target_tags are omitted; commit target carries tags.
    let commit = ActivateRequest {
        target: TargetRef::PlatformCommit {
            value: "def5678".into(),
        },
        apps: vec!["bot".into()],
        release_tags: vec!["apps-1-bot-abc1234".into()],
        target_tags: vec![],
    };
    assert_eq!(
        serde_json::to_value(&commit).unwrap(),
        json!({
            "target": { "kind": "platform_commit", "value": "def5678" },
            "apps": ["bot"],
            "release_tags": ["apps-1-bot-abc1234"]
        })
    );

    let release_tags = ActivateRequest {
        target: TargetRef::ReleaseTags {
            value: vec!["apps-1-bot-abc1234".into()],
        },
        apps: vec![],
        release_tags: vec![],
        target_tags: vec![],
    };
    assert_eq!(
        serde_json::to_value(&release_tags).unwrap(),
        json!({
            "target": { "kind": "release_tags", "value": ["apps-1-bot-abc1234"] }
        })
    );
}

/// A multi-app activation response, as the target-based endpoint returns it.
fn activation(ci_status: &str, apps: &[(&str, bool, bool)]) -> ActivateResponse {
    let app_values: Vec<_> = apps
        .iter()
        .map(|(name, is_active, loaded)| {
            json!({
                "name": name,
                "path": format!("apps/1/{name}"),
                "release_tag": format!("apps-{name}-abc1234"),
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
                "kind": "platform_pr",
                "value": "https://github.com/aomi-labs/krexa-apps/pull/9",
                "platform_repo": "aomi-labs/krexa-apps",
                "ci_status": ci_status,
                "ci_url": "https://github.com/aomi-labs/krexa-apps/actions/runs/1"
            },
            "apps": app_values
        }
    }))
    .unwrap()
}

fn release_tag_activation(apps: &[(&str, bool, bool)]) -> ActivateResponse {
    let app_values: Vec<_> = apps
        .iter()
        .map(|(name, is_active, loaded)| {
            json!({
                "name": name,
                "path": format!("apps/1/{name}"),
                "release_tag": format!("apps-1-{name}-abc1234"),
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
                "value": apps.iter().map(|(name, _, _)| format!("apps-1-{name}-abc1234")).collect::<Vec<_>>(),
                "platform_repo": "aomi-labs/krexa-apps",
                "platform_branch": "publish",
                "promoted": apps.iter().map(|(name, _, _)| json!({
                    "name": name,
                    "release_tag": format!("apps-1-{name}-abc1234"),
                    "source_branch": "a/b/1/abc1234",
                    "platform_commit_hash": "def5678",
                    "ci_status": "passed",
                    "ci_url": "https://github.com/aomi-labs/krexa-apps/actions/runs/1",
                    "release_assets": [
                        format!("aomi-plugins-apps-1-{name}-abc1234-x86_64-unknown-linux-gnu.tar.gz"),
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
    state.apply_target_activation(&activation("passed", &[("bot", true, true)]));
    assert_eq!(state.deployment.platform.apps[0].activated, Some(true));
    assert!(
        state.state.ci_passed,
        "ci_status=passed mirrors into ci_passed"
    );
    assert!(!state.state.activated, "bot2 still inactive");
    assert_eq!(
        state
            .last_activation
            .as_ref()
            .map(|a| a.target.kind.as_str()),
        Some("platform_pr")
    );

    // Second call activates `bot2`; now all apps are active.
    state.apply_target_activation(&activation("passed", &[("bot2", true, true)]));
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
    assert_eq!(last.target.promoted[0].release_tag, "apps-1-bot-abc1234");
}

#[test]
fn apply_target_activation_keeps_failed_app_inactive() {
    let mut state = sample_state();
    state.apply_target_activation(&activation(
        "passed",
        &[("bot", true, true), ("bot2", false, false)],
    ));
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
    state.apply_target_activation(&activation("passed", &[("bot", true, false)]));
    assert_eq!(
        state.deployment.platform.apps[0].activated,
        Some(false),
        "hot-reload failure should not persist as a usable activation"
    );
    assert!(!state.state.activated);
}

#[test]
fn infer_target_picks_kind_from_value() {
    use super::cli::infer_target;
    assert!(matches!(
        infer_target("https://github.com/o/r/pull/9"),
        TargetRef::PlatformPr { .. }
    ));
    assert!(matches!(
        infer_target("def5678"),
        TargetRef::PlatformCommit { .. }
    ));
    assert!(matches!(
        infer_target("feature/login"),
        TargetRef::PlatformBranch { .. }
    ));
}

fn activate_args() -> ActivateArgs {
    ActivateArgs {
        apps: Vec::new(),
        platform: None,
        target: None,
        pr: None,
        branch: None,
        commit: None,
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
fn activation_target_supports_explicit_target_flags() {
    let state = sample_state();

    let mut pr = activate_args();
    pr.pr = Some("https://github.com/o/r/pull/9".into());
    assert!(matches!(
        pr.activation_target(&state, &[]).unwrap(),
        TargetRef::PlatformPr { .. }
    ));

    let mut branch = activate_args();
    branch.branch = Some("alice/repo/12345678/abc1234def56".into());
    assert!(matches!(
        branch.activation_target(&state, &[]).unwrap(),
        TargetRef::PlatformBranch { .. }
    ));

    let mut commit = activate_args();
    commit.commit = Some("abc1234".into());
    assert!(matches!(
        commit.activation_target(&state, &[]).unwrap(),
        TargetRef::PlatformCommit { .. }
    ));

    let release = activate_args();
    assert_eq!(
        release
            .activation_target(&state, &["apps-123-demo-abc1234".to_string()])
            .unwrap(),
        TargetRef::ReleaseTags {
            value: vec!["apps-123-demo-abc1234".to_string()]
        }
    );
}

#[test]
fn activation_target_rejects_ambiguous_targets() {
    let state = sample_state();
    let mut args = activate_args();
    args.target = Some("abc1234".into());
    args.pr = Some("https://github.com/o/r/pull/9".into());
    assert!(args.activation_target(&state, &[]).is_err());
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
