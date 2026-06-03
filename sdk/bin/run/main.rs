//! `aomi-run` — minimal local runtime for Aomi dynamic plugins.
//!
//! Loads a built plugin `cdylib`, reads its manifest, wires every declared
//! tool into a [`rig::agent::Agent`], stubs out any host-side namespaces the
//! plugin asks for, and opens an interactive REPL so developers can chat
//! with their agent without round-tripping through the backend.
//!
//! # What works
//!
//! - All `DynAomiTool`s the plugin exports (sync + async — async tools are
//!   blocked-on internally via `DynFnHandle::call_exec_tool`).
//! - Per-app secrets sourced from environment variables (and an optional
//!   `--env-file`), exposed to the plugin via `DynToolCallCtx.secrets`.
//! - Three LLM providers behind `--provider`: Anthropic, OpenAI, OpenRouter.
//!
//! # What is intentionally NOT supported in v1
//!
//! - Routed `ToolReturn` envelopes (`commit_eip712`, `stage_tx`,
//!   `svm_sign_tx`, …) — host-side wallet UX is missing, so the agent
//!   receives the envelope as opaque JSON and routes never fire.
//! - Skill activation.
//! - The host-side namespace toolsets (`evm-core`, `database`, …)
//!   are replaced with stub tools that return an "unavailable in dev
//!   runtime" sentinel value. The LLM still sees the tools by name but any
//!   call resolves to a no-op note rather than real behavior.
//! - `$SECRET:…` argument substitution. The plugin's
//!   `resolve_secret_value(ctx, …)` still works because its env-var
//!   fallback path is preserved.
//! - State attributes (`ctx.attribute_*` always returns `None`).
//!
//! For any of the above, deploy the plugin and exercise it against the
//! real backend.

mod agent;
mod cli;
mod host_stubs;
mod load;
mod repl;
mod secrets;
mod tool;

use anyhow::Result;
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    cli::Cli::parse().run().await
}
