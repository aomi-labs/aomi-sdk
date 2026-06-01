//! `PluginTool` — adapts one `DynToolMetadata` exposed by a loaded plugin
//! into a `rig::tool::Tool` that the agent can call.
//!
//! Each `PluginTool` holds an `Arc<DynFnHandle>` plus the metadata it
//! advertises. When rig dispatches a tool call:
//!
//! 1. We build a `DynToolCallCtx` for this invocation (fresh `call_id`,
//!    fixed `session_id`, this app's resolved secrets).
//! 2. JSON-serialize args + ctx.
//! 3. Cross the FFI on a blocking task (`call_exec_tool` polls async
//!    tools internally up to 300s — see `sdk/src/handle.rs`).
//! 4. Hand the resulting `Value` back to rig, which feeds it into the
//!    conversation as the tool_result.

use std::collections::HashMap;
use std::sync::Arc;

use rig::completion::ToolDefinition;
use rig::tool::{Tool, ToolDyn};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use aomi_sdk::{DynFnHandle, DynToolCallCtx, DynToolMetadata};

use crate::load::LoadedPlugin;

#[derive(Debug, Error)]
pub enum PluginToolError {
    #[error("failed to serialize tool args/ctx: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("plugin tool '{tool}' failed: {source}")]
    Ffi {
        tool: String,
        #[source]
        source: eyre::Report,
    },
    #[error("blocking task join failure: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// Wraps a single plugin tool so rig can dispatch it.
#[derive(Clone)]
pub struct PluginTool {
    handle: Arc<DynFnHandle>,
    meta: DynToolMetadata,
    session_id: String,
    /// Resolved per-app secrets, cloned into every `DynToolCallCtx` so the
    /// plugin's `resolve_secret_value(ctx, …)` can read them.
    secrets: HashMap<String, String>,
}

impl PluginTool {
    pub fn new(
        handle: Arc<DynFnHandle>,
        meta: DynToolMetadata,
        session_id: String,
        secrets: HashMap<String, String>,
    ) -> Self {
        Self {
            handle,
            meta,
            session_id,
            secrets,
        }
    }
}

impl Tool for PluginTool {
    // `NAME` is required as a const, but each PluginTool advertises its
    // real name through the `name()` override (rig's blanket
    // `ToolDyn for T: Tool` calls `T::name(&self)`, not the const, so
    // this placeholder never leaks to the agent).
    const NAME: &'static str = "__plugin__";

    type Error = PluginToolError;
    /// Accept any JSON object — the schema is enforced LLM-side via the
    /// `ToolDefinition.parameters` field.
    type Args = Value;
    type Output = Value;

    fn name(&self) -> String {
        self.meta.name.clone()
    }

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.meta.name.clone(),
            description: self.meta.description.clone(),
            parameters: self.meta.parameters_schema.clone(),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let ctx = DynToolCallCtx {
            session_id: self.session_id.clone(),
            tool_name: self.meta.name.clone(),
            call_id: Uuid::new_v4().to_string(),
            state_attributes: Default::default(),
            secrets: self.secrets.clone(),
        };

        let args_json = serde_json::to_string(&args)?;
        let ctx_json = serde_json::to_string(&ctx)?;

        // FFI is sync — move it off the async runtime. `call_exec_tool`
        // already handles the async-tool poll loop with a 300s cap.
        let handle = self.handle.clone();
        let name = self.meta.name.clone();
        let value = tokio::task::spawn_blocking(move || {
            handle.call_exec_tool(&name, &args_json, &ctx_json)
        })
        .await?
        .map_err(|source| PluginToolError::Ffi {
            tool: self.meta.name.clone(),
            source,
        })?;

        Ok(value)
    }
}

/// Build one `Box<dyn ToolDyn>` per tool declared in the manifest.
pub fn build_plugin_tools(
    loaded: &LoadedPlugin,
    session_id: String,
    secrets: HashMap<String, String>,
) -> Vec<Box<dyn ToolDyn>> {
    loaded
        .manifest
        .tools
        .iter()
        .cloned()
        .map(|meta| {
            Box::new(PluginTool::new(
                loaded.handle.clone(),
                meta,
                session_id.clone(),
                secrets.clone(),
            )) as Box<dyn ToolDyn>
        })
        .collect()
}
