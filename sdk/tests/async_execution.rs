use std::sync::Arc;

use aomi_sdk::{
    AsyncExecPool, AsyncExecQueue, DynAsyncSink, DynExecCancel, DynToolStart, RouteStep,
    TOOL_RETURN_MARKER, TOOL_RETURN_ROUTES_KEY, ToolReturn,
};
use serde_json::{Value, json};

#[test]
fn exec_envelopes_roundtrip() {
    let start = DynToolStart::AsyncQueued { execution_id: 42 };
    let start_json = serde_json::to_string(&start).unwrap();
    let parsed_start: DynToolStart = serde_json::from_str(&start_json).unwrap();
    assert!(matches!(
        parsed_start,
        DynToolStart::AsyncQueued { execution_id: 42 }
    ));

    let poll = AsyncExecPool::Update {
        value: json!({"step": 1}),
        has_more: false,
    };
    let poll_json = serde_json::to_string(&poll).unwrap();
    let parsed_poll: AsyncExecPool = serde_json::from_str(&poll_json).unwrap();
    assert!(matches!(
        parsed_poll,
        AsyncExecPool::Update {
            has_more: false,
            ..
        }
    ));

    let cancel_json = serde_json::to_string(&DynExecCancel { canceled: true }).unwrap();
    let parsed_cancel: DynExecCancel = serde_json::from_str(&cancel_json).unwrap();
    assert!(parsed_cancel.canceled);
}

#[test]
fn async_sink_pushes_updates() {
    let queue = Arc::new(AsyncExecQueue::default());
    let sink = DynAsyncSink::__from_queue(queue.clone());

    sink.emit(json!({"n": 1})).unwrap();
    sink.complete(json!({"n": 2})).unwrap();

    assert!(matches!(
        queue.poll(),
        AsyncExecPool::Update { has_more: true, .. }
    ));
    assert!(matches!(
        queue.poll(),
        AsyncExecPool::Update {
            has_more: false,
            ..
        }
    ));
}

#[test]
fn async_sink_complete_accepts_routed_tool_returns() {
    let queue = Arc::new(AsyncExecQueue::default());
    let sink = DynAsyncSink::__from_queue(queue.clone());

    sink.complete(routed_return(json!({"status": "awaiting_wallet"})))
        .expect("terminal complete should accept routed ToolReturn");

    match queue.poll() {
        AsyncExecPool::Update { value, has_more } => {
            assert!(!has_more, "complete pushes terminal update");
            let envelope = value.as_object().expect("envelope is a JSON object");
            assert_eq!(
                envelope.get(TOOL_RETURN_MARKER).and_then(Value::as_bool),
                Some(true),
                "envelope marker present in queued value"
            );
            assert!(envelope.get(TOOL_RETURN_ROUTES_KEY).is_some());
        }
        other => panic!("expected Update, got {other:?}"),
    }
}

#[test]
fn async_sink_emit_rejects_routed_tool_returns() {
    let sink = DynAsyncSink::__from_queue(Arc::new(AsyncExecQueue::default()));
    let err = sink
        .emit(routed_return(json!({"progress": 0.5})))
        .expect_err("intermediate emits should reject routed envelopes");

    assert!(
        err.contains("intermediate async updates do not support routed ToolReturn"),
        "error message should explain emit vs complete; got: {err}"
    );
}

fn routed_return(value: Value) -> ToolReturn {
    ToolReturn::with_route(
        value,
        RouteStep::on_return("submit_polymarket_order", json!({"market": "btc"})),
    )
}
