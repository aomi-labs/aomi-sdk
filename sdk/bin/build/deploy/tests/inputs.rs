//! Resolving a deploy's inputs from the working tree: source commit, the
//! `aomi.toml` set, and the destination platform.

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
fn platform_defaults_from_aomi_toml_then_community() {
    let repo = TestRepo::new();
    repo.write("aomi.toml", "[app]\nname = \"x\"\nplatform = \"krexa\"\n");
    repo.write_project_config(&["aomi.toml"]);
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
    let expected = crate::deploy::config::AomiConfig::load()
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
fn platform_prefers_project_manifest_over_saved_config() {
    let repo = TestRepo::new();
    repo.write(
        "apps/gecko/aomi.toml",
        "[app]\nname = \"gecko\"\nplatform = \"krexa\"\n",
    );
    repo.write_project_config(&["apps/gecko/aomi.toml"]);
    repo.commit("init");

    let platform = deploy_args(repo.root())
        .resolve_platform(repo.root(), repo.root(), Some("somm.finance".into()))
        .unwrap();
    assert_eq!(platform.as_str(), "krexa");
}

#[test]
fn platform_flag_wins_over_manifest() {
    let repo = TestRepo::new();
    repo.write("aomi.toml", "[app]\nname = \"x\"\nplatform = \"krexa\"\n");
    repo.write_project_config(&["aomi.toml"]);
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
    repo.write_project_config(&["aomi.toml"]);
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
    repo.write_project_config(&["apps/a/aomi.toml", "apps/b/aomi.toml"]);
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

    // An explicit platform overrides manifest hints for an already-bound Project.
    let mut flagged = deploy_args(repo.root());
    flagged.platform = Some(Platform::new("community"));
    let platform = flagged
        .resolve_platform(repo.root(), repo.root(), None)
        .unwrap();
    assert_eq!(platform, Platform::community());
}
