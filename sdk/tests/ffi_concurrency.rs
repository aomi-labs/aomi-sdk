mod common;

use aomi_sdk::{AsyncExecPool, DynToolStart};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[test]
fn concurrent_cross_ffi_calls_stay_isolated_on_one_plugin_instance() {
    let handle = common::load_dyn_hello();
    let exec_ids = Arc::new(Mutex::new(Vec::new()));
    let outputs = Arc::new(Mutex::new(BTreeMap::<String, Vec<(i64, bool, String)>>::new()));

    std::thread::scope(|scope| {
        for idx in 0..12 {
            let handle = handle.clone();
            let exec_ids = exec_ids.clone();
            let outputs = outputs.clone();

            scope.spawn(move || {
                let call_id = format!("ffi-conc-{idx}");
                let tag = format!("tag-{idx}");
                let args_json = json!({
                    "upto": 4,
                    "delay_ms": 2,
                    "tag": tag,
                })
                .to_string();
                let ctx_json = json!({
                    "session_id": "ffi-concurrency",
                    "tool_name": "count_async",
                    "call_id": call_id,
                    "state_attributes": {},
                })
                .to_string();

                let start = handle
                    .call_tool_start("count_async", &args_json, &ctx_json)
                    .expect("start should succeed");
                let execution_id = match start {
                    DynToolStart::AsyncQueued { execution_id } => execution_id,
                    other => panic!("expected queued start, got {other:?}"),
                };
                exec_ids.lock().unwrap().push(execution_id);

                loop {
                    match handle.call_tool_poll(execution_id).expect("poll should succeed") {
                        AsyncExecPool::Pending => std::thread::sleep(Duration::from_millis(5)),
                        AsyncExecPool::Update { value, has_more } => {
                            let call_id = value
                                .get("call_id")
                                .and_then(|value| value.as_str())
                                .expect("call_id should be present")
                                .to_string();
                            let count = value
                                .get("count")
                                .and_then(|value| value.as_i64())
                                .expect("count should be present");
                            let tag = value
                                .get("tag")
                                .and_then(|value| value.as_str())
                                .expect("tag should be present")
                                .to_string();
                            outputs
                                .lock()
                                .unwrap()
                                .entry(call_id)
                                .or_default()
                                .push((count, has_more, tag));

                            if !has_more {
                                break;
                            }
                        }
                        other => panic!("expected pending/update while polling concurrent call, got {other:?}"),
                    }
                }
            });
        }
    });

    let exec_ids = exec_ids.lock().unwrap().clone();
    assert_eq!(exec_ids.len(), 12);
    assert_eq!(exec_ids.iter().copied().collect::<BTreeSet<_>>().len(), 12);

    let outputs = outputs.lock().unwrap().clone();
    assert_eq!(outputs.len(), 12);
    for idx in 0..12 {
        let call_id = format!("ffi-conc-{idx}");
        let expected_tag = format!("tag-{idx}");
        let mut entries = outputs.get(&call_id).cloned().expect("missing outputs for call");
        entries.sort_by_key(|entry| entry.0);
        assert_eq!(
            entries.iter().map(|entry| entry.0).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(entries.iter().filter(|entry| !entry.1).count(), 1);
        assert!(entries.iter().all(|entry| entry.2 == expected_tag));
    }
}

#[test]
fn concurrent_sync_calls_stay_isolated_on_one_plugin_instance() {
    let handle = common::load_dyn_hello();
    let outputs = Arc::new(Mutex::new(BTreeMap::<String, (u64, String, String)>::new()));

    std::thread::scope(|scope| {
        for idx in 0..48 {
            let handle = handle.clone();
            let outputs = outputs.clone();

            scope.spawn(move || {
                let call_id = format!("ffi-sync-{idx}");
                let tag = format!("sync-tag-{idx}");
                let result = handle
                    .call_exec_tool(
                        "slow_sync",
                        &json!({
                            "duration_ms": 25,
                            "tag": tag,
                        })
                        .to_string(),
                        &json!({
                            "session_id": "ffi-sync-concurrency",
                            "tool_name": "slow_sync",
                            "call_id": call_id,
                            "state_attributes": {},
                        })
                        .to_string(),
                    )
                    .expect("slow_sync should succeed");

                let call_id = result
                    .get("call_id")
                    .and_then(|value| value.as_str())
                    .expect("call_id should be present")
                    .to_string();
                let slept_ms = result
                    .get("slept_ms")
                    .and_then(|value| value.as_u64())
                    .expect("slept_ms should be present");
                let tag = result
                    .get("tag")
                    .and_then(|value| value.as_str())
                    .expect("tag should be present")
                    .to_string();
                let session_id = result
                    .get("session_id")
                    .and_then(|value| value.as_str())
                    .expect("session_id should be present")
                    .to_string();

                outputs
                    .lock()
                    .unwrap()
                    .insert(call_id, (slept_ms, tag, session_id));
            });
        }
    });

    let outputs = outputs.lock().unwrap().clone();
    assert_eq!(outputs.len(), 48);
    for idx in 0..48 {
        let call_id = format!("ffi-sync-{idx}");
        let expected_tag = format!("sync-tag-{idx}");
        let (slept_ms, tag, session_id) = outputs.get(&call_id).expect("missing sync result");
        assert_eq!(*slept_ms, 25);
        assert_eq!(tag, &expected_tag);
        assert_eq!(session_id, "ffi-sync-concurrency");
    }
}

#[test]
fn mixed_sync_and_async_calls_share_one_plugin_instance_safely() {
    let handle = common::load_dyn_hello();
    let sync_outputs = Arc::new(Mutex::new(BTreeMap::<String, String>::new()));
    let async_outputs = Arc::new(Mutex::new(BTreeMap::<String, Vec<(i64, bool, String)>>::new()));

    std::thread::scope(|scope| {
        for idx in 0..12 {
            let handle = handle.clone();
            let sync_outputs = sync_outputs.clone();
            scope.spawn(move || {
                let call_id = format!("ffi-mixed-sync-{idx}");
                let tag = format!("mixed-sync-tag-{idx}");
                let result = handle
                    .call_exec_tool(
                        "slow_sync",
                        &json!({
                            "duration_ms": 20,
                            "tag": tag,
                        })
                        .to_string(),
                        &json!({
                            "session_id": "ffi-mixed",
                            "tool_name": "slow_sync",
                            "call_id": call_id,
                            "state_attributes": {},
                        })
                        .to_string(),
                    )
                    .expect("slow_sync should succeed");

                sync_outputs.lock().unwrap().insert(
                    result
                        .get("call_id")
                        .and_then(|value| value.as_str())
                        .expect("call_id should be present")
                        .to_string(),
                    result
                        .get("tag")
                        .and_then(|value| value.as_str())
                        .expect("tag should be present")
                        .to_string(),
                );
            });
        }

        for idx in 0..12 {
            let handle = handle.clone();
            let async_outputs = async_outputs.clone();
            scope.spawn(move || {
                let call_id = format!("ffi-mixed-async-{idx}");
                let tag = format!("mixed-async-tag-{idx}");
                let start = handle
                    .call_tool_start(
                        "count_async",
                        &json!({
                            "upto": 3,
                            "delay_ms": 3,
                            "tag": tag,
                        })
                        .to_string(),
                        &json!({
                            "session_id": "ffi-mixed",
                            "tool_name": "count_async",
                            "call_id": call_id,
                            "state_attributes": {},
                        })
                        .to_string(),
                    )
                    .expect("async start should succeed");

                let execution_id = match start {
                    DynToolStart::AsyncQueued { execution_id } => execution_id,
                    other => panic!("expected queued start, got {other:?}"),
                };

                loop {
                    match handle.call_tool_poll(execution_id).expect("poll should succeed") {
                        AsyncExecPool::Pending => std::thread::sleep(Duration::from_millis(5)),
                        AsyncExecPool::Update { value, has_more } => {
                            let call_id = value
                                .get("call_id")
                                .and_then(|value| value.as_str())
                                .expect("call_id should be present")
                                .to_string();
                            let count = value
                                .get("count")
                                .and_then(|value| value.as_i64())
                                .expect("count should be present");
                            let tag = value
                                .get("tag")
                                .and_then(|value| value.as_str())
                                .expect("tag should be present")
                                .to_string();
                            async_outputs
                                .lock()
                                .unwrap()
                                .entry(call_id)
                                .or_default()
                                .push((count, has_more, tag));
                            if !has_more {
                                break;
                            }
                        }
                        other => panic!("expected pending/update while polling mixed async call, got {other:?}"),
                    }
                }
            });
        }
    });

    let sync_outputs = sync_outputs.lock().unwrap().clone();
    assert_eq!(sync_outputs.len(), 12);
    for idx in 0..12 {
        let call_id = format!("ffi-mixed-sync-{idx}");
        let expected_tag = format!("mixed-sync-tag-{idx}");
        assert_eq!(
            sync_outputs.get(&call_id).expect("missing mixed sync result"),
            &expected_tag
        );
    }

    let async_outputs = async_outputs.lock().unwrap().clone();
    assert_eq!(async_outputs.len(), 12);
    for idx in 0..12 {
        let call_id = format!("ffi-mixed-async-{idx}");
        let expected_tag = format!("mixed-async-tag-{idx}");
        let mut entries = async_outputs
            .get(&call_id)
            .cloned()
            .expect("missing mixed async result");
        entries.sort_by_key(|entry| entry.0);
        assert_eq!(
            entries.iter().map(|entry| entry.0).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
        assert_eq!(entries.iter().filter(|entry| !entry.1).count(), 1);
        assert!(entries.iter().all(|entry| entry.2 == expected_tag));
    }
}
