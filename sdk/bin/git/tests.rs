use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{CommandFactory, Parser};
use tempfile::TempDir;

use crate::activate::{ActivationPlan, Visibility};
use crate::cli::{Cli, Command as CliCommand};
use crate::deployment_state::{DeploymentState, deployment_path, read as read_deployment_state};
use crate::plan::{Deployment, Mode};
use crate::platform::Platform;

#[test]
fn publish_target_uses_aomi_toml_git_as_source_repo() {
    // Post-ADR 0009: `aomi.toml[app].git` is the only source of truth for
    // source_repo. Platform name is just a label; the same platform name with
    // different `git` resolves to different source_repos.
    let community_app = TestRepo::new();
    community_app.write_aomi_toml("", "probe", "https://github.com/aomi-labs/community-apps");
    community_app.write("src/lib.rs", "pub fn marker() {}\n");
    community_app.commit("initial app");

    let community = Deployment::dry_run(community_app.root(), Platform::new("community"), false)
        .expect("community dry run")
        .publish;
    assert_eq!(community.source_repo, "aomi-labs/community-apps");
    assert_eq!(community.publish_branch, "publish");
    assert_eq!(community.app_path, "apps/probe");
    assert!(community.release_tag.starts_with("apps-probe-"));

    let krexa_app = TestRepo::new();
    krexa_app.write(
        "aomi.toml",
        r#"
[app]
name = "probe"
platform = "krexa"
git = "https://github.com/aomi-labs/krexa-hosted-apps"
"#,
    );
    krexa_app.write("src/lib.rs", "pub fn marker() {}\n");
    krexa_app.commit("initial app");

    let krexa = Deployment::dry_run(krexa_app.root(), Platform::new("krexa"), false)
        .expect("krexa dry run")
        .publish;
    assert_eq!(krexa.source_repo, "aomi-labs/krexa-hosted-apps");
    assert_eq!(krexa.publish_branch, "publish");
    assert_eq!(krexa.app_path, "apps/probe");
    assert!(krexa.release_tag.starts_with("apps-probe-"));
}

#[test]
fn access_token_env_ref_resolves_and_literal_token_is_rejected() {
    // Literal tokens (no `$` prefix) MUST be rejected at parse so committed
    // configs cannot accidentally leak a real PAT.
    let bad = TestRepo::new();
    bad.write(
        "aomi.toml",
        r#"
[app]
name = "leaky-bot"
platform = "krexa"
git = "https://github.com/aomi-labs/krexa-hosted-apps"
access_token = "ghp_realtokenlookingthing"
"#,
    );
    bad.write("src/lib.rs", "pub fn marker() {}\n");
    bad.commit("initial app");

    let err = Deployment::dry_run(bad.root(), Platform::new("krexa"), false)
        .expect_err("literal access_token must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("$ENV_VAR_NAME") || msg.contains("env-var reference"),
        "error should explain env-var reference rule: {msg}"
    );

    // Env-var reference is accepted; resolved_access_token reads the env.
    let good = TestRepo::new();
    good.write(
        "aomi.toml",
        r#"
[app]
name = "krexa-bot"
platform = "krexa"
git = "https://github.com/aomi-labs/krexa-hosted-apps"
access_token = "$AOMI_TEST_KREXA_TOKEN"
"#,
    );
    good.write("src/lib.rs", "pub fn marker() {}\n");
    good.commit("initial app");

    let deployment = Deployment::dry_run(good.root(), Platform::new("krexa"), false)
        .expect("env-var-ref access_token must parse");
    assert_eq!(
        deployment.app.access_token.as_deref(),
        Some("$AOMI_TEST_KREXA_TOKEN")
    );

    // Env unset → resolve errors with the var name in the message.
    // SAFETY: tests in this file already mutate env via the existing
    // activation-token tests; we follow the same pattern and serialize on
    // --test-threads=1 to avoid cross-test races.
    unsafe { std::env::remove_var("AOMI_TEST_KREXA_TOKEN") };
    let err = deployment
        .app
        .resolved_access_token()
        .expect_err("missing env should error");
    assert!(err.to_string().contains("AOMI_TEST_KREXA_TOKEN"));

    // Env set → resolves to the value, never to the literal `$...`.
    unsafe { std::env::set_var("AOMI_TEST_KREXA_TOKEN", "ghp_resolved_at_runtime") };
    let token = deployment
        .app
        .resolved_access_token()
        .expect("env-resolved token")
        .expect("token is Some when env var is set");
    assert_eq!(token, "ghp_resolved_at_runtime");
    unsafe { std::env::remove_var("AOMI_TEST_KREXA_TOKEN") };
}

#[test]
fn dry_run_without_git_field_errors() {
    // ADR 0009 made aomi.toml the source of truth — a bare Cargo.toml can no
    // longer carry a deploy because it has no `git`. The error message must
    // point the user at their aomi.toml.
    let repo = TestRepo::new();
    repo.write(
        "Cargo.toml",
        r#"
[package]
name = "bare-app"
version = "0.1.0"
"#,
    );
    repo.commit("initial app");

    let err = Deployment::dry_run(repo.root(), Platform::new("community"), false)
        .expect_err("bare Cargo.toml should error: no git declared");
    let msg = err.to_string();
    assert!(
        msg.contains("[app].git") && msg.contains("aomi.toml"),
        "error should mention aomi.toml + [app].git: {msg}"
    );
}

#[test]
fn activate_command_builds_activate_app_request() {
    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "apps-alpha-trader-v2-abc1234",
        "--platform",
        "krexa",
        "--backend-url",
        "https://api.example.test/",
        "--activation-token",
        "activation-secret",
        "--visibility",
        "public",
        "--label",
        " Alpha Trader V2 ",
        "--source-repo",
        "aomi-labs/krexa-hosted-apps",
        "--source-commit",
        "abc1234def567890",
        "--source-tree",
        "tree123",
        "--source-digest",
        "sha256:source",
        "--target-tag",
        "Prod",
        "--target-tag",
        "platform-x",
    ])
    .expect("parse activate command");
    let CliCommand::Activate(args) = cli.command else {
        panic!("expected activate command");
    };

    let plan = args.plan().expect("activation plan");
    assert_eq!(plan.backend_url, "https://api.example.test");
    assert_eq!(plan.activation_token, "activation-secret");
    assert_eq!(
        plan.endpoint(),
        "https://api.example.test/api/admin/apps/activate"
    );
    assert_eq!(plan.request.name, "alpha-trader-v2");
    assert_eq!(plan.request.label.as_deref(), Some("Alpha Trader V2"));
    assert_eq!(plan.request.platform, Platform::new("krexa"));
    assert_eq!(plan.request.source_repo, "aomi-labs/krexa-hosted-apps");
    assert_eq!(plan.request.app_release_tag, "apps-alpha-trader-v2-abc1234");
    assert_eq!(
        plan.request.source_commit.as_deref(),
        Some("abc1234def567890")
    );
    assert_eq!(plan.request.source_tree.as_deref(), Some("tree123"));
    assert_eq!(plan.request.source_digest.as_deref(), Some("sha256:source"));
    assert_eq!(plan.request.target_tags, vec!["prod", "platform-x"]);
    assert!(plan.request.is_active);
    assert!(plan.request.is_public);
    assert_eq!(plan.request.metadata["requested_by"], "aomi-git");
    assert_eq!(plan.request.metadata["short_commit"], "abc1234");
}

#[test]
fn activate_command_rejects_legacy_admin_token_flag() {
    let error = Cli::command()
        .try_get_matches_from([
            "aomi-git",
            "activate",
            "apps-zora-abc1234",
            "--backend-url",
            "https://api.example.test",
            "--admin-token",
            "legacy-secret",
        ])
        .expect_err("legacy flag must not parse");

    assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
}

#[test]
fn activation_plan_requires_apps_release_tag() {
    let error = ActivationPlan::new(
        "zora-abc1234",
        Platform::new("community"),
        "https://api.example.test".to_string(),
        "activation-secret".to_string(),
        Visibility::Private,
        "aomi-labs/community-apps".to_string(),
        None,
        Vec::new(),
        None,
        None,
        None,
        None,
    )
    .expect_err("invalid release tag");

    assert!(error.to_string().contains("must start with `apps-`"));
}

#[test]
fn dry_run_plan_uses_nearest_app_config() {
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "apps/zora",
        "zora",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("apps/zora/src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let deployment = Deployment::dry_run(repo.path("apps/zora"), Platform::new("community"), false)
        .expect("dry run plan");

    assert_eq!(deployment.mode, Mode::DryRun);
    assert_eq!(deployment.platform, Platform::new("community"));
    assert_eq!(deployment.app.name, "zora");
    assert_eq!(deployment.source.source_path, PathBuf::from("apps/zora"));
    assert_eq!(deployment.publish.source_repo, "aomi-labs/community-apps");
    assert_eq!(deployment.publish.app_path, "apps/zora");
    assert!(deployment.publish.release_tag.starts_with("apps-zora-"));
    assert!(deployment.source.digest.starts_with("sha256:"));
    assert!(deployment.files.is_empty());
    assert!(!deployment.mode.stages_files());
    assert!(!deployment.mode.pushes());
}

#[test]
fn unknown_platform_resolves_via_aomi_toml_git() {
    // ADR 0009 F-2: a platform name not in the offline registry should still
    // resolve as long as aomi.toml declares [app].git — that becomes the
    // source_repo, and the publish path/tag use the default convention.
    let repo = TestRepo::new();
    repo.write(
        "aomi.toml",
        r#"
[app]
name = "foo-bot"
platform = "foo"
git = "https://github.com/example/foo-hosted-apps"
"#,
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let deployment = Deployment::dry_run(repo.root(), Platform::new("foo"), false)
        .expect("dry run for unknown platform");

    assert_eq!(deployment.publish.source_repo, "example/foo-hosted-apps");
    assert_eq!(deployment.publish.publish_branch, "publish");
    assert_eq!(deployment.publish.app_path, "apps/foo-bot");
    assert!(deployment.publish.release_tag.starts_with("apps-foo-bot-"));
}

#[test]
fn unknown_platform_without_git_field_errors() {
    let repo = TestRepo::new();
    repo.write(
        "aomi.toml",
        r#"
[app]
name = "lonely-app"
"#,
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let err = Deployment::dry_run(repo.root(), Platform::new("unknown-platform"), false)
        .expect_err("missing source repo should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("[app].git") && msg.contains("aomi.toml"),
        "error should mention [app].git + aomi.toml: {msg}"
    );
}

#[test]
fn server_tags_default_to_staging_when_aomi_toml_omits_them() {
    // Missing field → defaulted to ["staging"] and surfaced in deployment.json
    // checks so an operator can see at a glance that the deploy is staging-only.
    let repo = TestRepo::new();
    repo.write(
        "aomi.toml",
        r#"
[app]
name = "no-tags"
platform = "community"
git = "https://github.com/aomi-labs/community-apps"
"#,
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let deployment =
        Deployment::dry_run(repo.root(), Platform::new("community"), false).expect("dry run");
    assert!(deployment.app.server_tags_defaulted);
    assert_eq!(deployment.app.server_tags, vec!["staging"]);

    let state = deployment.to_state();
    assert_eq!(state.target.server_tags, vec!["staging"]);
    let server_tags_check = state
        .checks
        .iter()
        .find(|c| c.name == "server_tags")
        .expect("plan should record a server_tags check");
    assert!(server_tags_check.passed);
    let detail = server_tags_check.detail.as_deref().unwrap_or("");
    assert!(
        detail.contains("defaulted") && detail.contains("staging"),
        "default should be visible in the check detail: {detail}"
    );
}

#[test]
fn server_tags_default_applied_when_aomi_toml_sets_empty_array() {
    // Explicit `server_tags = []` is treated the same as missing — operators
    // should not be able to opt out of the safe default by writing `[]`.
    let repo = TestRepo::new();
    repo.write(
        "aomi.toml",
        r#"
[app]
name = "empty-tags"
platform = "community"
git = "https://github.com/aomi-labs/community-apps"
server_tags = []
"#,
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let deployment =
        Deployment::dry_run(repo.root(), Platform::new("community"), false).expect("dry run");
    assert!(deployment.app.server_tags_defaulted);
    assert_eq!(deployment.app.server_tags, vec!["staging"]);
}

#[test]
fn explicit_server_tags_are_not_overridden_by_default() {
    let repo = TestRepo::new();
    repo.write(
        "aomi.toml",
        r#"
[app]
name = "explicit-tags"
platform = "community"
git = "https://github.com/aomi-labs/community-apps"
server_tags = ["Prod", "community"]
"#,
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let deployment =
        Deployment::dry_run(repo.root(), Platform::new("community"), false).expect("dry run");
    assert!(!deployment.app.server_tags_defaulted);
    assert_eq!(deployment.app.server_tags, vec!["prod", "community"]);

    let state = deployment.to_state();
    let server_tags_check = state
        .checks
        .iter()
        .find(|c| c.name == "server_tags")
        .expect("plan should record a server_tags check");
    let detail = server_tags_check.detail.as_deref().unwrap_or("");
    assert!(
        detail.contains("from aomi.toml") && !detail.contains("defaulted"),
        "explicit value should not be reported as defaulted: {detail}"
    );
}

#[test]
fn deployment_state_round_trips_with_offline_checks() {
    let repo = TestRepo::new();
    repo.write(
        "aomi.toml",
        r#"
[app]
name = "alice-bot"
display_name = "Alice Bot"
platform = "community"
git = "https://github.com/aomi-labs/community-apps"
branch = "experiment"
public = true
server_tags = ["Prod", "community", "prod"]
"#,
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let deployment =
        Deployment::dry_run(repo.root(), Platform::new("community"), false).expect("dry run");

    // App fields surface through to the in-memory plan.
    assert_eq!(deployment.app.name, "alice-bot");
    assert_eq!(deployment.app.platform.as_deref(), Some("community"));
    assert_eq!(
        deployment.app.git.as_deref(),
        Some("https://github.com/aomi-labs/community-apps")
    );
    assert_eq!(deployment.app.branch.as_deref(), Some("experiment"));
    assert_eq!(deployment.app.public, Some(true));
    assert_eq!(deployment.app.server_tags, vec!["prod", "community"]);

    // to_state produces a coherent artifact with all three flags false.
    let state = deployment.to_state();
    assert!(!state.state.pushed);
    assert!(!state.state.deployed);
    assert!(!state.state.activated);
    assert_eq!(state.target.branch, "experiment");
    assert!(state.target.release_tag.starts_with("apps-alice-bot-"));
    assert_eq!(state.target.server_tags, vec!["prod", "community"]);
    assert_eq!(state.platform.name.as_deref(), Some("community"));
    assert_eq!(state.platform.resolved_deploy_branch, None);

    // Offline checks recorded.
    let names: Vec<&str> = state.checks.iter().map(|c| c.name.as_str()).collect();
    assert!(names.contains(&"git_clean"));
    assert!(names.contains(&"platform_declared"));
    assert!(names.contains(&"git_declared"));
    assert!(state.checks.iter().all(|c| c.passed));

    // Persist + reload.
    let path = crate::deployment_state::write(repo.root(), &state).expect("write state");
    assert_eq!(path, deployment_path(repo.root()));
    let reloaded: DeploymentState = read_deployment_state(repo.root())
        .expect("read state")
        .expect("file should exist after write");
    assert_eq!(reloaded.target.branch, state.target.branch);
    assert_eq!(reloaded.app.name, "alice-bot");
}

#[test]
fn dry_run_plan_uses_aomi_toml() {
    let repo = TestRepo::new();
    repo.write(
        "aomi.toml",
        r#"
[app]
name = "alpha-trader-v2"
display_name = "Alpha Trader V2"
platform = "krexa"
git = "https://github.com/aomi-labs/krexa-hosted-apps"
"#,
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let deployment =
        Deployment::dry_run(repo.root(), Platform::new("krexa"), false).expect("dry run plan");

    assert_eq!(deployment.platform, Platform::new("krexa"));
    assert_eq!(deployment.app.name, "alpha-trader-v2");
    assert_eq!(deployment.app.display_name, "Alpha Trader V2");
    assert_eq!(deployment.source.source_path, PathBuf::from("."));
    assert_eq!(
        deployment.publish.source_repo,
        "aomi-labs/krexa-hosted-apps"
    );
    assert!(
        deployment
            .publish
            .release_tag
            .starts_with("apps-alpha-trader-v2-")
    );
}

#[test]
fn dry_run_plan_serializes_to_json() {
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "json-app",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.commit("initial app");

    let deployment =
        Deployment::dry_run(repo.root(), Platform::new("community"), false).expect("dry run plan");
    let json = serde_json::to_value(&deployment).expect("plan json");

    assert_eq!(json["mode"], "dry-run");
    assert_eq!(json["platform"], "community");
    assert_eq!(json["publish"]["source_repo"], "aomi-labs/community-apps");
    assert!(
        json["publish"]["release_tag"]
            .as_str()
            .expect("release tag str")
            .starts_with("apps-json-app-")
    );
}

#[test]
fn dot_aomi_config_uses_parent_app_root() {
    let repo = TestRepo::new();
    repo.write(
        "apps/alpha/.aomi/app.toml",
        r#"
[app]
name = "alpha"
display_name = "Alpha"
platform = "community"
git = "https://github.com/aomi-labs/community-apps"
"#,
    );
    repo.write("apps/alpha/src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let deployment =
        Deployment::dry_run(repo.path("apps/alpha"), Platform::new("community"), false)
            .expect("dry run plan");

    assert_eq!(
        deployment.app.config_path,
        PathBuf::from("apps/alpha/.aomi/app.toml")
    );
    assert_eq!(deployment.source.source_path, PathBuf::from("apps/alpha"));
}

#[test]
fn dry_run_rejects_dirty_tree_by_default() {
    let repo = TestRepo::new();
    repo.write(
        "Cargo.toml",
        r#"
[package]
name = "dirty-app"
version = "0.1.0"
"#,
    );
    repo.commit("initial app");
    repo.write("src/lib.rs", "pub fn dirty() {}\n");

    let error = Deployment::dry_run(repo.root(), Platform::new("community"), false)
        .expect_err("dirty tree should fail");
    assert!(error.to_string().contains("git tree is dirty"));
}

#[test]
fn dirty_tree_can_be_allowed_for_plan_only() {
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "dirty-app",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.commit("initial app");
    repo.write("src/lib.rs", "pub fn dirty() {}\n");

    let deployment =
        Deployment::dry_run(repo.root(), Platform::new("community"), true).expect("dirty dry run");
    assert!(deployment.source.dirty);
}

#[test]
fn source_staging_writes_files_and_manifest() {
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "apps/zora",
        "zora",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("apps/zora/src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let stage = TempDir::new().expect("stage tempdir");
    let stale = stage.path().join("apps/zora/stale.txt");
    fs::create_dir_all(stale.parent().unwrap()).expect("stale parent");
    fs::write(&stale, "old").expect("stale file");

    let outcome = Deployment::stage(
        repo.path("apps/zora"),
        Platform::new("community"),
        stage.path(),
    )
    .expect("stage app");
    let stage_root = stage.path().canonicalize().expect("canonical stage root");

    assert_eq!(outcome.deployment.mode, Mode::Stage);
    assert!(outcome.deployment.mode.stages_files());
    assert!(!outcome.deployment.mode.pushes());
    assert_eq!(outcome.app_dir, stage_root.join("apps/zora"));
    assert_eq!(
        outcome.manifest_path,
        stage_root.join("apps/zora/.aomi/deployment.json")
    );
    assert!(stage.path().join("apps/zora/aomi.toml").is_file());
    assert!(stage.path().join("apps/zora/src/lib.rs").is_file());
    assert!(!stale.exists(), "stale target files should be pruned");

    let manifest: DeploymentState =
        serde_json::from_slice(&fs::read(&outcome.manifest_path).expect("manifest bytes"))
            .expect("manifest json");
    assert_eq!(manifest.platform.name.as_deref(), Some("community"));
    assert_eq!(manifest.app.name, "zora");
    assert_eq!(
        manifest.platform.github_repo.as_deref(),
        Some("https://github.com/aomi-labs/community-apps")
    );
    let paths: Vec<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
    assert!(paths.contains(&"aomi.toml"));
    assert!(paths.contains(&"src/lib.rs"));
    assert!(
        manifest
            .files
            .iter()
            .all(|file| file.sha256.starts_with("sha256:") && file.bytes > 0)
    );
}

#[test]
fn source_staging_rejects_dirty_tree() {
    let repo = TestRepo::new();
    repo.write(
        "Cargo.toml",
        r#"
[package]
name = "dirty-app"
version = "0.1.0"
"#,
    );
    repo.commit("initial app");
    repo.write("src/lib.rs", "pub fn dirty() {}\n");

    let stage = TempDir::new().expect("stage tempdir");
    let error = Deployment::stage(repo.root(), Platform::new("community"), stage.path())
        .expect_err("dirty staging should fail");

    assert!(error.to_string().contains("git tree is dirty"));
}

#[test]
fn git_transport_commits_without_push() {
    let source = TestRepo::new();
    source.write_aomi_toml(
        "apps/zora",
        "zora",
        "git@github.com:aomi-labs/community-apps.git",
    );
    source.write("apps/zora/src/lib.rs", "pub fn marker() {}\n");
    source.write("apps/zora/.gitignore", "/.aomi/\n");
    source.commit("initial app");

    let platform = TestRepo::new();
    platform.set_origin("git@github.com:aomi-labs/community-apps.git");
    platform.write("README.md", "community apps\n");
    platform.commit("initial platform repo");

    let outcome = Deployment::git_transport(
        source.path("apps/zora"),
        Platform::new("community"),
        platform.root(),
        false,
    )
    .expect("git transport");

    assert_eq!(outcome.deployment.mode, Mode::GitTransport);
    assert!(outcome.deployment.mode.stages_files());
    assert!(outcome.deployment.mode.pushes());
    assert_eq!(outcome.branch.as_deref(), Some("publish"));
    assert!(outcome.commit.is_some());
    assert!(!outcome.pushed);
    assert!(platform.path("apps/zora/aomi.toml").is_file());
    assert!(platform.path("apps/zora/.gitignore").is_file());
    assert!(platform.path("apps/zora/.aomi/deployment.json").is_file());

    let message = test_git_output(platform.root(), ["log", "-1", "--pretty=%B"]);
    assert!(message.contains("zora"));
    assert!(message.contains("community"));
    assert!(message.contains(&outcome.deployment.source.commit));
    assert!(message.contains(&outcome.deployment.publish.release_tag));
    assert_eq!(
        test_git_output(platform.root(), ["branch", "--show-current"]).trim(),
        "publish"
    );
}

#[test]
fn git_transport_rejects_wrong_platform_remote() {
    let source = TestRepo::new();
    source.write_aomi_toml("", "zora", "git@github.com:aomi-labs/community-apps.git");
    source.commit("initial app");

    let platform = TestRepo::new();
    platform.set_origin("git@github.com:aomi-labs/krexa-hosted-apps.git");
    platform.write("README.md", "wrong repo\n");
    platform.commit("initial platform repo");

    let error = Deployment::git_transport(
        source.root(),
        Platform::new("community"),
        platform.root(),
        false,
    )
    .expect_err("wrong remote should fail");
    assert!(
        error
            .to_string()
            .contains("does not match expected publish repo")
    );
}

#[test]
fn git_transport_rejects_unowned_dirty_platform_files() {
    let source = TestRepo::new();
    source.write_aomi_toml("", "zora", "git@github.com:aomi-labs/community-apps.git");
    source.commit("initial app");

    let platform = TestRepo::new();
    platform.set_origin("git@github.com:aomi-labs/community-apps.git");
    platform.write("README.md", "community apps\n");
    platform.commit("initial platform repo");
    platform.write("README.md", "dirty unrelated file\n");

    let error = Deployment::git_transport(
        source.root(),
        Platform::new("community"),
        platform.root(),
        false,
    )
    .expect_err("unowned dirty platform file should fail");
    assert!(
        error
            .to_string()
            .contains("dirty files outside owned publish path")
    );
    assert!(error.to_string().contains("README.md"));
}

#[test]
fn git_transport_allows_owned_dirty_platform_files() {
    let source = TestRepo::new();
    source.write_aomi_toml("", "zora", "git@github.com:aomi-labs/community-apps.git");
    source.commit("initial app");

    let platform = TestRepo::new();
    platform.set_origin("git@github.com:aomi-labs/community-apps.git");
    platform.write("README.md", "community apps\n");
    platform.commit("initial platform repo");
    platform.write("apps/zora/stale.txt", "owned dirty file\n");

    let outcome = Deployment::git_transport(
        source.root(),
        Platform::new("community"),
        platform.root(),
        false,
    )
    .expect("owned dirty platform file can be replaced");

    assert!(outcome.commit.is_some());
    assert!(!platform.path("apps/zora/stale.txt").exists());
    assert!(platform.path("apps/zora/aomi.toml").is_file());
}

struct TestRepo {
    tmp: TempDir,
}

impl TestRepo {
    fn new() -> Self {
        let tmp = TempDir::new().expect("tempdir");
        run_git(tmp.path(), ["init", "-q"]);
        run_git(
            tmp.path(),
            ["config", "user.email", "aomi-git@example.test"],
        );
        run_git(tmp.path(), ["config", "user.name", "Aomi Git"]);
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

    fn set_origin(&self, url: &str) {
        run_git(self.root(), ["remote", "add", "origin", url]);
    }

    fn commit(&self, message: &str) {
        run_git(self.root(), ["add", "."]);
        run_git(self.root(), ["commit", "-q", "-m", message]);
    }

    /// Write a minimal `aomi.toml` at `relative_dir/aomi.toml` that satisfies
    /// `PublishTarget::resolve`. Pass `""` for the repo root.
    fn write_aomi_toml(&self, relative_dir: &str, name: &str, git: &str) {
        let path = if relative_dir.is_empty() {
            "aomi.toml".to_string()
        } else {
            format!("{relative_dir}/aomi.toml")
        };
        self.write(
            &path,
            &format!("[app]\nname = \"{name}\"\nplatform = \"community\"\ngit = \"{git}\"\n"),
        );
    }
}

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .status()
        .expect("run git");
    assert!(status.success(), "git command failed");
}

fn test_git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> String {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git command failed");
    String::from_utf8(output.stdout).expect("git output utf8")
}
