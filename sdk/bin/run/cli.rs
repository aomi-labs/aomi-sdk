//! CLI surface for `aomi-run`.
//!
//! Single subcommand-less binary: positional plugin path + a handful of
//! flags for provider/model selection, env-file loading, and turn limit.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use uuid::Uuid;

use crate::{agent, host_stubs, load, repl, secrets, tool};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Provider {
    Anthropic,
    Openai,
    Openrouter,
}

impl Provider {
    /// Default model id when `--model` isn't passed.
    pub fn default_model(self) -> &'static str {
        match self {
            // Latest constants from rig 0.35's provider completion modules.
            Provider::Anthropic => "claude-sonnet-4-6",
            Provider::Openai => "gpt-5",
            // OpenRouter expects `<vendor>/<model>` slugs.
            Provider::Openrouter => "anthropic/claude-sonnet-4",
        }
    }

    /// Name of the env var holding the API key for this provider.
    pub fn api_key_env(self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::Openai => "OPENAI_API_KEY",
            Provider::Openrouter => "OPENROUTER_API_KEY",
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "aomi-run")]
#[command(about = "Run an Aomi plugin against an LLM locally — no backend required.")]
pub struct Cli {
    /// Path to the built plugin shared library (`.dylib` / `.so` / `.dll`).
    pub plugin: PathBuf,

    /// LLM provider to chat with.
    #[arg(long, value_enum, default_value_t = Provider::Anthropic)]
    pub provider: Provider,

    /// Model identifier. Defaults to a sane choice per provider.
    #[arg(long)]
    pub model: Option<String>,

    /// Run one prompt and exit instead of opening the interactive REPL.
    #[arg(long)]
    pub prompt: Option<String>,

    /// Max tool-call rounds rig is allowed inside a single user turn before
    /// it must produce a text response. 0 means "no tool round-trips".
    #[arg(long, default_value_t = 20)]
    pub max_turns: usize,

    /// Cap on output tokens per LLM response.
    #[arg(long, default_value_t = 4096)]
    pub max_tokens: u64,

    /// Optional dotenv file to load before reading any env vars (API key
    /// and plugin secrets).
    #[arg(long, value_name = "FILE")]
    pub env_file: Option<PathBuf>,

    /// Override the session id baked into every `DynToolCallCtx`. Default
    /// is a fresh v4 UUID.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Increase log verbosity (sets `RUST_LOG=aomi_sdk=debug,aomi_run=debug`
    /// if `RUST_LOG` isn't already set).
    #[arg(long, short)]
    pub verbose: bool,
}

impl Cli {
    pub async fn run(self) -> Result<()> {
        init_tracing(self.verbose);

        // Load .env BEFORE we touch any environment-dependent state.
        if let Some(path) = &self.env_file {
            dotenvy::from_filename(path)
                .with_context(|| format!("failed to load env file {}", path.display()))?;
        }

        // Verify the LLM key is present before doing the heavier work of
        // loading the cdylib. Better to fail fast.
        let key_env = self.provider.api_key_env();
        if std::env::var(key_env).is_err() {
            anyhow::bail!(
                "missing API key: set {key_env} (or pass --env-file pointing at a file that defines it)"
            );
        }

        let session_id = self
            .session_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // 1. dlopen the plugin and read its manifest.
        let loaded = load::load_plugin(&self.plugin)?;
        load::print_manifest_summary(&loaded.manifest);

        // 2. Resolve every declared secret slot from env. Required slots
        //    that are missing abort startup.
        let secrets = secrets::resolve(&loaded.manifest)?;

        // 3. Build the rig tool set: one PluginTool per manifest tool, plus
        //    stub tools for any host namespaces the plugin asked for.
        let mut tools = tool::build_plugin_tools(&loaded, session_id.clone(), secrets);
        let host_namespaces = host_stubs::namespaces_for_manifest(&loaded.manifest);
        if !host_namespaces.is_empty() {
            tools.extend(host_stubs::build_stub_tools(&host_namespaces));
        }

        // 4. Build the agent for the chosen provider and start the REPL.
        let model = self
            .model
            .clone()
            .unwrap_or_else(|| self.provider.default_model().to_string());
        let max_turns = self.max_turns;
        let max_tokens = self.max_tokens;
        let preamble = loaded.manifest.preamble.clone();
        let provider = self.provider;

        match provider {
            Provider::Anthropic => {
                let agent = agent::build_anthropic(&model, &preamble, tools, max_tokens)?;
                if let Some(prompt) = &self.prompt {
                    repl::run_prompt(agent, prompt, max_turns).await
                } else {
                    repl::run(agent, &session_id, &loaded.manifest.name, max_turns).await
                }
            }
            Provider::Openai => {
                let agent = agent::build_openai(&model, &preamble, tools, max_tokens)?;
                if let Some(prompt) = &self.prompt {
                    repl::run_prompt(agent, prompt, max_turns).await
                } else {
                    repl::run(agent, &session_id, &loaded.manifest.name, max_turns).await
                }
            }
            Provider::Openrouter => {
                let agent = agent::build_openrouter(&model, &preamble, tools, max_tokens)?;
                if let Some(prompt) = &self.prompt {
                    repl::run_prompt(agent, prompt, max_turns).await
                } else {
                    repl::run(agent, &session_id, &loaded.manifest.name, max_turns).await
                }
            }
        }
    }
}

fn init_tracing(verbose: bool) {
    use tracing_subscriber::{EnvFilter, fmt};
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if verbose {
            EnvFilter::new("aomi_sdk=debug,aomi_run=debug,rig=info")
        } else {
            EnvFilter::new("aomi_sdk=info,aomi_run=info,rig=warn")
        }
    });
    let _ = fmt()
        .with_env_filter(env_filter)
        .with_writer(std::io::stderr)
        .try_init();
}
