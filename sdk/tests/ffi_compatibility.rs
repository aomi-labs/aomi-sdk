//! Historical SDKs compile separately; these binaries are never relinked to the
//! candidate SDK. This tests binary/wire compatibility, not source migration.
mod common;

use aomi_sdk::{AsyncExecPool, DynFnHandle, DynToolStart};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

fn legacy_plugin(version: &str) -> PathBuf {
    static BUILT: OnceLock<()> = OnceLock::new();
    BUILT.get_or_init(|| {
        let status = Command::new("python3")
            .arg(common::workspace_root().join("scripts/build_legacy_ffi_fixtures.py"))
            .status()
            .expect("build historical SDK fixtures");
        assert!(status.success(), "historical SDK fixtures must compile");
    });
    let name = format!(
        "{}ffi_legacy{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    common::workspace_root()
        .join("target/ffi-fixtures")
        .join(version)
        .join("target/debug")
        .join(name)
}

fn context() -> &'static str {
    r#"{"session_id":"legacy","tool_name":"test","call_id":"call","state_attributes":{},"secrets":{}}"#
}

#[test]
fn historical_binaries_keep_manifest_sync_async_error_and_cancel_behavior() {
    for version in ["5.1.1", "5.1.2"] {
        let handle = unsafe { DynFnHandle::load(&legacy_plugin(version)) }
            .unwrap_or_else(|error| panic!("SDK {version}: {error:#}"));
        let manifest = handle.call_manifest().unwrap();
        assert_eq!(manifest.sdk_version, version);
        assert_eq!(manifest.ffi_version, None);
        let greeting = handle
            .call_exec_tool("greet", r#"{"name":"Aomi"}"#, context())
            .unwrap();
        assert_eq!(greeting["greeting"], "Hello, Aomi!");
        assert!(handle.call_exec_tool("unknown_tool", "{}", context()).is_err());
        assert!(handle.call_exec_tool("greet", "not-json", context()).is_err());
        assert!(handle.call_exec_tool("panic_sync", "{}", context()).is_err());
        let result = handle
            .call_exec_tool("count_async", r#"{"upto":2}"#, context())
            .unwrap();
        assert_eq!(result["count"], 2);
        assert!(handle
            .call_exec_tool("fail_async", r#"{"message":"expected failure"}"#, context())
            .is_err());

        let started = handle
            .call_tool_start("wait_for_cancel_async", "{}", context())
            .unwrap();
        let DynToolStart::AsyncQueued { execution_id } = started else {
            panic!("legacy async tool must queue");
        };
        assert!(handle.call_tool_cancel(execution_id).unwrap().canceled);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if matches!(handle.call_tool_poll(execution_id).unwrap(), AsyncExecPool::Canceled) {
                break;
            }
            assert!(Instant::now() < deadline, "cancel must terminate execution");
            std::thread::sleep(Duration::from_millis(5));
        }
        // Drop releases the instance while the old library is still loaded.
        drop(handle);
    }
}

fn probe(ffi: u32, manifest: serde_json::Value) -> (tempfile::TempDir, PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let source = include_str!("fixtures/ffi-probe.rs")
        .replace("__SDK__", "99.0.0")
        .replace("__FFI__", &ffi.to_string())
        .replace("__MANIFEST__", &manifest.to_string());
    let path = directory.path().join("probe.rs");
    std::fs::write(&path, source).unwrap();
    let status = Command::new(std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into()))
        .args(["--edition=2024", "--crate-type=cdylib", "--crate-name=ffi_probe"])
        .arg(&path)
        .arg("--out-dir")
        .arg(directory.path())
        .status()
        .unwrap();
    assert!(status.success());
    let library = directory.path().join(format!(
        "{}ffi_probe{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    (directory, library)
}

fn probe_manifest() -> serde_json::Value {
    serde_json::json!({
        "sdk_version": "99.0.0", "ffi_version": 1,
        "name": "probe", "version": "0.0.0", "preamble": "probe", "tools": []
    })
}

#[test]
fn independent_sdk_release_loads_when_ffi_matches() {
    let (_directory, library) = probe(1, probe_manifest());
    let handle = unsafe { DynFnHandle::load(&library) }.unwrap();
    assert_eq!(handle.call_manifest().unwrap().sdk_version, "99.0.0");
}

#[test]
fn unsupported_exports_and_inconsistent_manifests_are_rejected() {
    let (_directory, library) = probe(2, probe_manifest());
    let error = unsafe { DynFnHandle::load(&library) }.err().unwrap();
    assert!(format!("{error:#}").contains("FFI version mismatch"));

    let mut manifest = probe_manifest();
    manifest["ffi_version"] = serde_json::json!(2);
    let (_directory, library) = probe(1, manifest);
    assert!(unsafe { DynFnHandle::load(&library) }.is_err());

    let mut manifest = probe_manifest();
    manifest.as_object_mut().unwrap().remove("ffi_version");
    let (_directory, library) = probe(1, manifest);
    let error = unsafe { DynFnHandle::load(&library) }.err().unwrap();
    assert!(error.to_string().contains("missing ffi_version"));

    let mut manifest = probe_manifest();
    manifest["required_capabilities"] = serde_json::json!(["future_permission_v2"]);
    let (_directory, library) = probe(1, manifest);
    let error = unsafe { DynFnHandle::load(&library) }.err().unwrap();
    assert!(error.to_string().contains("unsupported required FFI capability"));
}
