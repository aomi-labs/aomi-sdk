//! Example dynamic plugin for the Aomi platform.
//!
//! This plugin provides a single `greet` tool that returns a greeting message.
//! Build with `cargo build -p dyn-hello` to produce a `.so`/`.dylib` that can
//! be loaded by `DynLoader`.

use aomi_dyn_sdk::serde::Deserialize;
use aomi_dyn_sdk::*;
use schemars::JsonSchema;
use serde_json::json;

/// A simple plugin runtime that implements one tool: `greet`.
#[derive(Default)]
pub struct HelloRuntime;

#[derive(Debug, Deserialize, JsonSchema)]
struct GreetArgs {
    #[schemars(description = "The name of the person to greet")]
    name: Option<String>,
}

impl HelloRuntime {
    fn greet(&self, args: GreetArgs, ctx: DynCtx) -> Result<serde_json::Value, String> {
        let person_name = args.name.as_deref().unwrap_or("World");

        Ok(json!({
            "greeting": format!("Hello, {person_name}!"),
            "session_id": ctx.session_id,
            "plugin": "dyn-hello v0.1.0"
        }))
    }
}

dyn_runtime! {
    impl DynRuntime for HelloRuntime {
        manifest {
            name: "hello",
            version: "0.1.0",
            preamble: "You are a friendly greeter. When users ask for a greeting, use the greet tool.",
            model_preference: DynModelPreference::default(),
        }
        tools [
            {
                name: "greet",
                description: "Generate a personalized greeting message for the given name.",
                schema: derive,
                args: GreetArgs,
                ctx: DynCtx,
                handler: Self::greet,
                is_async: false,
            }
        ]
    }
}

// Generate the C ABI entry points
declare_dyn!(HelloRuntime);
