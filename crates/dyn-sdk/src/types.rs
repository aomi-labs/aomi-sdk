//! Core types for the Aomi dynamic plugin system.
//!
//! These types define the contract between a dynamically loaded plugin (`.so`/`.dylib`)
//! and the host backend. All types are serializable to JSON for crossing the FFI boundary.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// ABI version constant. The host checks this before loading a plugin.
/// Bump this when making breaking changes to the FFI contract.
pub const DYN_ABI_VERSION: u32 = 1;

// ============================================================================
// Tool Context (crosses FFI boundary as JSON)
// ============================================================================

/// Simplified tool execution context passed across the FFI boundary.
///
/// This is a projection of the host-side `ToolCallCtx` — contains only
/// what the plugin needs to execute a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynCtx {
    /// Session identifier (unique per chat session)
    pub session_id: String,
    /// Name of the tool being called
    pub tool_name: String,
    /// Unique identifier for this specific tool invocation
    pub call_id: String,
    /// Chain ID from the user's connected wallet (None if not connected)
    pub user_chain_id: Option<u64>,
    /// Address from the user's connected wallet (None if not connected)
    pub user_address: Option<String>,
}

// ============================================================================
// Tool Descriptor (declarative metadata)
// ============================================================================

/// Declarative description of a single tool provided by the plugin.
///
/// The host uses this to register the tool with the LLM agent.
/// The `parameters_schema` must be a valid JSON Schema object describing
/// the tool's arguments (what the LLM will produce).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynToolDescriptor {
    /// Unique tool name (e.g. "delta_create_quote")
    pub name: String,
    /// Namespace for access control and grouping (e.g. "delta")
    pub namespace: String,
    /// Human-readable description shown to the LLM
    pub description: String,
    /// JSON Schema for the tool's parameters
    pub parameters_schema: Value,
    /// Whether the tool supports async/streaming execution
    pub is_async: bool,
}

// ============================================================================
// Model Preference (hint to the host)
// ============================================================================

/// Model preference hint from the plugin.
///
/// The host uses these strings to select which LLM model to use.
/// If `None`, the host uses its default model. Values are model slugs
/// like `"claude-opus-4"`, `"claude-sonnet-4"`, `"gpt-5"`, etc.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DynModelPreference {
    /// Model for the main LLM agent (e.g. "claude-opus-4")
    pub rig: Option<String>,
    /// Model for BAML/structured extraction tasks
    pub baml: Option<String>,
}

// ============================================================================
// Plugin Manifest
// ============================================================================

/// Complete manifest describing a dynamic plugin.
///
/// The host reads this once after loading the plugin to understand
/// what tools it provides and how to configure the LLM agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynManifest {
    /// ABI version — must match [`DYN_ABI_VERSION`]
    pub abi_version: u32,
    /// Plugin name (used as the namespace key, e.g. "delta", "hello")
    pub name: String,
    /// Plugin version (semver, e.g. "0.1.0")
    pub version: String,
    /// System prompt / preamble for the LLM agent
    pub preamble: String,
    /// Model preference hints
    pub model_preference: DynModelPreference,
    /// Tools provided by this plugin
    pub tools: Vec<DynToolDescriptor>,
}

// ============================================================================
// Tool Execution Result
// ============================================================================

/// Result of a tool execution across the FFI boundary.
///
/// Serialized to JSON and returned from `aomi_dyn_exec_tool`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DynResult {
    /// Successful execution with a JSON value result
    Ok(Value),
    /// Failed execution with an error message
    Err(String),
}

impl DynResult {
    /// Create a successful result from a serializable value.
    pub fn ok(value: impl Serialize) -> Self {
        DynResult::Ok(serde_json::to_value(value).unwrap_or(Value::Null))
    }

    /// Create an error result.
    pub fn err(msg: impl Into<String>) -> Self {
        DynResult::Err(msg.into())
    }

    /// Check if the result is successful.
    pub fn is_ok(&self) -> bool {
        matches!(self, DynResult::Ok(_))
    }

    /// Check if the result is an error.
    pub fn is_err(&self) -> bool {
        matches!(self, DynResult::Err(_))
    }
}

// ============================================================================
// Plugin Runtime Trait
// ============================================================================

/// Trait that plugin authors implement to define their plugin's behavior.
///
/// The plugin crate should:
/// 1. Define a struct that implements `DynRuntime` (and `Default`)
/// 2. Call `declare_dyn!(MyRuntime)` to generate the C ABI entry points
///
/// # Example
///
/// ```rust,ignore
/// use aomi_dyn_sdk::*;
///
/// #[derive(Default)]
/// struct MyPlugin;
///
/// impl DynRuntime for MyPlugin {
///     fn manifest(&self) -> DynManifest {
///         DynManifest {
///             abi_version: DYN_ABI_VERSION,
///             name: "my_plugin".into(),
///             version: "0.1.0".into(),
///             preamble: "You are a helpful assistant...".into(),
///             model_preference: DynModelPreference::default(),
///             tools: vec![/* ... */],
///         }
///     }
///
///     fn execute_tool(&self, name: &str, args_json: &str, ctx_json: &str) -> DynResult {
///         match name {
///             "my_tool" => { /* ... */ DynResult::ok(json!({"result": "done"})) }
///             _ => DynResult::err(format!("unknown tool: {name}")),
///         }
///     }
/// }
///
/// declare_dyn!(MyPlugin);
/// ```
pub trait DynRuntime: Send + Sync {
    /// Return the plugin manifest describing its tools and configuration.
    fn manifest(&self) -> DynManifest;

    /// Execute a tool by name.
    ///
    /// - `name`: The tool name (matches `DynToolDescriptor::name`)
    /// - `args_json`: JSON string of the tool's arguments (from the LLM)
    /// - `ctx_json`: JSON string of [`DynCtx`] (session info, wallet state)
    ///
    /// This function may block (e.g. for HTTP calls). The host calls it
    /// from a blocking-capable context.
    fn execute_tool(&self, name: &str, args_json: &str, ctx_json: &str) -> DynResult;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_dyn_ctx_roundtrip() {
        let ctx = DynCtx {
            session_id: "sess_123".into(),
            tool_name: "greet".into(),
            call_id: "call_456".into(),
            user_chain_id: Some(1),
            user_address: Some("0xabc".into()),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: DynCtx = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id, "sess_123");
        assert_eq!(parsed.user_chain_id, Some(1));
    }

    #[test]
    fn test_dyn_tool_descriptor_roundtrip() {
        let desc = DynToolDescriptor {
            name: "get_pools".into(),
            namespace: "defi".into(),
            description: "List liquidity pools".into(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "chain_id": { "type": "integer" }
                },
                "required": ["chain_id"]
            }),
            is_async: false,
        };
        let json = serde_json::to_string(&desc).unwrap();
        let parsed: DynToolDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "get_pools");
        assert_eq!(parsed.namespace, "defi");
        assert!(!parsed.is_async);
    }

    #[test]
    fn test_dyn_manifest_roundtrip() {
        let manifest = DynManifest {
            abi_version: DYN_ABI_VERSION,
            name: "test_plugin".into(),
            version: "1.0.0".into(),
            preamble: "You are a test assistant.".into(),
            model_preference: DynModelPreference {
                rig: Some("claude-opus-4".into()),
                baml: None,
            },
            tools: vec![DynToolDescriptor {
                name: "echo".into(),
                namespace: "test".into(),
                description: "Echo input".into(),
                parameters_schema: json!({"type": "object", "properties": {}}),
                is_async: false,
            }],
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: DynManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.abi_version, DYN_ABI_VERSION);
        assert_eq!(parsed.name, "test_plugin");
        assert_eq!(parsed.tools.len(), 1);
        assert_eq!(parsed.model_preference.rig, Some("claude-opus-4".into()));
    }

    #[test]
    fn test_dyn_result_ok() {
        let result = DynResult::ok(json!({"greeting": "hello"}));
        assert!(result.is_ok());
        let json = serde_json::to_string(&result).unwrap();
        let parsed: DynResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_dyn_result_err() {
        let result = DynResult::err("something went wrong");
        assert!(result.is_err());
        let json = serde_json::to_string(&result).unwrap();
        let parsed: DynResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_err());
        if let DynResult::Err(msg) = parsed {
            assert_eq!(msg, "something went wrong");
        }
    }

    #[test]
    fn test_dyn_ctx_optional_fields() {
        let ctx = DynCtx {
            session_id: "s1".into(),
            tool_name: "t1".into(),
            call_id: "c1".into(),
            user_chain_id: None,
            user_address: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let parsed: DynCtx = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.user_chain_id, None);
        assert_eq!(parsed.user_address, None);
    }

    #[test]
    fn test_default_model_preference() {
        let pref = DynModelPreference::default();
        assert!(pref.rig.is_none());
        assert!(pref.baml.is_none());
    }
}
