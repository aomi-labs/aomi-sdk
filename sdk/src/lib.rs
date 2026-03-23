//! # aomi-sdk
//!
//! Lightweight SDK for building dynamic Aomi plugins that are loaded at runtime
//! as shared libraries (`cdylib`) via a stable C ABI.
//!
//! # Quick start
//!
//! A minimal plugin needs four pieces:
//!
//! **1. An app struct** — marker that ties everything together:
//!
//! ```rust,ignore
//! #[derive(Clone, Default)]
//! struct MyApp;
//! ```
//!
//! **2. A typed args struct** — deserialized from incoming JSON:
//!
//! ```rust,ignore
//! use schemars::JsonSchema;
//! use serde::Deserialize;
//!
//! #[derive(Debug, Deserialize, JsonSchema)]
//! struct GreetArgs {
//!     name: String,
//! }
//! ```
//!
//! **3. A tool implementation** — the actual logic:
//!
//! ```rust,ignore
//! use aomi_sdk::{DynAomiTool, DynToolCallCtx};
//! use serde_json::Value;
//!
//! struct Greet;
//!
//! impl DynAomiTool for Greet {
//!     type App = MyApp;
//!     type Args = GreetArgs;
//!     const NAME: &'static str = "greet";
//!     const DESCRIPTION: &'static str = "Greet someone by name.";
//!
//!     fn run(_app: &MyApp, args: GreetArgs, _ctx: DynToolCallCtx) -> Result<Value, String> {
//!         Ok(serde_json::json!({ "message": format!("Hello, {}!", args.name) }))
//!     }
//! }
//! ```
//!
//! **4. The [`dyn_aomi_app!`] macro** — generates manifest, router, and FFI exports:
//!
//! ```rust,ignore
//! aomi_sdk::dyn_aomi_app!(
//!     app = MyApp,
//!     name = "greeter",
//!     version = "0.1.0",
//!     preamble = "You are a friendly greeter.",
//!     tools = [Greet],
//! );
//! ```
//!
//! **`Cargo.toml`** — the crate must be a `cdylib`:
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! aomi-sdk = "0.1"
//! schemars = "1"
//! serde = { version = "1", features = ["derive"] }
//! serde_json = "1"
//! ```
//!
//! # Async tools
//!
//! For long-running or streaming tools, set `IS_ASYNC = true` and implement
//! [`DynAomiTool::run_async`] instead of `run`. The host polls for updates via
//! the [`DynAsyncSink`]:
//!
//! ```rust,ignore
//! impl DynAomiTool for StreamingTool {
//!     type App = MyApp;
//!     type Args = StreamArgs;
//!     const NAME: &'static str = "stream";
//!     const DESCRIPTION: &'static str = "Stream results over time.";
//!     const IS_ASYNC: bool = true;
//!
//!     fn run_async(
//!         _app: &MyApp,
//!         args: StreamArgs,
//!         _ctx: DynToolCallCtx,
//!         sink: DynAsyncSink,
//!     ) -> Result<(), String> {
//!         // Emit intermediate results
//!         sink.emit(serde_json::json!({ "step": 1 })).map_err(|e| e.to_string())?;
//!         // Signal completion
//!         sink.complete(serde_json::json!({ "done": true })).map_err(|e| e.to_string())?;
//!         Ok(())
//!     }
//! }
//! ```
//!
//! # Architecture
//!
//! ```text
//! ┌──────────────────────┐         C ABI (cdylib)         ┌───────────────────┐
//! │     Host / Backend   │◄──────────────────────────────►│   Plugin (.so)    │
//! │                      │  aomi_create()                  │                   │
//! │  DynFnHandle::load() │  aomi_manifest()                │  dyn_aomi_app!()  │
//! │  call_manifest()     │  aomi_async_tool_start()        │  DynAomiTool impls│
//! │  call_exec_tool()    │  aomi_dyn_exec_poll()           │                   │
//! │                      │  aomi_dyn_exec_cancel()         │                   │
//! │                      │  aomi_destroy()                 │                   │
//! └──────────────────────┘  aomi_free_string()            └───────────────────┘
//! ```
//!
//! All data crosses the boundary as JSON-serialized C strings. The `abi` module
//! defines the function signatures; [`declare_dyn!`] and [`dyn_aomi_app!`] generate
//! the implementations.
//!
//! # Audience guide
//!
//! | You are a…             | Start here                                         |
//! |------------------------|----------------------------------------------------|
//! | **Plugin author**      | [`DynAomiTool`], [`dyn_aomi_app!`], [`testing`]    |
//! | **Host integrator**    | [`DynFnHandle`], [`DynManifest`], `abi` types        |
//!
//! # Re-exports
//!
//! [`schemars`] and [`serde_json`] are re-exported so plugin crates don't need to
//! manage version alignment for these common dependencies.
//!
//! # Testing
//!
//! The [`testing`] module provides [`testing::TestCtxBuilder`] and
//! [`testing::run_tool`] / [`testing::run_async_tool`] helpers to unit-test tools
//! without loading the full FFI plugin.

mod abi;
mod ffi;
mod handle;
pub mod testing;
mod types;

pub use abi::*;
pub use handle::*;
pub use types::*;

// Re-export serde_json and schemars for convenience in plugin code.
pub use schemars;
pub use serde_json;

/// Internal helpers for macros. Do not use directly.
#[doc(hidden)]
pub mod __private {
    pub use crate::ffi::{
        free_c_string, log_async_tool_error, log_poll_error, log_tool_exec_error,
        log_tool_start_error, parse_c_str, serialize_to_c_ptr, string_to_c_ptr,
    };
    pub use crate::types::AsyncExecQueue;
}
