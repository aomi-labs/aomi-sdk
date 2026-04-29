//! E2E integration tests against the live Molinar API.
//!
//! Run with:
//!   MOLINAR_BOT_ID=<uuid> cargo test -p dyn-molinar --test molinar_e2e -- --nocapture
//!
//! If MOLINAR_BOT_ID is not set, tests will attempt to self-provision via
//! POST /api/bot/connect. If that also fails, tests are skipped.

use dyn_molinar::MolinarClient;
use dyn_molinar::types::{CustomizeRequest, ExploreRequest, MoveRequest, PingRequest};
use serde_json::Value;
use std::sync::OnceLock;

static BOT_ID: OnceLock<Option<String>> = OnceLock::new();

/// Try to get a bot_id — first from env, then by calling connect.
fn resolve_bot_id() -> Option<String> {
    BOT_ID
        .get_or_init(|| {
            // 1. env var
            if let Ok(id) = std::env::var("MOLINAR_BOT_ID") {
                if !id.is_empty() {
                    eprintln!("✅ Using MOLINAR_BOT_ID from env: {id}");
                    return Some(id);
                }
            }

            // 2. try self-provision via connect
            eprintln!("⏳ MOLINAR_BOT_ID not set — trying POST /connect …");

            let api = std::env::var("MOLINAR_API_ENDPOINT")
                .unwrap_or_else(|_| "https://molinar.ai/api/bot".into());
            let url = format!("{api}/connect");

            let client = reqwest::blocking::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .ok()?;

            let resp = client
                .post(&url)
                .header("Content-Type", "application/json")
                .body("{}")
                .send()
                .ok()?;

            if !resp.status().is_success() {
                eprintln!(
                    "⏭  /connect returned {} — skipping E2E tests (set MOLINAR_BOT_ID to override)",
                    resp.status()
                );
                return None;
            }

            let body: Value = resp.json().ok()?;
            if let Some(id) = body.get("botId").and_then(|v| v.as_str()) {
                eprintln!("✅ Self-provisioned bot_id via /connect: {id}");
                Some(id.to_string())
            } else {
                eprintln!("⏭  /connect returned unexpected body: {body} — skipping");
                None
            }
        })
        .clone()
}

/// Assert a response has the WorldSnapshot shape.
/// Note: `source` is injected by the tool layer, not the client, so we don't
/// require it here — these tests call the client directly.
fn assert_world_snapshot(v: &Value, label: &str) {
    assert!(v.get("me").is_some(), "{label}: missing `me`");
    assert!(v.get("world").is_some(), "{label}: missing `world`");
    assert!(v.get("nearby").is_some(), "{label}: missing `nearby`");
    assert!(
        v.get("allPlayers").is_some(),
        "{label}: missing `allPlayers`"
    );
    assert!(v.get("chat").is_some(), "{label}: missing `chat`");
}

fn print_snapshot_summary(v: &Value, label: &str) {
    let me = &v["me"];
    let pos = &me["position"];
    let world = &v["world"];
    let nearby = &v["nearby"];
    let players_count = v["allPlayers"].as_array().map(|a| a.len()).unwrap_or(0);
    let coins_count = nearby["coins"].as_array().map(|a| a.len()).unwrap_or(0);
    let chat_count = v["chat"].as_array().map(|a| a.len()).unwrap_or(0);

    println!(
        "[{label}] pos=({}, {}) biome={} terrain={} | {} players, {} nearby coins, {} chat msgs",
        pos["x"].as_f64().unwrap_or(0.0),
        pos["z"].as_f64().unwrap_or(0.0),
        world["biome"].as_str().unwrap_or("?"),
        world["terrain"].as_str().unwrap_or("?"),
        players_count,
        coins_count,
        chat_count,
    );
}

#[test]
fn test_get_state() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client.get_state(&bot_id).expect("get_state failed");
    assert_world_snapshot(&res, "get_state");
    print_snapshot_summary(&res, "get_state");
}

#[test]
fn test_look() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client.look(&bot_id).expect("look failed");
    assert_world_snapshot(&res, "look");
    print_snapshot_summary(&res, "look");
}

#[test]
fn test_move_direction() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .move_bot(
            &bot_id,
            &MoveRequest {
                direction: Some("north".to_string()),
                ..MoveRequest::default()
            },
        )
        .expect("move north failed");
    assert_world_snapshot(&res, "move");
    println!("[move north] action = {:?}", res["action"]);
}

#[test]
fn test_move_stop() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .move_bot(
            &bot_id,
            &MoveRequest {
                stop: Some(true),
                ..MoveRequest::default()
            },
        )
        .expect("move stop failed");
    assert_world_snapshot(&res, "move stop");
    println!("[move stop] action = {:?}", res["action"]);
}

#[test]
fn test_move_coordinates() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .move_bot(
            &bot_id,
            &MoveRequest {
                target_x: Some(5.0),
                target_z: Some(-3.0),
                ..MoveRequest::default()
            },
        )
        .expect("move coords failed");
    assert_world_snapshot(&res, "move coords");
    println!("[move coords] action = {:?}", res["action"]);
}

#[test]
fn test_move_raw_input() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .move_bot(
            &bot_id,
            &MoveRequest {
                dx: Some(0.5),
                dz: Some(-0.5),
                ..MoveRequest::default()
            },
        )
        .expect("move dx/dz failed");
    assert_world_snapshot(&res, "move dx/dz");
    println!("[move dx/dz] action = {:?}", res["action"]);
}

#[test]
fn test_jump() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client.jump(&bot_id).expect("jump failed");
    assert_world_snapshot(&res, "jump");
    println!("[jump] action.jumped = {:?}", res["action"]["jumped"]);
}

#[test]
fn test_send_chat() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .send_chat(&bot_id, "hello from aomi e2e!")
        .expect("send_chat failed");
    assert_world_snapshot(&res, "chat send");
    println!("[chat send] action.sent = {:?}", res["action"]["sent"]);
}

#[test]
fn test_get_chat() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client.get_chat(&bot_id).expect("get_chat failed");
    assert_world_snapshot(&res, "get_chat");
    let count = res["chat"].as_array().map(|a| a.len()).unwrap_or(0);
    println!("[get_chat] {} messages in history", count);
}

#[test]
fn test_get_new_messages() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .get_new_messages(&bot_id)
        .expect("get_new_messages failed");
    assert_world_snapshot(&res, "get_new_messages");
    println!("[get_new_messages] action = {:?}", res["action"]);
}

#[test]
fn test_get_players() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client.get_players(&bot_id).expect("get_players failed");
    assert_world_snapshot(&res, "get_players");
    let count = res["allPlayers"].as_array().map(|a| a.len()).unwrap_or(0);
    println!("[get_players] {} players online", count);
}

#[test]
fn test_collect_coins() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client.collect_coins(&bot_id).expect("collect_coins failed");
    assert_world_snapshot(&res, "collect");
    println!(
        "[collect] collected={} movingTo={:?}",
        res["action"]["collected"], res["action"]["movingTo"]
    );
}

#[test]
fn test_explore() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .explore(&bot_id, &ExploreRequest::default())
        .expect("explore failed");
    assert_world_snapshot(&res, "explore");
    print_snapshot_summary(&res, "explore");
}

#[test]
fn test_explore_target() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .explore(
            &bot_id,
            &ExploreRequest {
                target_x: Some(10.0),
                target_z: Some(10.0),
            },
        )
        .expect("explore target failed");
    assert_world_snapshot(&res, "explore target");
    print_snapshot_summary(&res, "explore target");
}

#[test]
fn test_customize() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .customize(
            &bot_id,
            &CustomizeRequest {
                name: Some("AomiE2E".to_string()),
                color: Some("#3498db".to_string()),
            },
        )
        .expect("customize failed");
    assert_world_snapshot(&res, "customize");
    println!(
        "[customize] name={:?} color={:?}",
        res["me"]["name"], res["me"]["color"]
    );
}

#[test]
fn test_ping() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    let res = client
        .ping(&bot_id, &PingRequest::default())
        .expect("ping failed");
    assert_world_snapshot(&res, "ping");
    println!("[ping] action = {:?}", res["action"]);
}

#[test]
fn test_create_object() {
    let Some(bot_id) = resolve_bot_id() else {
        return;
    };
    let client = MolinarClient::new().unwrap();

    // This may be rate limited or slow — treat errors as non-fatal
    match client.create_object(&bot_id, "a small red mushroom") {
        Ok(res) => {
            assert_world_snapshot(&res, "create_object");
            println!("[create_object] action = {:?}", res["action"]);
        }
        Err(e) => {
            println!("[create_object] skipped (may be rate limited): {}", e);
        }
    }
}
