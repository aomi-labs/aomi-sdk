mod common;

use aomi_sdk::{
    AOMI_DYN_EXEC_CANCEL, AOMI_DYN_EXEC_POLL, AOMI_FREE_STRING, AsyncExecPool, DynExecCancel,
    DynFreeStringFn, DynToolCancelFn, DynToolPollFn, DynToolResult, DynToolStart, DynToolStartFn,
    SYM_AOMI_ASYNC_TOOL_START,
};
use libloading::Library;
use std::ffi::CStr;

fn read_and_free_string(raw: *mut std::ffi::c_char, free_string: DynFreeStringFn) -> String {
    assert!(
        !raw.is_null(),
        "ffi function should return a non-null string"
    );
    let text = unsafe { CStr::from_ptr(raw) }
        .to_str()
        .expect("ffi string should be utf-8")
        .to_string();
    unsafe { free_string(raw) };
    text
}

#[test]
fn malformed_requests_return_ready_errors() {
    let handle = common::load_dyn_hello();

    let invalid_args = handle
        .call_tool_start(
            "greet",
            "{not-json",
            r#"{"session_id":"ffi-bad","tool_name":"greet","call_id":"bad-args","state_attributes":{}}"#,
        )
        .expect("invalid args should still produce a JSON envelope");
    assert!(matches!(
        invalid_args,
        DynToolStart::Ready {
            result: DynToolResult::Err(_)
        }
    ));

    let invalid_ctx = handle
        .call_tool_start("greet", r#"{"name":"Alice"}"#, "{not-json")
        .expect("invalid ctx should still produce a JSON envelope");
    assert!(matches!(
        invalid_ctx,
        DynToolStart::Ready {
            result: DynToolResult::Err(_)
        }
    ));

    let unknown = handle
        .call_tool_start(
            "does_not_exist",
            "{}",
            r#"{"session_id":"ffi-bad","tool_name":"does_not_exist","call_id":"unknown","state_attributes":{}}"#,
        )
        .expect("unknown tool should still produce a JSON envelope");
    assert!(matches!(
        unknown,
        DynToolStart::Ready {
            result: DynToolResult::Err(_)
        }
    ));
}

#[test]
fn async_failures_surface_as_terminal_error_events() {
    let handle = common::load_dyn_hello();
    let start = handle
        .call_tool_start(
            "fail_async",
            r#"{"message":"boom","tag":"ffi-fail"}"#,
            r#"{"session_id":"ffi-fail","tool_name":"fail_async","call_id":"fail-1","state_attributes":{}}"#,
        )
        .expect("start should succeed");

    let execution_id = match start {
        DynToolStart::AsyncQueued { execution_id } => execution_id,
        other => panic!("expected queued async failure, got {other:?}"),
    };

    match handle
        .call_tool_poll(execution_id)
        .expect("poll should succeed")
    {
        AsyncExecPool::Pending => {
            std::thread::sleep(std::time::Duration::from_millis(10));
            match handle
                .call_tool_poll(execution_id)
                .expect("second poll should succeed")
            {
                AsyncExecPool::Error { message } => {
                    assert!(message.contains("boom"));
                    assert!(message.contains("ffi-fail"));
                }
                other => panic!("expected async error, got {other:?}"),
            }
        }
        AsyncExecPool::Error { message } => {
            assert!(message.contains("boom"));
            assert!(message.contains("ffi-fail"));
        }
        other => panic!("expected pending or error, got {other:?}"),
    }

    assert!(matches!(
        handle
            .call_tool_poll(execution_id)
            .expect("errored execution should be removed"),
        AsyncExecPool::NotFound
    ));
}

#[test]
fn panic_sync_is_contained_inside_the_ffi_start_envelope() {
    let handle = common::load_dyn_hello();
    let start = handle
        .call_tool_start(
            "panic_sync",
            "{}",
            r#"{"session_id":"ffi-panic","tool_name":"panic_sync","call_id":"panic-1","state_attributes":{}}"#,
        )
        .expect("panic should be converted into an error envelope");

    match start {
        DynToolStart::Ready {
            result: DynToolResult::Err(message),
        } => {
            assert!(message.contains("plugin panicked during start of tool 'panic_sync'"));
        }
        other => panic!("expected panic to be contained as a ready error, got {other:?}"),
    }
}

#[test]
fn raw_null_pointer_entrypoints_return_well_formed_envelopes() {
    common::ensure_dyn_hello_built();
    let path = common::dyn_hello_path();
    let library = unsafe { Library::new(&path) }.expect("failed to open dyn-hello library");

    unsafe {
        let start = *library
            .get::<DynToolStartFn>(SYM_AOMI_ASYNC_TOOL_START)
            .expect("missing aomi_async_tool_start");
        let poll = *library
            .get::<DynToolPollFn>(AOMI_DYN_EXEC_POLL)
            .expect("missing aomi_dyn_exec_poll");
        let cancel = *library
            .get::<DynToolCancelFn>(AOMI_DYN_EXEC_CANCEL)
            .expect("missing aomi_dyn_exec_cancel");
        let free_string = *library
            .get::<DynFreeStringFn>(AOMI_FREE_STRING)
            .expect("missing aomi_free_string");

        let start_json = read_and_free_string(
            start(
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            ),
            free_string,
        );
        let start_envelope: DynToolStart =
            serde_json::from_str(&start_json).expect("start envelope should deserialize");
        assert!(matches!(
            start_envelope,
            DynToolStart::Ready {
                result: DynToolResult::Err(_)
            }
        ));

        let poll_json = read_and_free_string(poll(std::ptr::null_mut(), 1), free_string);
        let poll_envelope: AsyncExecPool =
            serde_json::from_str(&poll_json).expect("poll envelope should deserialize");
        assert!(matches!(poll_envelope, AsyncExecPool::Error { .. }));

        let cancel_json = read_and_free_string(cancel(std::ptr::null_mut(), 1), free_string);
        let cancel_envelope: DynExecCancel =
            serde_json::from_str(&cancel_json).expect("cancel envelope should deserialize");
        assert!(!cancel_envelope.canceled);
    }
}
