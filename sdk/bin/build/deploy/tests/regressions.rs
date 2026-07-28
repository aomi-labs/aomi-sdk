//! Pins on bugs that shipped: each of these failed before its fix.

use super::*;

#[test]
fn config_update_merges_onto_disk_instead_of_clobbering() {
    // The wizard loads the config, then `ensure_logged_in` may persist a fresh
    // CLI bearer through its own handle. Saving the wizard's pre-login snapshot
    // wholesale used to drop that bearer, so a first-time user was sent through
    // the browser login twice.
    use crate::deploy::config::AomiConfig;
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
    use crate::deploy::types::BuildActivateInput;

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
    use crate::deploy::types::BuildDeployResult;

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
    use crate::deploy::types::ActivatedApp;

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
