use clap::{Parser, Subcommand};
use eyre::Result;

mod client;
mod compile;
mod hosted;
mod init;
mod new_app;
mod spec_load;
mod specs;
mod test_schema;
mod tighten;
mod tool;

#[derive(Parser)]
#[command(
    name = "aomi-build",
    about = "Build, deploy, and activate Aomi apps: spec → client → tool → backend"
)]
struct Cli {
    /// No subcommand launches the interactive wizard (connect → deploy → activate).
    #[command(subcommand)]
    cmd: Option<Cmd>,
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
    Deploy(hosted::cli::DeployArgs),
    /// Show local + backend deployment status.
    Status(hosted::cli::StatusArgs),
    /// Activate platform releases by release tag.
    Activate(hosted::cli::ActivateArgs),
    /// Connect: install the Aomi GitHub App and save your activation token.
    Connect(hosted::cli::ConnectArgs),
    /// Mint, list, or revoke platform/app activation tokens.
    Token(hosted::cli::TokenArgs),
    /// Resolve a connected source repo to its `app_source_id`.
    Source(hosted::cli::SourceArgs),
    /// List a platform's apps.
    Apps(hosted::cli::AppsArgs),
    /// Ask platform ops for legacy onboarding details.
    Request(hosted::cli::RequestArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let Some(cmd) = cli.cmd else {
        return hosted::wizard::run().await.map_err(git_error);
    };
    match cmd {
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
        Cmd::Connect(args) => args.run().await.map_err(git_error),
        Cmd::Token(args) => args.run().await.map_err(git_error),
        Cmd::Source(args) => args.run().await.map_err(git_error),
        Cmd::Apps(args) => args.run().await.map_err(git_error),
        Cmd::Request(args) => args.run().await.map_err(git_error),
    }
}

fn git_error(err: anyhow::Error) -> eyre::Report {
    eyre::eyre!("{err:#}")
}
