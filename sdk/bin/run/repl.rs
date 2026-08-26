//! Interactive REPL — read stdin lines, stream model output, loop.
//!
//! This is the entire agentic loop: one stdin line per user turn, one
//! `agent.stream_prompt(...).with_history(...).multi_turn(N).await` per
//! turn, and rig handles every tool round-trip inside that single stream.
//! On `FinalResponse` we capture the updated history so the next turn
//! preserves context.

use std::io::{self, Write};
use std::sync::Arc;

use anyhow::Result;
use futures::StreamExt;
use rig::agent::{Agent, MultiTurnStreamItem};
use rig::completion::{CompletionModel, GetTokenUsage};
use rig::message::Message;
use rig::streaming::{StreamedAssistantContent, StreamingPrompt};
use rig::wasm_compat::WasmCompatSend;
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;

/// Run the chat loop against a built agent.
///
/// Generic over the provider's completion model so the same loop works
/// for Anthropic, OpenAI, and OpenRouter.
pub async fn run<M>(
    agent: Arc<Agent<M>>,
    session_id: &str,
    app_name: &str,
    max_turns: usize,
) -> Result<()>
where
    M: CompletionModel + 'static,
    M::StreamingResponse: WasmCompatSend + GetTokenUsage + Clone + Unpin,
{
    print_banner(app_name, session_id, max_turns);

    let mut rl = DefaultEditor::new()?;
    let mut history: Vec<Message> = Vec::new();

    loop {
        let line = match rl.readline("you ▸ ") {
            Ok(line) => line,
            // Ctrl-D / Ctrl-C → graceful exit.
            Err(ReadlineError::Eof) | Err(ReadlineError::Interrupted) => {
                eprintln!("bye.");
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let _ = rl.add_history_entry(trimmed);

        // Slash commands take precedence over LLM prompts.
        if let Some(rest) = trimmed.strip_prefix('/') {
            match handle_slash(rest, &mut history) {
                SlashOutcome::Quit => return Ok(()),
                SlashOutcome::Handled => continue,
            }
        }

        // Run one user turn: rig will internally execute any tool calls
        // the model emits and continue streaming until it produces a
        // final text response (or hits `multi_turn`).
        if let Err(e) = run_one_turn(&agent, trimmed, &mut history, max_turns).await {
            eprintln!("\n⚠ turn failed: {e:#}");
        }
    }
}

/// Run one prompt and exit. This is the non-interactive path used by
/// orchestrators such as `aomi-workbench` for smoke tests.
pub async fn run_prompt<M>(agent: Arc<Agent<M>>, prompt: &str, max_turns: usize) -> Result<()>
where
    M: CompletionModel + 'static,
    M::StreamingResponse: WasmCompatSend + GetTokenUsage + Clone + Unpin,
{
    let mut history: Vec<Message> = Vec::new();
    run_one_turn(&agent, prompt, &mut history, max_turns).await
}

async fn run_one_turn<M>(
    agent: &Arc<Agent<M>>,
    prompt: &str,
    history: &mut Vec<Message>,
    max_turns: usize,
) -> Result<()>
where
    M: CompletionModel + 'static,
    M::StreamingResponse: WasmCompatSend + GetTokenUsage + Clone + Unpin,
{
    let mut stream = agent
        .stream_prompt(prompt.to_string())
        .with_history(history.clone())
        .multi_turn(max_turns)
        .await;

    print!("bot ▸ ");
    io::stdout().flush().ok();

    while let Some(item) = stream.next().await {
        match item? {
            MultiTurnStreamItem::StreamAssistantItem(content) => match content {
                StreamedAssistantContent::Text(text) => {
                    print!("{}", text.text);
                    io::stdout().flush().ok();
                }
                StreamedAssistantContent::Reasoning(reasoning) => {
                    // Show reasoning text dim-prefixed so it doesn't blend
                    // into the assistant's final answer. `display_text`
                    // joins text/summary/redacted blocks; encrypted blocks
                    // are dropped (they're not human-readable).
                    let text = reasoning.display_text();
                    if !text.is_empty() {
                        eprintln!("\n  ∙ {text}");
                    }
                }
                StreamedAssistantContent::ToolCall { tool_call, .. } => {
                    // Compact one-liner per tool call. Helpful for watching
                    // the agent navigate a multi-tool plan.
                    let args = tool_call.function.arguments.to_string();
                    let preview = preview_args(&args);
                    eprintln!("\n  🔧 {}({preview})", tool_call.function.name);
                }
                // Deltas and final response markers are accumulated by rig
                // itself — nothing extra to render in a basic REPL.
                StreamedAssistantContent::ToolCallDelta { .. }
                | StreamedAssistantContent::ReasoningDelta { .. }
                | StreamedAssistantContent::Final(_) => {}
            },
            MultiTurnStreamItem::StreamUserItem(_user_item) => {
                // Tool results coming back from a dispatched tool. rig
                // already feeds these into the conversation; the REPL has
                // no extra job beyond a tiny visual hint.
                eprint!(" ✓");
            }
            MultiTurnStreamItem::FinalResponse(fr) => {
                if let Some(updated) = fr.history() {
                    *history = updated.to_vec();
                }
                let usage = fr.usage();
                if usage.input_tokens > 0 || usage.output_tokens > 0 {
                    eprintln!(
                        "\n  [tokens: in={} out={} total={}]",
                        usage.input_tokens, usage.output_tokens, usage.total_tokens,
                    );
                }
            }
            // MultiTurnStreamItem is #[non_exhaustive]; future rig
            // versions may add variants. Ignore them rather than block
            // the build.
            _ => {}
        }
    }

    // Always move to a fresh line before the next "you ▸ " prompt.
    println!();
    Ok(())
}

enum SlashOutcome {
    Quit,
    Handled,
}

fn handle_slash(rest: &str, history: &mut Vec<Message>) -> SlashOutcome {
    let cmd = rest.split_whitespace().next().unwrap_or("");
    match cmd {
        "quit" | "exit" | "q" => SlashOutcome::Quit,
        "reset" | "clear" => {
            history.clear();
            eprintln!("(history cleared)");
            SlashOutcome::Handled
        }
        "history" => {
            eprintln!("(history: {} messages)", history.len());
            SlashOutcome::Handled
        }
        "help" | "?" => {
            eprintln!("commands: /quit  /reset  /history  /help");
            SlashOutcome::Handled
        }
        _ => {
            eprintln!("unknown command /{cmd} (try /help)");
            SlashOutcome::Handled
        }
    }
}

fn print_banner(app: &str, session_id: &str, max_turns: usize) {
    eprintln!("─────────────────────────────────────────");
    eprintln!(" aomi-run REPL · app={app} · session={session_id}");
    eprintln!(" max_turns={max_turns} · /help for commands");
    eprintln!("─────────────────────────────────────────");
}

/// Truncate noisy JSON arg blobs for the call-trace line.
fn preview_args(s: &str) -> String {
    const MAX: usize = 80;
    if s.len() <= MAX {
        s.to_string()
    } else {
        format!("{}… ({} bytes)", &s[..MAX], s.len())
    }
}
