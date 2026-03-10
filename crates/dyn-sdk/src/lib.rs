//! # aomi-dyn-sdk
//!
//! Lightweight SDK for building dynamic Aomi plugins.
//!
//! A plugin is a shared library (`.so` on Linux, `.dylib` on macOS) that exports
//! a set of C ABI functions. The host backend loads these at runtime using `libloading`,
//! reads the plugin's [`DynManifest`], and wires the plugin's tools into the LLM agent.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use aomi_dyn_sdk::*;
//! use serde_json::json;
//!
//! #[derive(Default)]
//! struct MyPlugin;
//!
//! impl DynRuntime for MyPlugin {
//!     fn manifest(&self) -> DynManifest {
//!         DynManifest {
//!             abi_version: DYN_ABI_VERSION,
//!             name: "my_plugin".into(),
//!             version: "0.1.0".into(),
//!             preamble: "You are a helpful assistant for MyPlugin.".into(),
//!             model_preference: DynModelPreference::default(),
//!             tools: vec![
//!                 DynToolDescriptor {
//!                     name: "hello".into(),
//!                     namespace: "my_plugin".into(),
//!                     description: "Say hello".into(),
//!                     parameters_schema: json!({
//!                         "type": "object",
//!                         "properties": {
//!                             "name": { "type": "string" }
//!                         },
//!                         "required": ["name"]
//!                     }),
//!                     is_async: false,
//!                 },
//!             ],
//!         }
//!     }
//!
//!     fn execute_tool(&self, name: &str, args_json: &str, _ctx_json: &str) -> DynResult {
//!         match name {
//!             "hello" => {
//!                 let args: serde_json::Value = serde_json::from_str(args_json)
//!                     .unwrap_or_default();
//!                 let who = args["name"].as_str().unwrap_or("World");
//!                 DynResult::ok(json!({ "message": format!("Hello, {who}!") }))
//!             }
//!             _ => DynResult::err(format!("unknown tool: {name}")),
//!         }
//!     }
//! }
//!
//! declare_dyn!(MyPlugin);
//! ```
//!
//! ## Building
//!
//! In your `Cargo.toml`:
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! aomi-dyn-sdk = { version = "0.1" }
//! serde_json = "1.0"
//! ```
//!
//! Then `cargo build --release` produces `target/release/libmy_plugin.so` (or `.dylib`).
//! Drop this file into the backend's plugin directory and it's live.

mod ffi;
mod types;

pub use types::*;

// Re-export serde_json for convenience in plugin code
pub use serde_json;

/// Internal helpers for the `declare_dyn!` macro. Do not use directly.
#[doc(hidden)]
pub mod __private {
    pub use crate::ffi::{
        dyn_error_ptr, free_c_string, parse_c_str, serialize_dyn_result, string_to_c_ptr,
    };
}
