//! `.aomi/deployment.json` round-trips and the source id recorded in it.

use super::*;

// ── deployment.json round-trip via the state file ───────────────────────────

#[test]
fn project_apps_record_application_ids_in_deployment_json() {
    // Build BFF `GET /api/build/deployments/apps?projectId=…` response.
    let apps: crate::deploy::types::ProjectAppsResult = serde_json::from_value(json!({
        "ok": true,
        "projectId": 42,
        "state": "live",
        "apps": [
            { "id": 2937816, "name": "bot", "is_active": true, "loaded": true,
              "app_release_tag": "apps-1-r00a1b2c3d4-bot-abc1234" },
            { "id": 99, "name": "other-app", "is_active": true, "loaded": true,
              "app_release_tag": null }
        ]
    }))
    .unwrap();
    let mut state = sample_state();
    state.apply_live_attempt();
    state.apply_application_ids(&apps.apps);
    assert_eq!(
        state.deployment.platform.apps[0].application_id,
        Some(2937816)
    );
    assert_eq!(state.deployment.platform.apps[1].application_id, None);

    let repo = TestRepo::new();
    state.write(repo.root()).unwrap();
    let raw: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repo.root().join(".aomi/deployment.json")).unwrap())
            .unwrap();
    assert_eq!(raw["platform"]["apps"][0]["application_id"], 2937816);
    assert!(raw["platform"]["apps"][1].get("application_id").is_none());
    assert_eq!(LocalDeployment::read(repo.root()).unwrap().unwrap(), state);
}

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
fn deploy_records_project_id_and_round_trips() {
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
                "platform_branch": "a/b/1/abc1234", "deploy_branch": "main",
                "apps": [
                    { "name": "bot", "path": "apps/1/r0/bot", "aomi_toml_path": "aomi.toml", "release_tag": "apps-1-r0-bot-abc1234" }
                ]
            }
        }
    }))
    .unwrap();

    let state = LocalDeployment::from_deploy(resp, 219);
    assert_eq!(state.project_id, 219);

    // Persists into .aomi/deployment.json and reads back intact.
    let repo = TestRepo::new();
    state.write(repo.root()).unwrap();
    assert_eq!(
        LocalDeployment::read(repo.root())
            .unwrap()
            .unwrap()
            .project_id,
        219
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
