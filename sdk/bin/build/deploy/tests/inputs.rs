//! Resolving a deploy's source commit, Project manifest set, and platform.

use super::*;

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
fn source_ref_honors_commit_flag() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("", "root");
    repo.commit("init");

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
        project_id: 42,
        source_ref: "0badc0de".to_string(),
        preflight: true,
    };

    assert_eq!(
        serde_json::to_value(&input).unwrap(),
        json!({
            "project_id": 42,
            "source_ref": "0badc0de",
            "preflight": true
        })
    );
}

// ── deploy: canonical Project configuration ──────────────────────────────────

#[test]
fn project_applications_come_only_from_root_config() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("", "root");
    repo.write_aomi_toml("apps/bot", "bot");
    repo.commit("init");

    let paths = deploy_args(repo.root())
        .project_applications(repo.root())
        .unwrap();
    assert_eq!(paths, vec!["aomi.toml", "apps/bot/aomi.toml"]);
}

#[test]
fn project_config_rejects_missing_and_unsafe_manifests() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("", "root");
    repo.commit("init");
    repo.write_project_config(&["../escape/aomi.toml"]);
    assert!(
        deploy_args(repo.root())
            .project_applications(repo.root())
            .is_err()
    );
    repo.write_project_config(&["apps/missing/aomi.toml"]);
    assert!(
        deploy_args(repo.root())
            .project_applications(repo.root())
            .is_err()
    );
}

// ── deploy: platform resolution ─────────────────────────────────────────────

#[test]
fn platform_ignores_retired_manifest_field() {
    let repo = TestRepo::new();
    repo.write("aomi.toml", "[app]\nname = \"x\"\nplatform = \"krexa\"\n");
    repo.write_project_config(&["aomi.toml"]);
    repo.commit("init");

    let app = crate::deploy::app::AomiAppFiles::discover(repo.root(), repo.root()).unwrap();
    assert_eq!(app.name, "x");
    assert_eq!(
        deploy_args(repo.root()).resolve_platform(Some("somm.finance".into())),
        Platform::new("somm.finance")
    );
    assert_eq!(
        deploy_args(repo.root()).resolve_platform(None),
        Platform::community()
    );
}

#[test]
fn platform_flag_wins_over_saved_config() {
    let repo = TestRepo::new();
    let mut args = deploy_args(repo.root());
    args.platform = Some(Platform::new("somm.finance"));
    let platform = args.resolve_platform(Some("other".into()));
    assert_eq!(platform.as_str(), "somm.finance");
}

#[test]
fn platform_falls_back_to_saved_config_then_community() {
    let repo = TestRepo::new();
    let platform = deploy_args(repo.root()).resolve_platform(Some("somm.finance".into()));
    assert_eq!(platform.as_str(), "somm.finance");

    let platform = deploy_args(repo.root()).resolve_platform(None);
    assert_eq!(platform, Platform::community());
}
