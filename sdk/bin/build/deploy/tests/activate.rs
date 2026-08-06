//! Building activation requests and folding responses back into local state.

use super::*;

// ── activate: request building ──────────────────────────────────────────────

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

#[test]
fn activation_accepts_manager_v2_and_builder_shapes() {
    let mut manager: ActivateResult = serde_json::from_value(json!({
        "ok": true,
        "activation": {
            "status": "activating",
            "platform": "krexa",
            "target": {
                "kind": "release_tags",
                "value": ["apps-1-r00a1b2c3d4-bot-abc1234"]
            },
            "apps": [{
                "application_id": 7,
                "name": "bot",
                "path": "apps/1/r00a1b2c3d4/bot",
                "release_tag": "apps-1-r00a1b2c3d4-bot-abc1234",
                "is_active": true,
                "loaded": true,
                "error": null,
                "platform_branch": "publish",
                "activation_status": "promoted"
            }]
        }
    }))
    .unwrap();
    assert!(!manager.activation.apps[0].artifact_ready);
    assert_eq!(
        manager.activation.apps[0].platform_branch.as_deref(),
        Some("publish")
    );

    // The direct activation path verifies the live app projection before it
    // folds the response into local state.
    manager.activation.apps[0].artifact_ready = true;
    let mut state = sample_state();
    state.apply_target_activation(&manager);
    assert_eq!(state.deployment.platform.apps[0].activated, Some(true));
    assert!(state.state.ci_passed);

    let builder: ActivateResult = serde_json::from_value(json!({
        "ok": true,
        "activation": {
            "status": "activating",
            "platform": "krexa",
            "target": {
                "kind": "release_tags",
                "value": ["apps-1-r00a1b2c3d4-bot-abc1234"]
            },
            "apps": [{
                "applicationId": 7,
                "name": "bot",
                "path": "apps/1/r00a1b2c3d4/bot",
                "releaseTag": "apps-1-r00a1b2c3d4-bot-abc1234",
                "isActive": true,
                "artifactReady": true,
                "loaded": true,
                "error": null,
                "platformBranch": "publish",
                "activationStatus": "promoted"
            }]
        }
    }))
    .unwrap();
    assert!(builder.activation.apps[0].artifact_ready);
    assert_eq!(
        builder.activation.apps[0].platform_branch.as_deref(),
        Some("publish")
    );
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
