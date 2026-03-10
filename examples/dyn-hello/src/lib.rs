//! Example dynamic plugin for the Aomi platform.
//!
//! This plugin provides a single `greet` tool that returns a greeting message.
//! Build with `cargo build -p dyn-hello` to produce a `.so`/`.dylib` that can
//! be loaded by `DynLoader`.

use aomi_dyn_sdk::*;
use serde_json::json;

/// A simple plugin runtime that implements one tool: `greet`.
#[derive(Default)]
pub struct HelloRuntime;

impl DynRuntime for HelloRuntime {
    fn manifest(&self) -> DynManifest {
        DynManifest {
            abi_version: DYN_ABI_VERSION,
            name: "hello".into(),
            version: "0.1.0".into(),
            preamble:
                "You are a friendly greeter. When users ask for a greeting, use the greet tool."
                    .into(),
            model_preference: DynModelPreference::default(),
            tools: vec![DynToolDescriptor {
                name: "greet".into(),
                namespace: "hello".into(),
                description: "Generate a personalized greeting message for the given name.".into(),
                parameters_schema: json!({
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "The name of the person to greet"
                        }
                    },
                    "required": ["name"]
                }),
                is_async: false,
            }],
        }
    }

    fn execute_tool(&self, name: &str, args_json: &str, ctx_json: &str) -> DynResult {
        match name {
            "greet" => {
                let args: serde_json::Value = match serde_json::from_str(args_json) {
                    Ok(v) => v,
                    Err(e) => return DynResult::err(format!("invalid args: {e}")),
                };
                let ctx: DynCtx = match serde_json::from_str(ctx_json) {
                    Ok(v) => v,
                    Err(e) => return DynResult::err(format!("invalid ctx: {e}")),
                };

                let person_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("World");

                DynResult::ok(json!({
                    "greeting": format!("Hello, {person_name}!"),
                    "session_id": ctx.session_id,
                    "plugin": "dyn-hello v0.1.0"
                }))
            }
            _ => DynResult::err(format!("unknown tool: {name}")),
        }
    }
}

// Generate the C ABI entry points
declare_dyn!(HelloRuntime);
