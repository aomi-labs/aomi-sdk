mod common;

use aomi_sdk::{
    AOMI_ABI_VERSION, AOMI_CREATE, AOMI_DESTROY, AOMI_DYN_EXEC_CANCEL,
    AOMI_DYN_EXEC_POLL, AOMI_FREE_STRING, AOMI_MANIFEST, AsyncExecPool, DynAbiVersionFn,
    DynCreateFn, DynDestroyFn, DynFreeStringFn, DynManifestFn, DynToolCancelFn, DynToolPollFn,
    DynToolStart, DynToolStartFn, SYM_AOMI_ABI_VERSION, SYM_AOMI_ASYNC_TOOL_START,
};
use libloading::Library;

#[test]
fn raw_ffi_symbols_match_the_documented_abi_surface() {
    common::ensure_dyn_hello_built();
    let path = common::dyn_hello_path();
    let library = unsafe { Library::new(&path) }.expect("failed to open dyn-hello library");

    unsafe {
        let abi_version = *library
            .get::<DynAbiVersionFn>(SYM_AOMI_ABI_VERSION)
            .expect("missing aomi_abi_version");
        let create = *library
            .get::<DynCreateFn>(AOMI_CREATE)
            .expect("missing aomi_create");
        let manifest = *library
            .get::<DynManifestFn>(AOMI_MANIFEST)
            .expect("missing aomi_manifest");
        let start = *library
            .get::<DynToolStartFn>(SYM_AOMI_ASYNC_TOOL_START)
            .expect("missing aomi_async_tool_start");
        let poll = *library
            .get::<DynToolPollFn>(AOMI_DYN_EXEC_POLL)
            .expect("missing aomi_dyn_exec_poll");
        let cancel = *library
            .get::<DynToolCancelFn>(AOMI_DYN_EXEC_CANCEL)
            .expect("missing aomi_dyn_exec_cancel");
        let destroy = *library
            .get::<DynDestroyFn>(AOMI_DESTROY)
            .expect("missing aomi_destroy");
        let free_string = *library
            .get::<DynFreeStringFn>(AOMI_FREE_STRING)
            .expect("missing aomi_free_string");

        let instance = create();
        assert!(
            !instance.is_null(),
            "create should return a non-null instance"
        );
        assert_eq!(abi_version(), AOMI_ABI_VERSION);

        let manifest_raw = manifest(instance);
        assert!(!manifest_raw.is_null(), "manifest should return JSON");
        free_string(manifest_raw);

        // Function pointers are loaded only to prove the symbols exist.
        let _ = start;
        let _ = poll;
        let _ = cancel;

        destroy(instance);
    }
}

#[test]
fn envelope_status_tags_match_current_wire_format() {
    let start = serde_json::to_value(DynToolStart::AsyncQueued { execution_id: 7 }).unwrap();
    assert_eq!(
        start.get("status").and_then(|value| value.as_str()),
        Some("async_queued")
    );

    let poll = serde_json::to_value(AsyncExecPool::Pending).unwrap();
    assert_eq!(
        poll.get("status").and_then(|value| value.as_str()),
        Some("pending")
    );
}
