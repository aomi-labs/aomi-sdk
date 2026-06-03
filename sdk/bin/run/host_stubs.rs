//! Stubs for host-side namespace tools.
//!
//! When a plugin declares `namespaces = ["evm-core"]` (or similar), the
//! real backend wires in a concrete `ToolNamespace` that implements
//! `commit_tx`, `stage_tx`, `simulate_batch`, etc. The dev runtime has
//! none of that machinery — but we can't simply omit the tools, because
//! the LLM would then have no way to discover that these capabilities
//! exist.
//!
//! Instead we register a `StubTool` for each well-known host tool. Its
//! schema accepts an arbitrary JSON object; its implementation returns
//! an `Ok(...)` sentinel describing why the call did nothing. Returning
//! `Ok` (rather than `Err`) keeps the LLM from looping on retries — it
//! reads the note and moves on.
//!
//! The set of stubbed tools mirrors `aomi_sdk::route::host` — the marker
//! types developers use when building route plans. Namespaces that we
//! don't recognise just get a startup warning and no stubs.

use rig::completion::ToolDefinition;
use rig::tool::{Tool, ToolDyn};
use std::collections::BTreeSet;

use aomi_sdk::DynManifest;
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StubError {}

#[derive(Clone)]
pub struct StubTool {
    name: &'static str,
    namespace: &'static str,
    description: String,
}

impl Tool for StubTool {
    const NAME: &'static str = "__stub__";

    type Error = StubError;
    type Args = Value;
    type Output = Value;

    fn name(&self) -> String {
        self.name.to_string()
    }

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: self.name.to_string(),
            description: self.description.clone(),
            // Accept anything — the real schema lives on the backend.
            parameters: json!({
                "type": "object",
                "additionalProperties": true,
                "description": "Stubbed host tool — accepts any object.",
            }),
        }
    }

    async fn call(&self, _args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(json!({
            "error": format!(
                "host tool '{}' (namespace '{}') is unavailable in aomi-run runtime; \
                 test this flow against the deployed backend",
                self.name, self.namespace
            ),
            "tool": self.name,
            "namespace": self.namespace,
            "stubbed": true,
        }))
    }
}

/// Static registry: `(namespace, tool_name, one-line description)`.
///
/// Keep in sync with the `host_target!` macro invocations in
/// `sdk/src/route.rs` — those are the names that `RouteBuilder::after::<T>`
/// emits, so they're the canonical list of host tools plugins know about.
const STUB_REGISTRY: &[(&str, &str, &str)] = &[
    // evm-core: wallet + EVM read/write primitives.
    ("evm-core", "brave_search", "(stub) Web search via Brave."),
    (
        "evm-core",
        "commit_tx",
        "(stub) Submit a single EVM transaction for user signing.",
    ),
    (
        "evm-core",
        "commit_txs",
        "(stub) Submit a batch of EVM transactions for user signing.",
    ),
    (
        "evm-core",
        "commit_eip712",
        "(stub) Request an EIP-712 typed-data signature.",
    ),
    (
        "evm-core",
        "stage_tx",
        "(stub) Stage an unsigned EVM transaction for later commit.",
    ),
    (
        "evm-core",
        "simulate_batch",
        "(stub) Simulate a staged batch.",
    ),
    (
        "evm-core",
        "view_state",
        "(stub) Inspect the agent's user state.",
    ),
    (
        "evm-core",
        "run_tx",
        "(stub) Execute a transaction end-to-end.",
    ),
    (
        "evm-core",
        "get_time_and_onchain_context",
        "(stub) Current block time + chain context.",
    ),
    (
        "evm-core",
        "get_contract",
        "(stub) Fetch a verified contract by address.",
    ),
    (
        "evm-core",
        "get_account_info",
        "(stub) Read EVM account balance + nonce.",
    ),
    (
        "evm-core",
        "sync_chain",
        "(stub) Switch the active EVM chain.",
    ),
    // SVM host primitives.
    (
        "svm-reads",
        "svm_get_context",
        "(stub) Read current Solana wallet and cluster context.",
    ),
    (
        "svm-reads",
        "svm_get_account_info",
        "(stub) Read Solana account metadata.",
    ),
    (
        "svm-reads",
        "svm_get_token_holdings",
        "(stub) Read Solana token holdings.",
    ),
    (
        "svm-reads",
        "svm_get_program",
        "(stub) Read Solana program metadata.",
    ),
    (
        "svm-ix-broadcast",
        "svm_stage_ix",
        "(stub) Stage Solana instructions for later simulation or commit.",
    ),
    (
        "svm-ix-broadcast",
        "svm_simulate_ix",
        "(stub) Simulate staged Solana instructions.",
    ),
    (
        "svm-ix-broadcast",
        "svm_commit_ix",
        "(stub) Request wallet approval for staged Solana instructions.",
    ),
    (
        "svm-ix-sign",
        "svm_stage_ix",
        "(stub) Stage Solana instructions for signing.",
    ),
    (
        "svm-ix-sign",
        "svm_simulate_ix",
        "(stub) Simulate staged Solana instructions.",
    ),
    (
        "svm-tx-broadcast",
        "svm_stage_tx",
        "(stub) Stage a base64 Solana transaction blob.",
    ),
    (
        "svm-tx-broadcast",
        "svm_simulate_tx",
        "(stub) Simulate a staged Solana transaction blob.",
    ),
    (
        "svm-tx-broadcast",
        "svm_commit_tx",
        "(stub) Request wallet approval for a staged Solana transaction blob.",
    ),
    (
        "svm-tx-sign",
        "svm_stage_tx",
        "(stub) Stage a base64 Solana transaction blob for signing.",
    ),
    (
        "svm-tx-sign",
        "svm_simulate_tx",
        "(stub) Simulate a staged Solana transaction blob before signing.",
    ),
    (
        "svm-tx-sign",
        "svm_sign_tx",
        "(stub) Sign a base64 or staged Solana transaction via the host wallet.",
    ),
    (
        "svm-sign-data",
        "svm_sign_data",
        "(stub) Sign off-chain Solana bytes via the host wallet.",
    ),
];

const SVM_CORE_NAMESPACES: &[&str] = &[
    "svm-reads",
    "svm-ix-broadcast",
    "svm-ix-sign",
    "svm-tx-broadcast",
    "svm-tx-sign",
    "svm-sign-data",
    "svm-bundle",
];

fn insert_namespace(set: &mut BTreeSet<String>, namespace: &str) {
    if namespace == "svm-core" {
        set.extend(SVM_CORE_NAMESPACES.iter().map(|ns| ns.to_string()));
    } else {
        set.insert(namespace.to_string());
    }
}

pub fn namespaces_for_manifest(manifest: &DynManifest) -> Vec<String> {
    let mut namespaces = BTreeSet::new();

    if let Some(explicit) = manifest.namespaces.as_deref() {
        for namespace in explicit {
            insert_namespace(&mut namespaces, namespace);
        }
    }

    namespaces.into_iter().collect()
}

/// Build stub tools for every recognised namespace in `requested`.
/// Unknown namespaces emit a warning and contribute zero tools.
pub fn build_stub_tools(requested: &[String]) -> Vec<Box<dyn ToolDyn>> {
    let mut out: Vec<Box<dyn ToolDyn>> = Vec::new();
    let mut seen_tools = BTreeSet::new();

    for ns in requested {
        let mut count = 0usize;
        for &(stub_ns, name, desc) in STUB_REGISTRY {
            if stub_ns == ns && seen_tools.insert(name) {
                out.push(Box::new(StubTool {
                    name,
                    namespace: stub_ns,
                    description: desc.to_string(),
                }) as Box<dyn ToolDyn>);
                count += 1;
            }
        }
        if count == 0 {
            eprintln!(
                "  ⚠ namespace '{ns}' has no stub tools registered; agent will see no tools from it"
            );
        } else {
            eprintln!("  ⚙ stubbed {count} tools for namespace '{ns}'");
        }
    }

    out
}
