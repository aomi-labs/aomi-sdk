//! `aomi-build init <NAME>` — scaffold a bare app skeleton under `apps/<NAME>/`.
//!
//! Counterpart to `aomi-build new-app`, which is OpenAPI-driven. `init` is
//! the greenfield path: stub `lib.rs` + `client.rs` + `tool.rs`, no spec.
//!
//! Ported from the old `cargo xtask new-app` subcommand. The workspace
//! `exclude` list mutation now goes through `toml_edit` for robust round-trip.

use std::fs;

use clap::Args;
use eyre::{Result, bail};
use toml_edit::{Array, DocumentMut, value};

use crate::spec_load::pascal_case;
use crate::specs::workspace_root;

#[derive(Args, Debug)]
pub struct InitArgs {
    /// New app name. Lowercase alphanumeric with hyphens (e.g. `my-app`).
    pub name: String,
}

pub fn run(args: InitArgs) -> Result<()> {
    let name = &args.name;

    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || name.is_empty()
    {
        bail!("app name must be lowercase alphanumeric with hyphens (e.g. 'my-app')");
    }

    let repo_root = workspace_root()?;
    let app_dir = repo_root.join("apps").join(name);

    if app_dir.exists() {
        bail!("apps/{name} already exists");
    }

    let src_dir = app_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Derive a Rust-friendly struct name from the app name.
    let struct_name = format!("{}App", pascal_case(name));

    // Cargo.toml
    fs::write(
        app_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
aomi-sdk = {{ path = "../../sdk" }}
reqwest = {{ version = "0.12", default-features = false, features = ["json", "rustls-tls", "blocking"] }}
schemars = "1.0.4"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#
        ),
    )?;

    // src/lib.rs — built via push_str to avoid nested raw-string delimiter issues.
    let mut lib_rs = String::new();
    lib_rs.push_str("mod client;\nmod tool;\n\n");
    lib_rs.push_str("use aomi_sdk::dyn_aomi_app;\n\n");
    lib_rs.push_str("const PREAMBLE: &str = r#\"\n");
    lib_rs.push_str(&format!("## Role\nYou are the {name} app.\n\n"));
    lib_rs.push_str("## Purpose\nTODO: describe what this app does.\n");
    lib_rs.push_str("\"#;\n\n");
    lib_rs.push_str(&format!(
        "dyn_aomi_app!(\n\
         \x20   app = client::{struct_name},\n\
         \x20   name = \"{name}\",\n\
         \x20   version = \"0.1.0\",\n\
         \x20   preamble = PREAMBLE,\n\
         \x20   tools = [\n\
         \x20       tool::ExampleTool,\n\
         \x20   ],\n\
         \x20   namespaces = [\"common\"]\n\
         );\n"
    ));
    fs::write(src_dir.join("lib.rs"), lib_rs)?;

    // src/client.rs
    fs::write(
        src_dir.join("client.rs"),
        format!(
            r#"#[derive(Clone, Default)]
pub(crate) struct {struct_name};
"#
        ),
    )?;

    // src/tool.rs
    let snake_name = name.replace('-', "_");
    fs::write(
        src_dir.join("tool.rs"),
        format!(
            r#"use aomi_sdk::{{DynAomiTool, DynToolCallCtx}};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::client::{struct_name};

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ExampleToolArgs {{
    /// A sample input parameter.
    pub query: String,
}}

pub(crate) struct ExampleTool;

impl DynAomiTool for ExampleTool {{
    type App = {struct_name};
    type Args = ExampleToolArgs;

    const NAME: &'static str = "{snake_name}_example";
    const DESCRIPTION: &'static str = "TODO: describe this tool.";

    fn run(
        _app: &{struct_name},
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {{
        Ok(serde_json::json!({{
            "echo": args.query,
        }}))
    }}
}}
"#
        ),
    )?;

    // Register in workspace exclude list via toml_edit (round-trip safe).
    register_workspace_exclude(&repo_root, name)?;

    println!("created apps/{name}/");
    println!("  src/lib.rs    — app manifest + preamble");
    println!("  src/client.rs — app struct + HTTP client");
    println!("  src/tool.rs   — tool implementations");
    println!();
    println!("next steps:");
    println!("  1. edit the preamble in src/lib.rs");
    println!("  2. add your HTTP client in src/client.rs");
    println!("  3. implement your tools in src/tool.rs");
    println!("  4. aomi-build compile --app {name}");
    Ok(())
}

fn register_workspace_exclude(repo_root: &std::path::Path, name: &str) -> Result<()> {
    let workspace_toml_path = repo_root.join("Cargo.toml");
    let raw = fs::read_to_string(&workspace_toml_path)?;
    let mut doc: DocumentMut = raw.parse()?;

    let workspace = doc
        .get_mut("workspace")
        .and_then(|w| w.as_table_mut())
        .ok_or_else(|| eyre::eyre!("workspace Cargo.toml has no [workspace] table"))?;

    let entry = format!("apps/{name}");
    let exclude_item = workspace.entry("exclude").or_insert_with(|| {
        let mut arr = Array::new();
        arr.set_trailing_comma(true);
        value(arr)
    });
    let exclude = exclude_item
        .as_array_mut()
        .ok_or_else(|| eyre::eyre!("workspace.exclude is not an array"))?;

    let already_listed = exclude
        .iter()
        .any(|v| v.as_str().map(|s| s == entry).unwrap_or(false));
    if !already_listed {
        // Match the existing one-entry-per-line style: prefix the new value
        // with a newline + four-space indent so the rendered TOML stays
        // diff-friendly. Without this, toml_edit appends inline after the
        // previous trailing comma.
        let mut v = toml_edit::Value::from(entry);
        v.decor_mut().set_prefix("\n    ");
        exclude.push_formatted(v);
    }

    fs::write(&workspace_toml_path, doc.to_string())?;
    Ok(())
}
