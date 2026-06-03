//! Build a `rig::agent::Agent` for the chosen provider.
//!
//! Each builder returns the concrete `Agent<M>` for its provider's
//! completion model. The REPL is generic over `M`, so we just pick one
//! at startup and feed it in.

use std::sync::Arc;

use anyhow::Result;
use rig::agent::Agent;
use rig::client::{CompletionClient, ProviderClient};
use rig::providers::{anthropic, openai, openrouter};
use rig::tool::ToolDyn;

/// Anthropic agent (Claude family).
pub fn build_anthropic(
    model: &str,
    preamble: &str,
    tools: Vec<Box<dyn ToolDyn>>,
    max_tokens: u64,
) -> Result<Arc<Agent<anthropic::completion::CompletionModel>>> {
    let client = anthropic::Client::from_env();
    let agent = client
        .agent(model)
        .preamble(preamble)
        .tools(tools)
        .max_tokens(max_tokens)
        .build();
    Ok(Arc::new(agent))
}

/// OpenAI agent (GPT-* family via the Responses API — the default for
/// `openai::Client` in rig 0.35).
pub fn build_openai(
    model: &str,
    preamble: &str,
    tools: Vec<Box<dyn ToolDyn>>,
    max_tokens: u64,
) -> Result<Arc<Agent<openai::responses_api::ResponsesCompletionModel>>> {
    let client = openai::Client::from_env();
    let agent = client
        .agent(model)
        .preamble(preamble)
        .tools(tools)
        .max_tokens(max_tokens)
        .build();
    Ok(Arc::new(agent))
}

/// OpenRouter agent — slug like `anthropic/claude-sonnet-4` or
/// `openai/gpt-4o-mini`.
pub fn build_openrouter(
    model: &str,
    preamble: &str,
    tools: Vec<Box<dyn ToolDyn>>,
    max_tokens: u64,
) -> Result<Arc<Agent<openrouter::CompletionModel>>> {
    let client = openrouter::Client::from_env();
    let agent = client
        .agent(model)
        .preamble(preamble)
        .tools(tools)
        .max_tokens(max_tokens)
        .build();
    Ok(Arc::new(agent))
}
