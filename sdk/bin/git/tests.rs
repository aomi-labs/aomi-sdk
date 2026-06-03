use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::{CommandFactory, Parser};
use tempfile::TempDir;

use crate::activate::{ActivationPlan, Visibility};
use crate::cli::{Cli, Command as CliCommand};
use crate::deployment_state::{
    DeploymentState, StageId, deployment_path, read as read_deployment_state,
};
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
    assert!(community.app_release_tag.starts_with("apps-probe-"));

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
    assert!(krexa.app_release_tag.starts_with("apps-probe-"));
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

#[tokio::test]
async fn activate_command_builds_activate_app_request() {
    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "apps-alpha-trader-v2-abc1234",
        "--platform",
        "krexa",
        "--backend",
        "https://api.example.test/",
        "--activation-token",
        "activation-secret",
        "--visibility",
        "public",
        "--display-name",
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

    let plan = args.plan().await.expect("activation plan");
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
    assert_eq!(plan.request.server_tags, vec!["prod", "platform-x"]);
    let body = serde_json::to_value(&plan.request).expect("serialize activation request");
    assert_eq!(
        body["target_tags"],
        serde_json::json!(["prod", "platform-x"])
    );
    assert!(body.get("server_tags").is_none());
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
            "--backend",
            "https://api.example.test",
            "--admin-token",
            "legacy-secret",
        ])
        .expect_err("legacy flag must not parse");

    assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
}

#[tokio::test]
async fn activate_falls_back_to_deployment_json_when_flags_omitted() {
    // After a successful `aomi-git deploy`, .aomi/deployment.json carries
    // every field activate needs. In the source-repo happy path the operator
    // should be able to run `aomi-git activate --target-tag staging` and have
    // the rest filled in automatically.
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "fallback-bot",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    // Stage a deployment.json the way deploy would.
    let state = Deployment::dry_run(repo.root(), Platform::new("community"), false)
        .expect("dry run")
        .to_state();
    crate::deployment_state::write(repo.root(), &state).expect("write state");

    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "--backend",
        "https://api.example.test",
        "--activation-token",
        "activation-secret",
        "--target-tag",
        "staging",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse activate");
    let CliCommand::Activate(args) = cli.command else {
        panic!("expected activate");
    };

    let plan = args
        .plan()
        .await
        .expect("plan resolves from deployment.json");
    // app_release_tag pulled from deployment.json's target.app_release_tag.
    assert!(
        plan.request
            .app_release_tag
            .starts_with("apps-fallback-bot-")
    );
    // git pulled from deployment.json's app.git → normalized owner/repo.
    assert_eq!(plan.request.source_repo, "aomi-labs/community-apps");
    // platform pulled from deployment.json's app.platform.
    assert_eq!(plan.request.platform, Platform::new("community"));
    assert_eq!(plan.request.server_tags, vec!["staging"]);
}

#[tokio::test]
async fn activate_app_release_tag_flag_overrides_deployment_json() {
    // CLI flag wins over deployment.json — operators can re-activate a prior
    // app_release_tag without re-deploying.
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "override-bot",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let state = Deployment::dry_run(repo.root(), Platform::new("community"), false)
        .expect("dry run")
        .to_state();
    crate::deployment_state::write(repo.root(), &state).expect("write state");

    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "apps-override-bot-deadbeef0123",
        "--backend",
        "https://api.example.test",
        "--activation-token",
        "activation-secret",
        "--target-tag",
        "staging",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse activate");
    let CliCommand::Activate(args) = cli.command else {
        panic!("expected activate");
    };

    let plan = args.plan().await.expect("plan");
    // CLI positional wins over deployment.json's target.app_release_tag.
    assert_eq!(
        plan.request.app_release_tag,
        "apps-override-bot-deadbeef0123"
    );
}

#[tokio::test]
async fn activate_dry_run_persists_effective_flag_overrides_to_deployment_json() {
    let repo = TestRepo::new();
    repo.write(
        "aomi.toml",
        r#"
[app]
name = "token-bot"
platform = "community"
git = "https://github.com/aomi-labs/community-apps"
access_token = "$OLD_ACCESS_TOKEN"
server_tags = ["staging", "prod"]
"#,
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let state = Deployment::dry_run(repo.root(), Platform::new("community"), false)
        .expect("dry run")
        .to_state();
    crate::deployment_state::write(repo.root(), &state).expect("write state");

    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "apps-token-bot-deadbeef0123",
        "--backend",
        "https://api.example.test",
        "--activation-token",
        "activation-secret",
        "--access-token",
        "34567",
        "--target-tag",
        "staging",
        "--visibility",
        "public",
        "--display-name",
        "Token Bot Live",
        "--path",
        repo.root().to_str().unwrap(),
        "--dry-run",
    ])
    .expect("parse activate");
    let CliCommand::Activate(args) = cli.command else {
        panic!("expected activate");
    };

    args.run().await.expect("dry-run activate");

    let updated = read_deployment_state(repo.root())
        .expect("read state")
        .expect("deployment state");
    assert_eq!(
        updated.target.app_release_tag,
        "apps-token-bot-deadbeef0123"
    );
    assert_eq!(updated.app.access_token.as_deref(), Some("34567"));
    assert_eq!(updated.target.server_tags, vec!["staging"]);
    assert_eq!(updated.app.server_tags, vec!["staging"]);
    assert_eq!(updated.app.public, Some(true));
    assert_eq!(updated.app.display_name, "Token Bot Live");
    assert!(!updated.state.activated);
}

#[tokio::test]
async fn activate_without_app_release_tag_and_without_deployment_json_errors_clearly() {
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "lonely-bot",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    // No deployment.json yet.
    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "--backend",
        "https://api.example.test",
        "--activation-token",
        "activation-secret",
        "--target-tag",
        "staging",
        "--source-repo",
        "aomi-labs/community-apps",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse activate");
    let CliCommand::Activate(args) = cli.command else {
        panic!("expected activate");
    };

    let err = args
        .plan()
        .await
        .expect_err("missing app_release_tag should error");
    let msg = err.to_string();
    assert!(
        msg.contains("app_release_tag") && msg.contains("deployment.json"),
        "error should point at both fixes: {msg}"
    );
}

#[tokio::test]
async fn activate_server_tags_default_from_deployment_json_server_tags() {
    // Happy path: contributor declared `server_tags = ["staging"]` in
    // aomi.toml (or defaulted there), ops omits `--target-tag` on activate —
    // server_tags should auto-fill from the build's declared intent.
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "default-target-bot",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let state = Deployment::dry_run(repo.root(), Platform::new("community"), false)
        .expect("dry run")
        .to_state();
    assert_eq!(state.target.server_tags, vec!["staging"]); // sanity
    crate::deployment_state::write(repo.root(), &state).expect("write state");

    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "--backend",
        "https://api.example.test",
        "--activation-token",
        "activation-secret",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse activate (no --target-tag)");
    let CliCommand::Activate(args) = cli.command else {
        panic!("expected activate");
    };

    let plan = args
        .plan()
        .await
        .expect("plan resolves server_tags from deployment.json");
    assert_eq!(plan.request.server_tags, vec!["staging"]);
}

#[tokio::test]
async fn activate_rejects_target_tag_widening_beyond_server_tags() {
    // Footgun guard: build declared server_tags = ["staging"] but ops tries
    // to activate `--target-tag prod`. Reject — operator cannot widen scope
    // beyond the contributor's declared intent.
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "widen-bot",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let state = Deployment::dry_run(repo.root(), Platform::new("community"), false)
        .expect("dry run")
        .to_state();
    crate::deployment_state::write(repo.root(), &state).expect("write state");

    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "--backend",
        "https://api.example.test",
        "--activation-token",
        "activation-secret",
        "--target-tag",
        "prod",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse activate");
    let CliCommand::Activate(args) = cli.command else {
        panic!("expected activate");
    };

    let err = args.plan().await.expect_err("widening must be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("widen") && msg.contains("server_tags") && msg.contains("prod"),
        "error should name the widening tag and reference server_tags: {msg}"
    );
}

#[tokio::test]
async fn activate_allows_target_tag_narrowing_within_server_tags() {
    // Build declared server_tags = ["staging", "prod"]; ops activates to just
    // "staging" first. Subset OK — operator can narrow.
    let repo = TestRepo::new();
    repo.write(
        "aomi.toml",
        r#"
[app]
name = "narrow-bot"
platform = "community"
git = "https://github.com/aomi-labs/community-apps"
server_tags = ["staging", "prod"]
"#,
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let state = Deployment::dry_run(repo.root(), Platform::new("community"), false)
        .expect("dry run")
        .to_state();
    assert_eq!(state.target.server_tags, vec!["staging", "prod"]); // sanity
    crate::deployment_state::write(repo.root(), &state).expect("write state");

    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "--backend",
        "https://api.example.test",
        "--activation-token",
        "activation-secret",
        "--target-tag",
        "staging",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse activate");
    let CliCommand::Activate(args) = cli.command else {
        panic!("expected activate");
    };

    let plan = args.plan().await.expect("narrowing must be allowed");
    assert_eq!(plan.request.server_tags, vec!["staging"]);
}

#[tokio::test]
async fn activate_without_server_tags_anywhere_errors_clearly() {
    // No --target-tag, no deployment.json — must fail with a message that
    // points at both fixes.
    let cli = Cli::try_parse_from([
        "aomi-git",
        "activate",
        "apps-orphan-bot-deadbeef0123",
        "--backend",
        "https://api.example.test",
        "--activation-token",
        "activation-secret",
        "--platform",
        "community",
        "--source-repo",
        "aomi-labs/community-apps",
        "--path",
        "/nonexistent/path/no/deployment-json/here",
    ])
    .expect("parse activate");
    let CliCommand::Activate(args) = cli.command else {
        panic!("expected activate");
    };

    let err = args
        .plan()
        .await
        .expect_err("no target tags anywhere must error");
    let msg = err.to_string();
    assert!(
        msg.contains("target tag") || msg.contains("server_tags"),
        "error should point at the fix: {msg}"
    );
}

#[test]
fn recompute_deployed_requires_pushed_even_when_branch_matches() {
    // Regression: previously `recompute_deployed` only compared the resolved
    // deploy branch to `target.branch`, so a dry-run on a platform whose
    // deployment_branch matched would flip `deployed = true` despite
    // `pushed = false`. `deployed` is a strict subset of `pushed`.
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "deployed-flag-bot",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let mut state = Deployment::dry_run(repo.root(), Platform::new("community"), false)
        .expect("dry run")
        .to_state();

    // Simulate post-preflight: branch contract resolves cleanly, no push.
    state.platform.resolved_deploy_branch = Some(state.target.branch.clone());
    assert!(!state.state.pushed);
    state.recompute_deployed();
    assert!(
        !state.state.deployed,
        "deployed must stay false until pushed flips true"
    );

    state.state.pushed = true;
    state.recompute_deployed();
    assert!(state.state.deployed);

    state.platform.resolved_deploy_branch = Some("some-other-branch".to_string());
    state.recompute_deployed();
    assert!(!state.state.deployed);
}

#[test]
fn deploy_platform_flag_defaults_from_aomi_toml() {
    // `--platform` is now Option<Platform> defaulting to aomi.toml's
    // [app].platform. Passing no --platform on a community app must parse
    // and the args struct should hold None (defaulting happens at run time).
    let cli = Cli::try_parse_from(["aomi-git", "deploy", "--dry-run"])
        .expect("deploy --dry-run parses without --platform");
    let CliCommand::Deploy(args) = cli.command else {
        panic!("expected deploy");
    };
    assert!(
        args.platform.is_none(),
        "platform should be None on the args struct; defaulting happens at run time from aomi.toml"
    );
    assert!(args.dry_run);
}

#[test]
fn deploy_kills_legacy_flags() {
    // Retired flags must reject at parse time so users adopt the current surface.
    for legacy in [
        "--platform-repo-dir",
        "--stage-dir",
        "--no-push",
        "--preflight",
        "--backend-url",
        "--git",
    ] {
        let err = Cli::command()
            .try_get_matches_from(["aomi-git", "deploy", legacy, "value"])
            .expect_err(&format!("{legacy} must not parse"));
        assert_eq!(
            err.kind(),
            clap::error::ErrorKind::UnknownArgument,
            "{legacy} should be UnknownArgument, got {:?}",
            err.kind()
        );
    }
}

#[test]
fn activate_and_status_reject_legacy_git_flag() {
    for (subcommand, extra) in [
        ("activate", &["apps-my-bot-abc1234"][..]),
        ("status", &[][..]),
    ] {
        let err = Cli::command()
            .try_get_matches_from(
                std::iter::once("aomi-git")
                    .chain(std::iter::once(subcommand))
                    .chain(extra.iter().copied())
                    .chain(["--git", "aomi-labs/community-apps"].into_iter()),
            )
            .expect_err("--git must not parse");
        assert_eq!(
            err.kind(),
            clap::error::ErrorKind::UnknownArgument,
            "{subcommand} --git should be UnknownArgument, got {:?}",
            err.kind()
        );
    }
}

#[test]
fn activation_plan_requires_apps_app_release_tag() {
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
    .expect_err("invalid app_release_tag");

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
    assert!(deployment.publish.app_release_tag.starts_with("apps-zora-"));
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
    assert!(
        deployment
            .publish
            .app_release_tag
            .starts_with("apps-foo-bot-")
    );
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
    // resolved facts so an operator can see that the deploy is staging-only.
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
    let manifest = state
        .stages
        .iter()
        .find(|stage| stage.stage == StageId::Manifest)
        .expect("manifest stage");
    assert_eq!(
        manifest.resolved["server_tags"],
        serde_json::json!(["staging"])
    );
    assert_eq!(manifest.resolved["defaulted"], serde_json::json!(true));
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
    let manifest = state
        .stages
        .iter()
        .find(|stage| stage.stage == StageId::Manifest)
        .expect("manifest stage");
    assert_eq!(
        manifest.resolved["server_tags"],
        serde_json::json!(["prod", "community"])
    );
    assert_eq!(manifest.resolved["defaulted"], serde_json::json!(false));
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
    assert!(state.target.app_release_tag.starts_with("apps-alice-bot-"));
    assert_eq!(state.target.server_tags, vec!["prod", "community"]);
    assert_eq!(state.platform.name.as_deref(), Some("community"));
    assert_eq!(state.platform.resolved_deploy_branch, None);

    // Offline checks recorded.
    let names: Vec<&str> = state
        .stages
        .iter()
        .flat_map(|stage| stage.checks.iter().map(|check| check.name.as_str()))
        .collect();
    assert!(names.contains(&"git_clean"));
    assert!(names.contains(&"platform_declared"));
    assert!(names.contains(&"git_declared"));
    assert!(
        state
            .stages
            .iter()
            .flat_map(|stage| stage.checks.iter())
            .all(|check| check.passed)
    );

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
            .app_release_tag
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
        json["publish"]["app_release_tag"]
            .as_str()
            .expect("app_release_tag str")
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
    assert!(message.contains(&outcome.deployment.publish.app_release_tag));
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

#[tokio::test]
async fn status_errors_without_deployment_json() {
    // `aomi-git status` in a directory that never had a deploy should fail with
    // a message pointing at `aomi-git deploy` — and must not touch the network.
    let repo = TestRepo::new();
    let cli = Cli::try_parse_from([
        "aomi-git",
        "status",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse status");
    let CliCommand::Status(args) = cli.command else {
        panic!("expected status");
    };

    let err = args
        .run()
        .await
        .expect_err("missing deployment.json must error");
    let msg = err.to_string();
    assert!(
        msg.contains("deployment.json") && msg.contains("aomi-git deploy"),
        "error should point at running deploy first: {msg}"
    );
}

#[tokio::test]
async fn request_dry_run_resolves_app_and_repo_from_aomi_toml() {
    // `aomi-git request --dry-run` resolves app/platform/repo from aomi.toml
    // (no deployment.json needed — it runs before the first deploy) and prints
    // the ops message without posting.
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "request-bot",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let cli = Cli::try_parse_from([
        "aomi-git",
        "request",
        "--email",
        "alice@gmail.com",
        "--git-account",
        "alice-git-acc",
        "--dry-run",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse request --dry-run");
    let CliCommand::Request(args) = cli.command else {
        panic!("expected request");
    };

    args.run()
        .await
        .expect("dry-run should resolve from aomi.toml and not post");
}

#[tokio::test]
async fn request_rejects_a_malformed_email() {
    let repo = TestRepo::new();
    repo.write_aomi_toml(
        "",
        "request-bot",
        "https://github.com/aomi-labs/community-apps",
    );
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let cli = Cli::try_parse_from([
        "aomi-git",
        "request",
        "--email",
        "not-an-email",
        "--git-account",
        "alice-git-acc",
        "--dry-run",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse request");
    let CliCommand::Request(args) = cli.command else {
        panic!("expected request");
    };

    let err = args.run().await.expect_err("malformed email must error");
    assert!(err.to_string().contains("email"), "{err}");
}

#[tokio::test]
async fn request_errors_when_app_slug_is_unknown() {
    // No aomi.toml and no --app: nothing to identify the app, so it must fail
    // rather than post a request for an unknown app.
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "pub fn marker() {}\n");
    repo.commit("initial app");

    let cli = Cli::try_parse_from([
        "aomi-git",
        "request",
        "--email",
        "alice@gmail.com",
        "--git-account",
        "alice-git-acc",
        "--dry-run",
        "--path",
        repo.root().to_str().unwrap(),
    ])
    .expect("parse request");
    let CliCommand::Request(args) = cli.command else {
        panic!("expected request");
    };

    let err = args.run().await.expect_err("unknown app slug must error");
    assert!(err.to_string().contains("app slug"), "{err}");
}

#[test]
fn status_report_renders_ready_to_activate() {
    // Offline rendering check: a published release + green CI on an
    // unactivated app should advertise "ready to activate".
    use crate::status::{BackendStatus, CiStatus, LocalState, ReleaseStatus, StatusReport};

    let report = StatusReport {
        repo: "aomi-labs/community-apps".to_string(),
        app_release_tag: "apps-my-bot-abc1234".to_string(),
        branch: "publish".to_string(),
        local: LocalState {
            pushed: true,
            deployed: true,
            activated: false,
            updated_at: 0,
        },
        ci: CiStatus::Success {
            name: Some("publish-apps".to_string()),
            url: "https://github.com/aomi-labs/community-apps/actions/runs/1".to_string(),
        },
        release: ReleaseStatus::Available {
            url: "https://github.com/aomi-labs/community-apps/releases/tag/apps-my-bot-abc1234"
                .to_string(),
            assets: 2,
        },
        backend: BackendStatus::NotChecked,
    };

    assert!(report.ready_to_activate());
    let rendered = report.render();
    assert!(rendered.contains("ready to activate"), "{rendered}");
    assert!(rendered.contains("aomi-git activate"), "{rendered}");
    assert!(rendered.contains("apps-my-bot-abc1234"), "{rendered}");
}

#[test]
fn status_report_pending_release_is_not_ready() {
    use crate::status::{BackendStatus, CiStatus, LocalState, ReleaseStatus, StatusReport};

    let report = StatusReport {
        repo: "aomi-labs/community-apps".to_string(),
        app_release_tag: "apps-my-bot-abc1234".to_string(),
        branch: "publish".to_string(),
        local: LocalState {
            pushed: true,
            deployed: true,
            activated: false,
            updated_at: 0,
        },
        ci: CiStatus::Running {
            name: None,
            url: "https://github.com/aomi-labs/community-apps/actions/runs/2".to_string(),
        },
        release: ReleaseStatus::Pending,
        backend: BackendStatus::NotChecked,
    };

    assert!(!report.ready_to_activate());
    let rendered = report.render();
    assert!(rendered.contains("running"), "{rendered}");
    assert!(rendered.contains("pending"), "{rendered}");
    assert!(!rendered.contains("Request activation"), "{rendered}");
}

#[test]
fn status_report_renders_backend_db_row_and_server_health() {
    // Once CI is done and the app is activated, status surfaces the backend
    // registry row (DB) plus the runtime-loaded flag (server health).
    use crate::status::{BackendStatus, CiStatus, LocalState, ReleaseStatus, StatusReport};

    let report = StatusReport {
        repo: "aomi-labs/community-apps".to_string(),
        app_release_tag: "apps-my-bot-abc1234".to_string(),
        branch: "publish".to_string(),
        local: LocalState {
            pushed: true,
            deployed: true,
            activated: true,
            updated_at: 0,
        },
        ci: CiStatus::Success {
            name: Some("publish-apps".to_string()),
            url: "https://github.com/aomi-labs/community-apps/actions/runs/1".to_string(),
        },
        release: ReleaseStatus::Available {
            url: "https://github.com/aomi-labs/community-apps/releases/tag/apps-my-bot-abc1234"
                .to_string(),
            assets: 2,
        },
        backend: BackendStatus::Found {
            backend: "https://staging-api.aomi.dev".to_string(),
            registered: true,
            is_active: Some(true),
            visibility: Some("private".to_string()),
            loaded: true,
        },
    };

    let rendered = report.render();
    assert!(rendered.contains("staging-api.aomi.dev"), "{rendered}");
    assert!(rendered.contains("db row"), "{rendered}");
    assert!(rendered.contains("registered=true"), "{rendered}");
    assert!(rendered.contains("active=true"), "{rendered}");
    assert!(rendered.contains("visibility=private"), "{rendered}");
    assert!(rendered.contains("serving on this backend"), "{rendered}");
    // Already activated, so no "request activation" nudge.
    assert!(!rendered.contains("Request activation"), "{rendered}");
}

#[test]
fn status_report_renders_not_activated_when_backend_has_no_row() {
    use crate::status::{BackendStatus, CiStatus, LocalState, ReleaseStatus, StatusReport};

    let report = StatusReport {
        repo: "aomi-labs/community-apps".to_string(),
        app_release_tag: "apps-my-bot-abc1234".to_string(),
        branch: "publish".to_string(),
        local: LocalState {
            pushed: true,
            deployed: true,
            activated: false,
            updated_at: 0,
        },
        ci: CiStatus::Success {
            name: None,
            url: "https://github.com/aomi-labs/community-apps/actions/runs/1".to_string(),
        },
        release: ReleaseStatus::Available {
            url: "https://github.com/aomi-labs/community-apps/releases/tag/apps-my-bot-abc1234"
                .to_string(),
            assets: 1,
        },
        backend: BackendStatus::NotRegistered {
            backend: "https://staging-api.aomi.dev".to_string(),
        },
    };

    let rendered = report.render();
    assert!(rendered.contains("not activated yet"), "{rendered}");
    assert!(rendered.contains("staging-api.aomi.dev"), "{rendered}");
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
