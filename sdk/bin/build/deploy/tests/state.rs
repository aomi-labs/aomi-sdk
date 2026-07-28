//! `.aomi/deployment.json` round-trips and the source id recorded in it.

use super::*;

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
