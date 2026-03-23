mod common;

use aomi_sdk::{AsyncExecPool, DynToolResult, DynToolStart};
use serde_json::json;
use std::time::{Duration, Instant};

#[test]
fn sync_tool_returns_ready_without_polling() {
    let handle = common::load_dyn_hello();

    let start = handle
        .call_tool_start(
            "greet",
            r#"{"name":"Alice"}"#,
            r#"{"session_id":"ffi-sync","tool_name":"greet","call_id":"sync-1","state_attributes":{}}"#,
        )
        .expect("start should succeed");

    match start {
        DynToolStart::Ready {
            result: DynToolResult::Ok(value),
        } => {
            assert_eq!(
                value.get("greeting").and_then(|value| value.as_str()),
                Some("Hello, Alice!")
            );
            assert_eq!(
                value.get("session_id").and_then(|value| value.as_str()),
                Some("ffi-sync")
            );
        }
        other => panic!("expected immediate ready result, got {other:?}"),
    }
}

#[test]
fn async_tool_reaches_terminal_update_then_disappears() {
    let handle = common::load_dyn_hello();
    let start = handle
        .call_tool_start(
            "count_async",
            &json!({"upto": 3, "delay_ms": 2, "tag": "lifecycle"}).to_string(),
            r#"{"session_id":"ffi-async","tool_name":"count_async","call_id":"async-1","state_attributes":{}}"#,
        )
        .expect("start should succeed");

    let execution_id = match start {
        DynToolStart::AsyncQueued { execution_id } => execution_id,
        other => panic!("expected queued async result, got {other:?}"),
    };

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut counts = Vec::new();

    loop {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for terminal async update"
        );
        match handle
            .call_tool_poll(execution_id)
            .expect("poll should succeed")
        {
            AsyncExecPool::Pending => std::thread::sleep(Duration::from_millis(5)),
            AsyncExecPool::Update { value, has_more } => {
                counts.push(
                    value
                        .get("count")
                        .and_then(|value| value.as_i64())
                        .expect("count should be present"),
                );
                assert_eq!(
                    value.get("tag").and_then(|value| value.as_str()),
                    Some("lifecycle")
                );
                if !has_more {
                    break;
                }
            }
            other => panic!("expected update or pending, got {other:?}"),
        }
    }

    assert_eq!(counts, vec![1, 2, 3]);
    assert!(matches!(
        handle
            .call_tool_poll(execution_id)
            .expect("terminal execution should be removed"),
        AsyncExecPool::NotFound
    ));
}

#[test]
fn cancel_flow_returns_canceled_then_not_found() {
    let handle = common::load_dyn_hello();
    let start = handle
        .call_tool_start(
            "wait_for_cancel_async",
            &json!({"delay_ms": 5, "tag": "cancel-me"}).to_string(),
            r#"{"session_id":"ffi-cancel","tool_name":"wait_for_cancel_async","call_id":"cancel-1","state_attributes":{}}"#,
        )
        .expect("start should succeed");

    let execution_id = match start {
        DynToolStart::AsyncQueued { execution_id } => execution_id,
        other => panic!("expected queued async result, got {other:?}"),
    };

    let cancel = handle
        .call_tool_cancel(execution_id)
        .expect("cancel should succeed");
    assert!(cancel.canceled);

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for canceled poll"
        );
        match handle
            .call_tool_poll(execution_id)
            .expect("poll should deserialize")
        {
            AsyncExecPool::Pending => std::thread::sleep(Duration::from_millis(5)),
            AsyncExecPool::Canceled => break,
            other => panic!("expected canceled terminal event, got {other:?}"),
        }
    }

    assert!(matches!(
        handle
            .call_tool_poll(execution_id)
            .expect("canceled execution should be removed"),
        AsyncExecPool::NotFound
    ));
}
