mod common;

use std::fs;
use std::process::Command;

fn cargo_check_temp_crate(crate_name: &str, source: &str) -> std::process::Output {
    let temp_dir = tempfile::tempdir().expect("failed to create temp crate");
    let src_dir = temp_dir.path().join("src");
    fs::create_dir_all(&src_dir).expect("failed to create src dir");

    let cargo_toml = format!(
        r#"[package]
name = "{crate_name}"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
aomi-sdk = {{ path = "{}" }}
schemars = "1.0.4"
serde = {{ version = "1", features = ["derive"] }}
serde_json = "1"
"#,
        common::dyn_sdk_crate_dir().display()
    );

    fs::write(temp_dir.path().join("Cargo.toml"), cargo_toml).expect("failed to write Cargo.toml");
    fs::write(src_dir.join("lib.rs"), source).expect("failed to write lib.rs");

    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    Command::new(cargo)
        .current_dir(temp_dir.path())
        .env(
            "CARGO_TARGET_DIR",
            common::workspace_root().join("target/dyn-sdk-compile-shapes"),
        )
        .arg("check")
        .arg("--quiet")
        .output()
        .expect("failed to run cargo check")
}

#[test]
fn valid_sync_dyn_app_shape_compiles() {
    let output = cargo_check_temp_crate(
        "dyn-sdk-shape-pass-sync",
        r#"
use aomi_sdk::{DynAomiTool, DynToolCallCtx, dyn_aomi_app, schemars::JsonSchema, serde_json::{json, Value}};
use serde::Deserialize;

#[derive(Clone, Default)]
struct App;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct Args {
    name: String,
}

struct Tool;

impl DynAomiTool for Tool {
    type App = App;
    type Args = Args;

    const NAME: &'static str = "tool";
    const DESCRIPTION: &'static str = "shape test tool";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        Ok(json!({ "hello": args.name }))
    }
}

dyn_aomi_app!(app = App, name = "shape_sync", version = "0.1.0", preamble = "shape", tools = [Tool]);
"#,
    );

    assert!(
        output.status.success(),
        "expected valid sync shape to compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn valid_async_dyn_app_shape_compiles() {
    let output = cargo_check_temp_crate(
        "dyn-sdk-shape-pass-async",
        r#"
use aomi_sdk::{DynAomiTool, DynAsyncSink, DynToolCallCtx, dyn_aomi_app, schemars::JsonSchema, serde_json::json};
use serde::Deserialize;

#[derive(Clone, Default)]
struct App;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct Args {
    n: u64,
}

struct Tool;

impl DynAomiTool for Tool {
    type App = App;
    type Args = Args;

    const NAME: &'static str = "tool";
    const DESCRIPTION: &'static str = "shape test async tool";
    const IS_ASYNC: bool = true;

    fn run_async(
        _app: &Self::App,
        args: Self::Args,
        _ctx: DynToolCallCtx,
        sink: DynAsyncSink,
    ) -> Result<(), String> {
        sink.complete(json!({ "n": args.n }))?;
        Ok(())
    }
}

dyn_aomi_app!(app = App, name = "shape_async", version = "0.1.0", preamble = "shape", tools = [Tool]);
"#,
    );

    assert!(
        output.status.success(),
        "expected valid async shape to compile:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn dyn_app_missing_default_fails_to_compile() {
    let output = cargo_check_temp_crate(
        "dyn-sdk-shape-fail-default",
        r#"
use aomi_sdk::{DynAomiTool, DynToolCallCtx, dyn_aomi_app, schemars::JsonSchema, serde_json::{json, Value}};
use serde::Deserialize;

#[derive(Clone)]
struct App;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct Args {
    name: String,
}

struct Tool;

impl DynAomiTool for Tool {
    type App = App;
    type Args = Args;

    const NAME: &'static str = "tool";
    const DESCRIPTION: &'static str = "shape test tool";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        Ok(json!({ "hello": args.name }))
    }
}

dyn_aomi_app!(app = App, name = "shape_fail", version = "0.1.0", preamble = "shape", tools = [Tool]);
"#,
    );

    assert!(
        !output.status.success(),
        "expected missing Default bound to fail compilation"
    );
}

#[test]
fn dyn_args_missing_jsonschema_fail_to_compile() {
    let output = cargo_check_temp_crate(
        "dyn-sdk-shape-fail-schema",
        r#"
use aomi_sdk::{DynAomiTool, DynToolCallCtx, dyn_aomi_app, serde_json::{json, Value}};
use serde::Deserialize;

#[derive(Clone, Default)]
struct App;

#[derive(Debug, Clone, Deserialize)]
struct Args {
    name: String,
}

struct Tool;

impl DynAomiTool for Tool {
    type App = App;
    type Args = Args;

    const NAME: &'static str = "tool";
    const DESCRIPTION: &'static str = "shape test tool";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        Ok(json!({ "hello": args.name }))
    }
}

dyn_aomi_app!(app = App, name = "shape_fail_schema", version = "0.1.0", preamble = "shape", tools = [Tool]);
"#,
    );

    assert!(
        !output.status.success(),
        "expected missing JsonSchema bound to fail compilation"
    );
}
