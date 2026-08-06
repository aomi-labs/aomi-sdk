//! What the clap surface accepts: flags, subcommands, and the error text a
//! missing prerequisite prints.

use super::*;

// ── deploy: arg parsing ─────────────────────────────────────────────────────

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
            Some(crate::deploy::cli::deploy::DeployCmd::Preflight(step)) => {
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
            Some(crate::deploy::cli::deploy::DeployCmd::Status(status)) => assert!(status.json),
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
        "--project-id",
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
            assert_eq!(args.step.project_id, Some(626));
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
            Some(crate::deploy::cli::deploy::DeployCmd::Activate(activate)) => {
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
            Some(crate::deploy::cli::deploy::DeployCmd::Status(status)) => {
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
        "project",
        "create",
        "--platform",
        "community",
        "--repo",
        "aomi-labs/playground-example",
        "--backend",
        "https://api.aomi.dev",
        "--activation-token",
        "aat_live",
    ])
    .expect("parse project flags");
    match cli.cmd {
        Some(crate::Cmd::Project(args)) => match args.cmd {
            crate::deploy::cli::project::ProjectCmd::Create(create) => {
                assert_eq!(create.backend.as_deref(), Some("https://api.aomi.dev"));
                assert_eq!(create.activation_token.as_deref(), Some("aat_live"));
            }
        },
        _ => panic!("expected project create"),
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
            crate::deploy::cli::token::TokenCmd::Mint(mint) => {
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
    let backend = crate::deploy::cli::shared::missing_backend("deploy").to_string();
    assert!(backend.contains("deploy --backend <url>"));
    assert!(backend.contains("export AOMI_BACKEND_URL=<url>"));

    let token = crate::deploy::cli::shared::missing_activation_token("deploy activate").to_string();
    assert!(token.contains("deploy activate --activation-token <token>"));
    assert!(token.contains("export AOMI_APP_ACTIVATION_TOKEN=<token>"));

    let admin_key = crate::deploy::cli::shared::missing_admin_key("token mint").to_string();
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
        project_id: Some(1065),
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "platform": "somm.finance",
            "repo": "peggyjv/somm-agent",
            "sourceRef": "abc1234",
            "projectId": 1065
        })
    );
}

#[test]
fn backend_environment_maps_to_matching_build_frontend() {
    assert_eq!(
        crate::deploy::cli::shared::infer_build_url("https://api-staging.aomi.dev"),
        Some("https://build-staging.aomi.dev".into())
    );
    assert_eq!(
        crate::deploy::cli::shared::infer_build_url("https://api.aomi.dev/"),
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
