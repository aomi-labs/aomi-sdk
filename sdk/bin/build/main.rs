use clap::{Parser, Subcommand};
use eyre::Result;

#[path = "hosted/app.rs"]
mod app;
#[path = "hosted/backend.rs"]
mod backend;
#[allow(dead_code)]
#[path = "hosted/cli.rs"]
mod cli;
mod client;
mod compile;
#[allow(dead_code)]
#[path = "hosted/discord.rs"]
mod discord;
mod init;
mod new_app;
#[allow(dead_code)]
#[path = "hosted/platform.rs"]
mod platform;
mod spec_load;
mod specs;
#[path = "hosted/status.rs"]
mod status;
mod test_schema;
mod tighten;
mod tool;
#[path = "hosted/types.rs"]
mod types;

#[cfg(test)]
#[path = "hosted/tests.rs"]
mod tests;

#[derive(Parser)]
#[command(
    name = "aomi-build",
    about = "Build, deploy, and activate Aomi apps: spec → client → tool → backend"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Discover or fetch an OpenAPI spec for a platform and write it under ext/specs/.
    GenSpecs(specs::GenSpecsArgs),
    /// Generate a Rust client from an OpenAPI spec into ext/src/<platform>/.
    GenClient(client::GenClientArgs),
    /// Scaffold an Aomi app from a generated client into apps/<platform>/.
    GenTool(tool::GenToolArgs),
    /// Validate a spec against the live API using Schemathesis (auto-detected runner).
    TestSchema(test_schema::TestSchemaArgs),
    /// Orchestrator: gen-specs → gen-client → gen-tool → cargo build.
    NewApp(new_app::NewAppArgs),
    /// Tighten a spec's `additionalProperties: true` response bodies by inferring
    /// schemas from real captured JSON samples in `ext/specs/<platform>.samples/`.
    TightenSpec(tighten::TightenSpecArgs),
    /// Scaffold a bare app skeleton under `apps/<NAME>/`. Greenfield counterpart
    /// to `new-app` (which is OpenAPI-driven).
    Init(init::InitArgs),
    /// Build every app's cdylib, copy validated plugins into `plugins/`,
    /// codesign on macOS.
    Compile(compile::CompileArgs),
    /// Deploy tracked `aomi.toml` apps from a source ref through the backend.
    Deploy(cli::DeployArgs),
    /// Show local + backend deployment status.
    Status(cli::StatusArgs),
    /// Activate platform releases from a PR, branch, commit, or release tag.
    Activate(cli::ActivateArgs),
    /// Ask platform ops for legacy onboarding details.
    Request(cli::RequestArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::GenSpecs(args) => specs::run(args),
        Cmd::GenClient(args) => client::run(args),
        Cmd::GenTool(args) => tool::run(args),
        Cmd::TestSchema(args) => test_schema::run(args),
        Cmd::NewApp(args) => new_app::run(args),
        Cmd::TightenSpec(args) => tighten::run(args),
        Cmd::Init(args) => init::run(args),
        Cmd::Compile(args) => compile::run(args),
        Cmd::Deploy(args) => args.run().await.map_err(git_error),
        Cmd::Status(args) => args.run().await.map_err(git_error),
        Cmd::Activate(args) => args.run().await.map_err(git_error),
        Cmd::Request(args) => args.run().await.map_err(git_error),
    }
}

fn git_error(err: anyhow::Error) -> eyre::Report {
    eyre::eyre!("{err:#}")
}
