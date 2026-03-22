#![allow(dead_code)]

use aomi_sdk::DynFnHandle;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, OnceLock};

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

pub fn dyn_sdk_crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn ensure_dyn_hello_built() {
    static BUILD_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

    let result = BUILD_RESULT.get_or_init(|| {
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        let status = Command::new(cargo)
            .current_dir(workspace_root())
            .arg("build")
            .arg("-p")
            .arg("dyn-hello")
            .status()
            .map_err(|e| format!("failed to spawn cargo build for dyn-hello: {e}"))?;

        if status.success() {
            Ok(())
        } else {
            Err("cargo build failed for dyn-hello".to_string())
        }
    });

    if let Err(err) = result {
        panic!("{err}");
    }
}

pub fn dyn_hello_path() -> PathBuf {
    ensure_dyn_hello_built();

    let debug_dir = if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
        PathBuf::from(target_dir).join("debug")
    } else {
        workspace_root().join("target/debug")
    };

    for candidate in [
        debug_dir.join("libdyn_hello.dylib"),
        debug_dir.join("libdyn_hello.so"),
        debug_dir.join("dyn_hello.dll"),
    ] {
        if candidate.exists() {
            return candidate;
        }
    }

    panic!("dyn-hello plugin not found in {}", debug_dir.display());
}

pub fn load_dyn_hello() -> Arc<DynFnHandle> {
    Arc::new(unsafe { DynFnHandle::load(&dyn_hello_path()) }.expect("failed to load dyn-hello"))
}
