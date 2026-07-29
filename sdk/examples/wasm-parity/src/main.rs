use serde_json::{Value, json};
use std::{
    ffi::{CStr, CString, c_char, c_void},
    thread,
    time::{Duration, Instant},
};

fn read_owned_json(ptr: *mut c_char) -> Value {
    assert!(!ptr.is_null(), "plugin returned a null JSON pointer");
    let text = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .expect("plugin JSON must be UTF-8")
        .to_owned();
    unsafe { aomi_wasm_parity::aomi_free_string(ptr) };
    serde_json::from_str(&text).expect("plugin response must be JSON")
}

fn start(instance: *mut c_void, name: &str, args: Value, call_id: &str) -> Value {
    let name = CString::new(name).unwrap();
    let args = CString::new(args.to_string()).unwrap();
    let context = CString::new(
        json!({
            "session_id": "parity-session",
            "tool_name": name.to_str().unwrap(),
            "call_id": call_id,
            "state_attributes": {},
            "secrets": {},
        })
        .to_string(),
    )
    .unwrap();

    read_owned_json(unsafe {
        aomi_wasm_parity::aomi_async_tool_start(
            instance,
            name.as_ptr(),
            args.as_ptr(),
            context.as_ptr(),
        )
    })
}

fn collect_async(instance: *mut c_void, execution_id: u64) -> (Vec<Value>, Value) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut updates = Vec::new();

    loop {
        assert!(Instant::now() < deadline, "native async parity timed out");
        let poll = read_owned_json(unsafe {
            aomi_wasm_parity::aomi_dyn_exec_poll(instance, execution_id)
        });
        match poll.get("status").and_then(Value::as_str) {
            Some("pending") => thread::sleep(Duration::from_millis(1)),
            Some("update") => {
                let has_more = poll["has_more"].as_bool().unwrap();
                updates.push(poll);
                if !has_more {
                    break;
                }
            }
            status => panic!("unexpected async poll status: {status:?}"),
        }
    }

    let after_complete =
        read_owned_json(unsafe { aomi_wasm_parity::aomi_dyn_exec_poll(instance, execution_id) });
    (updates, after_complete)
}

fn main() {
    let instance = aomi_wasm_parity::aomi_create();
    assert!(!instance.is_null());

    let sdk_version = unsafe { CStr::from_ptr(aomi_wasm_parity::aomi_sdk_version()) }
        .to_str()
        .unwrap();
    let manifest = read_owned_json(unsafe { aomi_wasm_parity::aomi_manifest(instance) });
    let greet = start(instance, "greet", json!({"name": "Ada"}), "greet-1");
    let invalid_args = start(instance, "greet", json!({"name": 42}), "invalid-1");
    let unknown_tool = start(instance, "missing", json!({}), "missing-1");
    let async_start = start(instance, "count", json!({"upto": 3}), "count-1");
    let execution_id = async_start["execution_id"].as_u64().unwrap();
    let (async_updates, poll_after_complete) = collect_async(instance, execution_id);
    let cancel_start = start(instance, "count", json!({"upto": 3}), "cancel-1");
    let cancel_execution_id = cancel_start["execution_id"].as_u64().unwrap();
    let cancel_active = read_owned_json(unsafe {
        aomi_wasm_parity::aomi_dyn_exec_cancel(instance, cancel_execution_id)
    });
    let poll_canceled = read_owned_json(unsafe {
        aomi_wasm_parity::aomi_dyn_exec_poll(instance, cancel_execution_id)
    });
    let poll_after_cancel = read_owned_json(unsafe {
        aomi_wasm_parity::aomi_dyn_exec_poll(instance, cancel_execution_id)
    });
    let cancel_unknown =
        read_owned_json(unsafe { aomi_wasm_parity::aomi_dyn_exec_cancel(instance, 999_999) });

    unsafe { aomi_wasm_parity::aomi_destroy(instance) };

    println!(
        "{}",
        json!({
            "sdk_version": sdk_version,
            "manifest": manifest,
            "greet": greet,
            "invalid_args": invalid_args,
            "unknown_tool": unknown_tool,
            "async_start": async_start,
            "async_updates": async_updates,
            "poll_after_complete": poll_after_complete,
            "cancel_start": cancel_start,
            "cancel_active": cancel_active,
            "poll_canceled": poll_canceled,
            "poll_after_cancel": poll_after_cancel,
            "cancel_unknown": cancel_unknown,
        })
    );
}
