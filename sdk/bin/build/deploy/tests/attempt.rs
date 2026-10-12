//! The CLI starts and follows the same deployment attempt the Build portal
//! does, and only calls a deployment live once the attempt verified it.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;
use std::time::Duration;

use super::*;
use crate::deploy::attempt::{self, AttemptOutcome};
use crate::deploy::build_client::BuildClient;
use crate::deploy::state::AttemptRef;
use crate::deploy::types::Attempt;

const COMMIT: &str = "f5e993cba5bc00000000000000000000000000aa";

/// One request as the mock Build server saw it.
struct Seen {
    request_line: String,
    authorization: Option<String>,
    body: String,
}

/// A one-connection-per-response Build stand-in. Answers the queued
/// `(status, body)` pairs in order, then returns what it saw.
fn mock_build(responses: Vec<(u16, String)>) -> (String, JoinHandle<Vec<Seen>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for (status, body) in responses {
            let (mut stream, _) = listener.accept().unwrap();
            let mut raw = Vec::new();
            let mut buf = [0u8; 4096];
            let (head, mut rest) = loop {
                let n = stream.read(&mut buf).unwrap();
                raw.extend_from_slice(&buf[..n]);
                if let Some(at) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                    break (
                        String::from_utf8_lossy(&raw[..at]).to_string(),
                        raw[at + 4..].to_vec(),
                    );
                }
            };
            let length = head
                .lines()
                .find_map(|line| {
                    line.to_ascii_lowercase()
                        .strip_prefix("content-length:")
                        .and_then(|v| v.trim().parse::<usize>().ok())
                })
                .unwrap_or(0);
            while rest.len() < length {
                let n = stream.read(&mut buf).unwrap();
                rest.extend_from_slice(&buf[..n]);
            }
            seen.push(Seen {
                request_line: head.lines().next().unwrap_or_default().to_string(),
                authorization: head.lines().find_map(|line| {
                    line.to_ascii_lowercase()
                        .starts_with("authorization:")
                        .then(|| line.split_once(':').unwrap().1.trim().to_string())
                }),
                body: String::from_utf8_lossy(&rest).to_string(),
            });
            write!(
                stream,
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
        seen
    });
    (url, handle)
}

fn job(name: &str, status: &str, conclusion: Option<&str>) -> serde_json::Value {
    json!({ "name": name, "status": status, "conclusion": conclusion, "steps": [] })
}

fn live_jobs() -> Vec<serde_json::Value> {
    vec![
        job("Validate", "completed", Some("success")),
        job("Build / playground-example", "completed", Some("success")),
        job("Publish release", "completed", Some("success")),
        job("Activate", "completed", Some("success")),
        job(
            "Verify runtime / playground-example",
            "completed",
            Some("success"),
        ),
    ]
}

fn attempt_json(
    status: &str,
    conclusion: Option<&str>,
    jobs: Vec<serde_json::Value>,
) -> serde_json::Value {
    json!({ "attempt": {
        "id": 901, "commit": COMMIT, "branch": "sdk-5.1.1", "status": status,
        "conclusion": conclusion, "url": "https://github.com/aomi-labs/community-apps/actions/runs/901",
        "jobs": jobs, "diagnostics": []
    }})
}

fn parsed(value: serde_json::Value) -> Attempt {
    serde_json::from_value(value["attempt"].clone()).unwrap()
}

#[tokio::test]
async fn start_attempt_makes_the_portals_start_call_with_the_builder_bearer() {
    let (url, server) = mock_build(vec![(
        202,
        json!({ "attempt": { "id": 901, "commit": COMMIT, "branch": "sdk-5.1.1",
            "status": "queued", "conclusion": null, "url": "https://example.test/run/901" },
            "existing": false })
        .to_string(),
    )]);
    let client = BuildClient::new(url, "builder-token").unwrap();

    let started = client.start_attempt(1657, "sdk-5.1.1").await.unwrap();

    assert_eq!(started.attempt.id, 901);
    assert!(!started.existing);
    let seen = server.join().unwrap();
    assert_eq!(
        seen[0].request_line,
        "POST /api/bff/launch/attempts HTTP/1.1"
    );
    assert_eq!(
        seen[0].authorization.as_deref(),
        Some("Bearer builder-token")
    );
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&seen[0].body).unwrap(),
        json!({ "action": "start", "projectId": 1657, "branch": "sdk-5.1.1" })
    );
}

#[tokio::test]
async fn attempt_read_asks_for_the_run_of_the_project() {
    let (url, server) = mock_build(vec![(
        200,
        attempt_json("queued", None, vec![]).to_string(),
    )]);
    let client = BuildClient::new(url, "t").unwrap();

    let read = client.attempt(1657, 901).await.unwrap();

    assert_eq!(read.attempt.status, "queued");
    assert_eq!(
        server.join().unwrap()[0].request_line,
        "GET /api/bff/launch/attempts?projectId=1657&runId=901 HTTP/1.1"
    );
}

#[tokio::test]
async fn a_refused_start_surfaces_the_build_error_instead_of_a_phantom_attempt() {
    let (url, _server) = mock_build(vec![(
        502,
        json!({ "error": "Platform deployment workflow is unavailable" }).to_string(),
    )]);
    let client = BuildClient::new(url, "t").unwrap();

    let error = client.start_attempt(1657, "sdk-5.1.1").await.unwrap_err();

    assert!(error.to_string().contains("502"), "{error}");
    assert!(
        error.to_string().contains("workflow is unavailable"),
        "{error}"
    );
}

#[tokio::test]
async fn follow_reports_live_only_after_activation_and_verification_succeed() {
    let (url, server) = mock_build(vec![
        (
            200,
            attempt_json(
                "in_progress",
                None,
                vec![
                    job("Validate", "completed", Some("success")),
                    job("Build / playground-example", "in_progress", None),
                ],
            )
            .to_string(),
        ),
        (
            200,
            attempt_json("completed", Some("success"), live_jobs()).to_string(),
        ),
    ]);
    let client = BuildClient::new(url, "t").unwrap();
    let mut states = Vec::new();

    let outcome = attempt::follow_every(
        &client,
        1657,
        901,
        Duration::from_secs(30),
        Duration::from_millis(10),
        |state| states.push(state.to_string()),
    )
    .await
    .unwrap();

    assert_eq!(outcome, AttemptOutcome::Live);
    assert_eq!(states, ["Build / playground-example", "completed"]);
    assert_eq!(server.join().unwrap().len(), 2);
}

#[tokio::test]
async fn follow_names_the_failed_stage_and_step_with_diagnostics() {
    let mut failed_build = job("Build / playground-example", "completed", Some("failure"));
    failed_build["steps"] = json!([
        { "name": "Set up job", "conclusion": "success" },
        { "name": "cargo build --locked", "conclusion": "failure" }
    ]);
    let mut body = attempt_json(
        "completed",
        Some("failure"),
        vec![
            job("Validate", "completed", Some("success")),
            failed_build,
            job("Publish release", "completed", Some("skipped")),
        ],
    );
    body["attempt"]["diagnostics"] = json!(["error: failed to select a version for `aomi-sdk`"]);
    let (url, _server) = mock_build(vec![(200, body.to_string())]);
    let client = BuildClient::new(url, "t").unwrap();

    let outcome = attempt::follow_every(
        &client,
        1657,
        901,
        Duration::from_secs(30),
        Duration::from_millis(10),
        |_| {},
    )
    .await
    .unwrap();

    let AttemptOutcome::Failed(detail) = outcome else {
        panic!("expected a failed attempt");
    };
    assert!(
        detail.contains("stage `Build / playground-example` failure"),
        "{detail}"
    );
    assert!(
        detail.contains("at step `cargo build --locked`"),
        "{detail}"
    );
    assert!(detail.contains("failed to select a version"), "{detail}");
    assert!(detail.contains("actions/runs/901"), "{detail}");
    assert!(
        !detail.contains("Publish release"),
        "skipped job is not a failure: {detail}"
    );
}

#[test]
fn a_green_run_that_never_activated_or_verified_is_not_live() {
    let mut jobs = live_jobs();
    jobs[3] = job("Activate", "completed", Some("skipped"));
    let run = parsed(attempt_json("completed", Some("success"), jobs));

    let detail = attempt::verdict(&run).unwrap_err();

    assert!(detail.contains("`Activate`"), "{detail}");

    let no_verify = parsed(attempt_json(
        "completed",
        Some("success"),
        live_jobs().into_iter().take(4).collect(),
    ));
    assert!(
        attempt::verdict(&no_verify)
            .unwrap_err()
            .contains("`Verify runtime`")
    );
    assert!(
        attempt::verdict(&parsed(attempt_json(
            "completed",
            Some("success"),
            live_jobs()
        )))
        .is_ok()
    );
}

#[test]
fn a_cancelled_attempt_reports_the_cancellation_not_success() {
    let run = parsed(attempt_json(
        "completed",
        Some("cancelled"),
        vec![job(
            "Build / playground-example",
            "completed",
            Some("cancelled"),
        )],
    ));

    let detail = attempt::verdict(&run).unwrap_err();

    assert!(
        detail.starts_with("deployment attempt cancelled"),
        "{detail}"
    );
    assert_eq!(attempt::stage(&run), "completed (cancelled)");
}

#[test]
fn only_an_attempt_building_this_commit_is_adopted() {
    let building = parsed(attempt_json("queued", None, vec![]));
    assert!(attempt::adopt(&building, false, COMMIT).is_ok());
    // Joining an attempt someone else started is fine when it builds our commit.
    assert!(attempt::adopt(&building, true, COMMIT).is_ok());

    let elsewhere = "0000000000000000000000000000000000000001";
    let other = attempt::adopt(&building, true, elsewhere)
        .unwrap_err()
        .to_string();
    assert!(other.contains("already running"), "{other}");
    let wrong = attempt::adopt(&building, false, elsewhere)
        .unwrap_err()
        .to_string();
    assert!(wrong.contains("not the commit being deployed"), "{wrong}");

    let mut unknown = building.clone();
    unknown.commit = Some(String::new());
    assert!(attempt::adopt(&unknown, false, COMMIT).is_err());
}

#[test]
fn the_recorded_attempt_round_trips_and_older_state_files_still_read() {
    let mut state = sample_state();
    assert!(state.attempt.is_none(), "an older record has no attempt");
    assert!(!serde_json::to_string(&state).unwrap().contains("attempt"));

    state.attempt = Some(AttemptRef {
        id: 901,
        commit: COMMIT.into(),
        url: Some("https://example.test/run/901".into()),
    });
    state.apply_live_attempt();
    let repo = TestRepo::new();
    state.write(repo.root()).unwrap();

    let read = LocalDeployment::read(repo.root()).unwrap().unwrap();
    assert_eq!(read, state);
    assert!(read.state.activated && read.state.ci_passed);
    assert!(
        read.deployment
            .platform
            .apps
            .iter()
            .all(|app| app.activated == Some(true))
    );
}
