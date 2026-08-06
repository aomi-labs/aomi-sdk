//! Resolving a deploy's source commit and canonical Project configuration.

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
        source_ref: "0badc0de".to_string(),
        preflight: true,
    };

    assert_eq!(
        serde_json::to_value(&input).unwrap(),
        json!({
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

    let config = ProjectConfig::load(repo.root()).unwrap();
    assert_eq!(config.platform(), &Platform::community());
    assert_eq!(config.applications(), ["aomi.toml", "apps/bot/aomi.toml"]);
}

#[test]
fn project_config_rejects_missing_and_unsafe_manifests() {
    let repo = TestRepo::new();
    repo.write_aomi_toml("", "root");
    repo.commit("init");
    repo.write_project_config(&["../escape/aomi.toml"]);
    assert!(ProjectConfig::load(repo.root()).is_err());
    repo.write_project_config(&["apps/missing/aomi.toml"]);
    assert!(ProjectConfig::load(repo.root()).is_err());
}

// ── project create: singular platform config ────────────────────────────────

#[test]
fn project_config_owns_platform_and_ignores_retired_manifest_field() {
    let repo = TestRepo::new();
    repo.write("aomi.toml", "[app]\nname = \"x\"\nplatform = \"krexa\"\n");
    repo.write_project_config(&["aomi.toml"]);
    repo.commit("init");

    let app = crate::deploy::app::AomiAppFiles::discover(repo.root(), repo.root()).unwrap();
    assert_eq!(app.name, "x");
    assert_eq!(
        ProjectConfig::load(repo.root()).unwrap().platform(),
        &Platform::community()
    );
}

#[test]
fn project_create_discovers_manifests_and_rejects_platform_changes() {
    let repo = TestRepo::new();
    repo.write("aomi.toml", "[app]\nname = \"root\"\n");
    repo.write("apps/bot/aomi.toml", "[app]\nname = \"bot\"\n");
    let platform = Platform::new("somm.finance");
    let (config, path) = ProjectConfig::create(repo.root(), &platform).unwrap();
    assert_eq!(path, repo.path(".aomi/config.json"));
    assert_eq!(config.platform(), &platform);
    assert_eq!(config.applications(), ["aomi.toml", "apps/bot/aomi.toml"]);
    assert!(ProjectConfig::create(repo.root(), &Platform::community()).is_err());
}
