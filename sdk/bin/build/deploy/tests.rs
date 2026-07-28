//! Integration-style tests for the repo-scoped relay CLI. Contract round-trips
//! live in `types.rs`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use serde_json::json;
use tempfile::TempDir;

use super::cli::{ActivateArgs, DeployStepArgs, StatusArgs};
use super::platform::Platform;
use super::types::{
    ActivateInput, ActivateResult, BuildDeployInput, DeployInput, DeployResult, LocalDeployment,
    ReleaseTags,
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
            assert_eq!(args.step.aomi_toml.len(), 2);
        }
        _ => panic!("expected deploy"),
    }
}

#[test]
fn deploy_subcommands_parse_lifecycle_steps() {
    let cli = crate::Cli::try_parse_from([
        "aomi-build",
        "deploy",
        "preflight",
        "--platform",
        "community",
        "--repo",
        "aomi-labs/playground-example",
    ])
    .expect("parse preflight");
    match cli.cmd {
        Some(crate::Cmd::Deploy(args)) => match args.cmd {
            Some(super::cli::deploy::DeployCmd::Preflight(step)) => {
                assert_eq!(step.platform.unwrap().as_str(), "community");
                assert_eq!(step.repo.as_deref(), Some("aomi-labs/playground-example"));
            }
            _ => panic!("expected deploy preflight"),
        },
        _ => panic!("expected deploy"),
    }

    let cli = crate::Cli::try_parse_from(["aomi-build", "deploy", "status", "--json"])
        .expect("parse status");
    match cli.cmd {
        Some(crate::Cmd::Deploy(args)) => match args.cmd {
            Some(super::cli::deploy::DeployCmd::Status(status)) => assert!(status.json),
            _ => panic!("expected deploy status"),
        },
        _ => panic!("expected deploy"),
    }
}

#[test]
fn deploy_prerequisite_flags_parse_on_lifecycle_steps() {
    let cli = crate::Cli::try_parse_from([
        "aomi-build",
        "deploy",
        "--backend",
        "https://api.aomi.dev",
        "--build-url",
        "https://build.aomi.dev",
        "--activation-token",
        "aat_live",
        "--app-source-id",
        "626",
    ])
    .expect("parse deploy flags");
    match cli.cmd {
        Some(crate::Cmd::Deploy(args)) => {
            assert_eq!(args.step.backend.as_deref(), Some("https://api.aomi.dev"));
            assert_eq!(
                args.step.build_url.as_deref(),
                Some("https://build.aomi.dev")
            );
            assert_eq!(args.step.activation_token.as_deref(), Some("aat_live"));
            assert_eq!(args.step.app_source_id, Some(626));
        }
        _ => panic!("expected deploy"),
    }

    let cli = crate::Cli::try_parse_from([
        "aomi-build",
        "deploy",
        "activate",
        "--backend",
        "https://api.aomi.dev",
        "--activation-token",
        "aat_live",
    ])
    .expect("parse deploy activate flags");
    match cli.cmd {
        Some(crate::Cmd::Deploy(args)) => match args.cmd {
            Some(super::cli::deploy::DeployCmd::Activate(activate)) => {
                assert_eq!(activate.backend.as_deref(), Some("https://api.aomi.dev"));
                assert_eq!(activate.activation_token.as_deref(), Some("aat_live"));
            }
            _ => panic!("expected deploy activate"),
        },
        _ => panic!("expected deploy"),
    }

    let cli = crate::Cli::try_parse_from([
        "aomi-build",
        "deploy",
        "status",
        "--backend",
        "https://api.aomi.dev",
        "--activation-token",
        "aat_live",
    ])
    .expect("parse deploy status flags");
    match cli.cmd {
        Some(crate::Cmd::Deploy(args)) => match args.cmd {
            Some(super::cli::deploy::DeployCmd::Status(status)) => {
                assert_eq!(status.backend.as_deref(), Some("https://api.aomi.dev"));
                assert_eq!(status.activation_token.as_deref(), Some("aat_live"));
            }
            _ => panic!("expected deploy status"),
        },
        _ => panic!("expected deploy"),
    }
}

#[test]
fn support_commands_parse_prerequisite_flags() {
    let cli = crate::Cli::try_parse_from([
        "aomi-build",
        "source",
        "sync",
        "--platform",
        "community",
        "--repo",
        "aomi-labs/playground-example",
        "--backend",
        "https://api.aomi.dev",
        "--activation-token",
        "aat_live",
    ])
    .expect("parse source flags");
    match cli.cmd {
        Some(crate::Cmd::Source(args)) => match args.cmd {
            super::cli::source::SourceCmd::Sync(sync) => {
                assert_eq!(sync.backend.as_deref(), Some("https://api.aomi.dev"));
                assert_eq!(sync.activation_token.as_deref(), Some("aat_live"));
            }
        },
        _ => panic!("expected source sync"),
    }

    let cli = crate::Cli::try_parse_from([
        "aomi-build",
        "token",
        "mint",
        "--platform",
        "community",
        "--backend",
        "https://api.aomi.dev",
        "--admin-key",
        "admin.pem",
        "--admin-kid",
        "aomi-admin-prod-1",
    ])
    .expect("parse token mint flags");
    match cli.cmd {
        Some(crate::Cmd::Token(args)) => match args.cmd {
            super::cli::token::TokenCmd::Mint(mint) => {
                assert_eq!(mint.backend.as_deref(), Some("https://api.aomi.dev"));
                assert_eq!(mint.admin_key.as_deref(), Some("admin.pem"));
                assert_eq!(mint.admin_kid.as_deref(), Some("aomi-admin-prod-1"));
            }
            _ => panic!("expected token mint"),
        },
        _ => panic!("expected token mint"),
    }
}

#[test]
fn missing_prerequisite_errors_print_flag_first_hints() {
    let backend = super::cli::shared::missing_backend("deploy").to_string();
    assert!(backend.contains("deploy --backend <url>"));
    assert!(backend.contains("export AOMI_BACKEND_URL=<url>"));

    let token = super::cli::shared::missing_activation_token("deploy activate").to_string();
    assert!(token.contains("deploy activate --activation-token <token>"));
    assert!(token.contains("export AOMI_APP_ACTIVATION_TOKEN=<token>"));

    let admin_key = super::cli::shared::missing_admin_key("token mint").to_string();
    assert!(admin_key.contains("token mint --admin-key <pkcs8-pem-or-path>"));
    assert!(admin_key.contains("export AOMI_ADMIN_KEY=<pkcs8-pem-or-path>"));
}

// ── wizard + connect: arg parsing ───────────────────────────────────────────

#[test]
fn no_subcommand_enters_wizard() {
    let cli = crate::Cli::try_parse_from(["aomi-build"]).expect("parse");
    assert!(
        cli.cmd.is_none(),
        "a bare invocation should enter the wizard"
    );
}

#[test]
fn login_parses_build_environment() {
    let cli = crate::Cli::try_parse_from([
        "aomi-build",
        "login",
        "--build-url",
        "https://build-staging.aomi.dev",
        "--no-browser",
    ])
    .expect("parse login");
    match cli.cmd {
        Some(crate::Cmd::Login(args)) => {
            assert_eq!(
                args.build_url.as_deref(),
                Some("https://build-staging.aomi.dev")
            );
            assert!(args.no_browser);
        }
        _ => panic!("expected login"),
    }
}

#[test]
fn build_deploy_input_uses_bff_camel_case_contract() {
    let request = BuildDeployInput {
        platform: "somm.finance".into(),
        repo: "peggyjv/somm-agent".into(),
        source_ref: "abc1234".into(),
        aomi_toml_paths: vec!["aomi.toml".into()],
        app_source_id: Some(1065),
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "platform": "somm.finance",
            "repo": "peggyjv/somm-agent",
            "sourceRef": "abc1234",
            "aomiTomlPaths": ["aomi.toml"],
            "appSourceId": 1065
        })
    );
}

#[test]
fn backend_environment_maps_to_matching_build_frontend() {
    assert_eq!(
        super::cli::shared::infer_build_url("https://api-staging.aomi.dev"),
        Some("https://build-staging.aomi.dev".into())
    );
    assert_eq!(
        super::cli::shared::infer_build_url("https://api.aomi.dev/"),
        Some("https://build.aomi.dev".into())
    );
}

#[test]
fn connect_parses_authorize_and_drops_polling_flags() {
    let cli = crate::Cli::try_parse_from([
        "aomi-build",
        "connect",
        "--platform",
        "community",
        "--authorize",
    ])
    .expect("parse");
    match cli.cmd {
        Some(crate::Cmd::Connect(args)) => assert!(args.authorize),
        _ => panic!("expected connect"),
    }

    // The phantom-poll era flags are gone — connect now captures the
    // installation id by paste, matching how the portal reads it off GitHub's
    // redirect (there is no result/poll endpoint).
    for legacy in [["--manual"].as_slice(), ["--timeout-secs", "10"].as_slice()] {
        let argv = [
            &["aomi-build", "connect", "--platform", "community"],
            legacy,
        ]
        .concat();
        assert!(
            crate::Cli::try_parse_from(argv).is_err(),
            "removed flag {legacy:?} should no longer parse"
        );
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
    assert_eq!(args.source_ref(repo.root()).unwrap(), head);
}

#[test]
fn source_ref_rejects_branch_and_honors_commit_flag() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("", "root");
    repo.commit("init");

    let mut args = deploy_args(repo.root());
    args.branch = Some("release".into());
    assert!(args.source_ref(repo.root()).is_err());

    let mut args = deploy_args(repo.root());
    args.commit = Some("0badc0de".into());
    assert_eq!(args.source_ref(repo.root()).unwrap(), "0badc0de");

    let mut args = deploy_args(repo.root());
    args.commit = Some("main".into());
    assert!(args.source_ref(repo.root()).is_err());
}

#[test]
fn deploy_input_serializes_widget_contract_body() {
    let input = DeployInput {
        app_source_id: 42,
        source_ref: "0badc0de".to_string(),
        aomi_toml_paths: vec!["aomi.toml".into(), "apps/bot/aomi.toml".into()],
        preflight: true,
    };

    assert_eq!(
        serde_json::to_value(&input).unwrap(),
        json!({
            "app_source_id": 42,
            "source_ref": "0badc0de",
            "aomi_toml_paths": ["aomi.toml", "apps/bot/aomi.toml"],
            "preflight": true
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
            .unwrap()
            .as_str(),
        "krexa"
    );

    let bare = TestRepo::new();
    bare.write("README.md", "no aomi.toml\n");
    bare.commit("init");
    let expected = super::config::AomiConfig::load()
        .platform
        .map(Platform::new)
        .unwrap_or_else(Platform::community);
    assert_eq!(
        deploy_args(bare.root())
            .platform(bare.root(), bare.root())
            .unwrap(),
        expected
    );
}

#[test]
fn platform_prefers_scoped_manifest_over_saved_config() {
    // Live repro: `deploy preflight --path . --aomi-toml apps/gecko/aomi.toml`
    // used the saved-config platform even though the deployed manifest
    // declared `krexa`, because resolution only walked up from --path.
    let repo = TestRepo::new();
    repo.write(
        "apps/gecko/aomi.toml",
        "[app]\nname = \"gecko\"\nplatform = \"krexa\"\n",
    );
    repo.commit("init");

    let mut args = deploy_args(repo.root());
    args.aomi_toml = vec!["apps/gecko/aomi.toml".into()];
    let platform = args
        .resolve_platform(repo.root(), repo.root(), Some("somm.finance".into()))
        .unwrap();
    assert_eq!(platform.as_str(), "krexa");

    // Same precedence when the manifest set is auto-discovered from tracking.
    let platform = deploy_args(repo.root())
        .resolve_platform(repo.root(), repo.root(), Some("somm.finance".into()))
        .unwrap();
    assert_eq!(platform.as_str(), "krexa");
}

#[test]
fn platform_flag_wins_over_manifest() {
    let repo = TestRepo::new();
    repo.write("aomi.toml", "[app]\nname = \"x\"\nplatform = \"krexa\"\n");
    repo.commit("init");

    let mut args = deploy_args(repo.root());
    args.platform = Some(Platform::new("somm.finance"));
    let platform = args
        .resolve_platform(repo.root(), repo.root(), Some("other".into()))
        .unwrap();
    assert_eq!(platform.as_str(), "somm.finance");
}

#[test]
fn platform_falls_back_to_saved_config_when_manifests_are_silent() {
    let repo = TestRepo::new();
    repo.write("aomi.toml", "[app]\nname = \"x\"\n");
    repo.commit("init");

    let platform = deploy_args(repo.root())
        .resolve_platform(repo.root(), repo.root(), Some("somm.finance".into()))
        .unwrap();
    assert_eq!(platform.as_str(), "somm.finance");

    let platform = deploy_args(repo.root())
        .resolve_platform(repo.root(), repo.root(), None)
        .unwrap();
    assert_eq!(platform, Platform::community());
}

#[test]
fn platform_conflicting_manifests_error_unless_scoped_or_flagged() {
    let repo = TestRepo::new();
    repo.write(
        "apps/a/aomi.toml",
        "[app]\nname = \"a\"\nplatform = \"krexa\"\n",
    );
    repo.write(
        "apps/b/aomi.toml",
        "[app]\nname = \"b\"\nplatform = \"somm.finance\"\n",
    );
    repo.commit("init");

    let err = deploy_args(repo.root())
        .resolve_platform(repo.root(), repo.root(), None)
        .unwrap_err()
        .to_string();
    assert!(err.contains("conflicting platforms"), "{err}");
    assert!(err.contains("apps/a/aomi.toml -> krexa"), "{err}");
    assert!(err.contains("apps/b/aomi.toml -> somm.finance"), "{err}");

    // Scoping the deploy to one manifest resolves the conflict…
    let mut scoped = deploy_args(repo.root());
    scoped.aomi_toml = vec!["apps/b/aomi.toml".into()];
    let platform = scoped
        .resolve_platform(repo.root(), repo.root(), None)
        .unwrap();
    assert_eq!(platform.as_str(), "somm.finance");

    // …and an explicit --platform overrides the manifests entirely.
    let mut flagged = deploy_args(repo.root());
    flagged.platform = Some(Platform::new("community"));
    let platform = flagged
        .resolve_platform(repo.root(), repo.root(), None)
        .unwrap();
    assert_eq!(platform, Platform::community());
}

// ── activate: request building ──────────────────────────────────────────────

fn sample_state() -> LocalDeployment {
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
                "artifact_ready": loaded,
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
fn activation_response_accepts_builder_camel_case() {
    let response: ActivateResult = serde_json::from_value(json!({
        "ok": true,
        "activation": {
            "status": "activated",
            "platform": "community",
            "target": {
                "kind": "release_tags",
                "value": ["apps-141779906-r2bf7fd9ccb-playground-example-6fe687c7d6e4"],
                "platformRepo": "aomi-labs/community-apps",
                "platformBranch": "publish",
                "promoted": [{
                    "name": "playground-example",
                    "releaseTag": "apps-141779906-r2bf7fd9ccb-playground-example-6fe687c7d6e4",
                    "sourceBranch": "ceciliaz030/playground-example-1/141779906/6fe687c7d6e4",
                    "platformCommitHash": "cfb6a6411712f1f65ce81d7373decd1d21be4ea1",
                    "liveCommitHash": "cfb6a6411712f1f65ce81d7373decd1d21be4ea1",
                    "ciStatus": "passed",
                    "ciUrl": "https://github.com/aomi-labs/community-apps/actions/runs/1",
                    "releaseAssets": ["manifest.json"]
                }]
            },
            "apps": [{
                "applicationId": 42,
                "name": "playground-example",
                "path": "apps/141779906/r2bf7fd9ccb/playground-example",
                "releaseTag": "apps-141779906-r2bf7fd9ccb-playground-example-6fe687c7d6e4",
                "isActive": true,
                "artifactReady": true,
                "loaded": true,
                "error": null
            }]
        }
    }))
    .unwrap();

    let promoted = &response.activation.target.promoted[0];
    assert!(promoted.platform_commit_hash.is_some());
    assert_eq!(
        promoted.live_commit_hash.as_deref(),
        Some("cfb6a6411712f1f65ce81d7373decd1d21be4ea1")
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
        path: ".".into(),
        ..Default::default()
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
                "ref": "abc1234",
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

#[test]
fn deploy_reads_recorded_source_id_from_repo_root_state() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("apps/bot", "bot");
    repo.commit("init");

    let mut state = sample_state();
    state.set_app_source_id(777);
    state.write(repo.root()).unwrap();

    let app_dir = repo.path("apps/bot");
    assert!(
        LocalDeployment::read(&app_dir).unwrap().is_none(),
        "deployment state is intentionally rooted at the source repo"
    );

    let args = deploy_args(&app_dir);
    // `sample_state` records repository_link https://github.com/a/b.git.
    assert_eq!(args.resolve_app_source_id(repo.root(), "a/b"), Some(777));
}

#[test]
fn recorded_source_id_is_dropped_when_it_belongs_to_another_repo() {
    // The backend resolves the source repo from app_source_id, so reusing a
    // recorded id for a different repo would silently override the repo the
    // caller asked for (in the wizard, the answer to "Source repo (owner/name)").
    let repo = TestRepo::new();
    repo.write_aomi_toml("apps/bot", "bot");
    repo.commit("init");

    let mut state = sample_state();
    state.set_app_source_id(777);
    state.write(repo.root()).unwrap();

    let args = deploy_args(&repo.path("apps/bot"));
    assert_eq!(
        args.resolve_app_source_id(repo.root(), "a/b"),
        Some(777),
        "same repo still reuses the recorded id"
    );
    assert_eq!(
        args.resolve_app_source_id(repo.root(), "someone-else/other"),
        None,
        "a mismatched repo must re-resolve instead of deploying the recorded source"
    );
}

#[tokio::test]
async fn status_reads_deployment_from_repo_root_when_path_is_app_dir() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("apps/bot", "bot");
    repo.commit("init");
    sample_state().write(repo.root()).unwrap();

    StatusArgs {
        backend: Some(String::new()),
        build_url: None,
        path: repo.path("apps/bot"),
        activation_token: None,
        json: true,
    }
    .run()
    .await
    .unwrap();
}

#[tokio::test]
async fn activate_dry_run_reads_deployment_from_repo_root_when_path_is_app_dir() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("apps/bot", "bot");
    repo.commit("init");
    sample_state().write(repo.root()).unwrap();

    ActivateArgs {
        path: repo.path("apps/bot"),
        dry_run: true,
        ..activate_args()
    }
    .run()
    .await
    .unwrap();
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn deploy_args(path: &Path) -> DeployStepArgs {
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

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

// ── regressions ─────────────────────────────────────────────────────────────

#[test]
fn config_update_merges_onto_disk_instead_of_clobbering() {
    // The wizard loads the config, then `ensure_logged_in` may persist a fresh
    // CLI bearer through its own handle. Saving the wizard's pre-login snapshot
    // wholesale used to drop that bearer, so a first-time user was sent through
    // the browser login twice.
    use super::config::AomiConfig;
    let home = TempDir::new().unwrap();
    let dir = home.path();

    // wizard.rs loads the config up front, before any login has run.
    let stale = AomiConfig::load_in(dir);
    assert!(stale.cli_access_token.is_none());

    // ensure_logged_in persists a bearer through its own handle.
    AomiConfig::update_in(dir, |config| {
        config.cli_access_token = Some("bearer-from-browser-login".into());
        config.github_login = Some("CeciliaZ030".into());
    })
    .unwrap();

    // The wizard then records the backend/platform it just collected.
    AomiConfig::update_in(dir, |config| {
        config.backend_url = Some("https://api-staging.aomi.dev".into());
        config.platform = Some("somm.finance".into());
    })
    .unwrap();

    let after = AomiConfig::load_in(dir);
    assert_eq!(after.platform.as_deref(), Some("somm.finance"));
    assert_eq!(
        after.backend_url.as_deref(),
        Some("https://api-staging.aomi.dev")
    );
    assert_eq!(
        after.cli_access_token.as_deref(),
        Some("bearer-from-browser-login"),
        "update must not drop fields written by another code path"
    );
    assert_eq!(after.github_login.as_deref(), Some("CeciliaZ030"));
}

#[test]
fn build_activate_input_carries_target_tags_and_omits_them_when_unused() {
    use super::types::BuildActivateInput;

    let plain = BuildActivateInput {
        platform: "somm.finance".into(),
        app_source_id: 1065,
        release_tags: vec!["apps-1-r0-bot-abc1234".into()],
        apps: vec!["bot".into()],
        target_tags: vec![],
    };
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        json!({
            "platform": "somm.finance",
            "appSourceId": 1065,
            "releaseTags": ["apps-1-r0-bot-abc1234"],
            "apps": ["bot"]
        }),
        "the common body must stay byte-for-byte what the BFF already accepts"
    );

    let tagged = BuildActivateInput {
        target_tags: vec!["prod".into()],
        ..plain
    };
    assert_eq!(
        serde_json::to_value(&tagged).unwrap()["targetTags"],
        json!(["prod"]),
        "--target-tag must reach the BFF instead of being silently dropped"
    );
}

#[test]
fn build_deploy_result_accepts_snake_case_and_camel_case_envelopes() {
    use super::types::BuildDeployResult;

    let deployment = json!({
        "id": "dep_1",
        "status": "pr_created",
        "source": {
            "installation_id": 1, "repository_id": 2,
            "repository_link": "https://github.com/a/b.git",
            "ref": "abc1234", "commit_hash": "abc1234",
            "aomi_toml_paths": ["aomi.toml"]
        },
        "platform": {
            "platform": "somm.finance", "repository": "aomi-labs/somm-finance-apps",
            "source_branch": "main", "deploy_branch": "publish",
            "apps": [{
                "name": "bot", "path": "apps/1/r0/bot",
                "aomi_toml_path": "aomi.toml", "release_tag": "apps-1-r0-bot-abc1234"
            }]
        }
    });

    let camel: BuildDeployResult = serde_json::from_value(json!({
        "ok": true, "appSourceId": 1065,
        "deployment": deployment, "projectUrl": "https://build.example/p/1"
    }))
    .unwrap();
    let snake: BuildDeployResult = serde_json::from_value(json!({
        "ok": true, "app_source_id": 1065,
        "deployment": deployment, "project_url": "https://build.example/p/1"
    }))
    .expect("the envelope must be as case-tolerant as the payload it wraps");
    assert_eq!(camel, snake);
    assert_eq!(camel.app_source_id, 1065);
}

#[test]
fn activated_app_rejects_a_missing_artifact_ready_flag() {
    use super::types::ActivatedApp;

    let present: ActivatedApp = serde_json::from_value(json!({
        "name": "bot", "isActive": true, "artifactReady": true, "loaded": true
    }))
    .unwrap();
    assert!(present.artifact_ready);

    // Defaulting this to `false` made an omitted field indistinguishable from a
    // genuinely unready artifact, which `print_activation` reports as a failure.
    let missing = serde_json::from_value::<ActivatedApp>(json!({
        "name": "bot", "isActive": true, "loaded": true
    }));
    assert!(
        missing.is_err(),
        "a missing artifact_ready must not silently mean `false`"
    );
}
