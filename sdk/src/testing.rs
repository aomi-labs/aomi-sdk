//! Test utilities for plugin authors.
//!
//! Provides helpers to unit-test [`DynAomiTool`](crate::DynAomiTool)
//! implementations without loading the full FFI plugin.
//!
//! # Example
//!
//! ```ignore
//! use aomi_sdk::testing::{TestCtxBuilder, run_tool};
//! use serde_json::json;
//!
//! let ctx = TestCtxBuilder::new("my_tool").build();
//! let result = run_tool::<MyTool>(&MyApp, json!({"query": "eth"}), ctx);
//! assert!(result.is_ok());
//! ```

use serde_json::{Map, Value};

use crate::{AsyncExecQueue, DynAomiTool, DynAsyncSink, DynToolCallCtx};
use std::sync::Arc;

/// Builder for constructing [`DynToolCallCtx`] in tests.
pub struct TestCtxBuilder {
    session_id: String,
    tool_name: String,
    call_id: String,
    state_attributes: Map<String, Value>,
}

impl TestCtxBuilder {
    /// Create a new builder with the given tool name.
    /// Uses generated defaults for session_id and call_id.
    pub fn new(tool_name: &str) -> Self {
        Self {
            session_id: "test-session".to_string(),
            tool_name: tool_name.to_string(),
            call_id: "test-call-1".to_string(),
            state_attributes: Map::new(),
        }
    }

    /// Override the session id.
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.session_id = id.into();
        self
    }

    /// Override the call id.
    pub fn call_id(mut self, id: impl Into<String>) -> Self {
        self.call_id = id.into();
        self
    }

    /// Insert a state attribute.
    pub fn attribute(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.state_attributes.insert(key.into(), value.into());
        self
    }

    /// Build the [`DynToolCallCtx`].
    pub fn build(self) -> DynToolCallCtx {
        DynToolCallCtx {
            session_id: self.session_id,
            tool_name: self.tool_name,
            call_id: self.call_id,
            state_attributes: self.state_attributes,
        }
    }
}

/// Run a synchronous tool with typed args and return the result.
///
/// Convenience wrapper that serializes `args` into the tool's `Args` type
/// and invokes `T::run`.
pub fn run_tool<T: DynAomiTool>(
    app: &T::App,
    args: Value,
    ctx: DynToolCallCtx,
) -> Result<Value, String> {
    let typed_args: T::Args =
        serde_json::from_value(args).map_err(|e| format!("invalid test args: {e}"))?;
    T::run(app, typed_args, ctx)
}

/// Run an async tool and collect all emitted values.
///
/// Returns `Ok(updates)` where `updates` is a `Vec<Value>` of all emitted
/// values (including the terminal one), or `Err` if the tool failed.
pub fn run_async_tool<T: DynAomiTool>(
    app: &T::App,
    args: Value,
    ctx: DynToolCallCtx,
) -> Result<Vec<Value>, String> {
    let typed_args: T::Args =
        serde_json::from_value(args).map_err(|e| format!("invalid test args: {e}"))?;
    let queue = Arc::new(AsyncExecQueue::default());
    let sink = DynAsyncSink::__from_queue(queue.clone());

    T::run_async(app, typed_args, ctx, sink)?;

    let mut updates = Vec::new();
    loop {
        match queue.poll() {
            crate::AsyncExecPool::Pending => break,
            crate::AsyncExecPool::Update { value, has_more } => {
                updates.push(value);
                if !has_more {
                    break;
                }
            }
            crate::AsyncExecPool::Error { message } => return Err(message),
            crate::AsyncExecPool::Canceled => return Err("canceled".to_string()),
            crate::AsyncExecPool::NotFound => break,
        }
    }
    Ok(updates)
}
