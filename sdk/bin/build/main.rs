use clap::{Parser, Subcommand};
use eyre::Result;

use deploy::cli;

mod client;
mod compile;
mod deploy;
mod init;
mod new_app;
mod spec_load;
mod spec_patch;
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
    /// No subcommand launches the interactive wizard (login → deploy → activate).
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
    /// Deploy tracked `aomi.toml` apps through your Builder account.
    Deploy(cli::DeployArgs),
    /// Show local, Build CI, and backend deployment status.
    Status(cli::StatusArgs),
    /// Activate platform releases by release tag.
    Activate(cli::ActivateArgs),
    /// Admin/legacy setup: install the GitHub App and save an activation token.
    Connect(cli::ConnectArgs),
    /// Log in with GitHub and link this CLI to your Aomi Builder account.
    Login(cli::LoginArgs),
    /// Mint, list, or revoke platform/app activation tokens.
    Token(cli::TokenArgs),
    /// Resolve a connected source repo to its `app_source_id`.
    Source(cli::SourceArgs),
    /// List a platform's apps.
    Apps(cli::AppsArgs),
    /// Ask platform ops for legacy onboarding details.
    Request(cli::RequestArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let Some(cmd) = cli.cmd else {
        return deploy::wizard::run().await.map_err(to_eyre);
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
        Cmd::Deploy(args) => cli::deploy::run(args).await,
        Cmd::Status(args) => cli::status::run(args).await,
        Cmd::Activate(args) => cli::activate::run(args).await,
        Cmd::Connect(args) => cli::connect::run(args).await,
        Cmd::Login(args) => cli::login::run(args).await,
        Cmd::Token(args) => cli::token::run(args).await,
        Cmd::Source(args) => cli::source::run(args).await,
        Cmd::Apps(args) => cli::apps::run(args).await,
        Cmd::Request(args) => cli::request::run(args).await,
    }
}

fn to_eyre(err: anyhow::Error) -> eyre::Report {
    eyre::eyre!("{err:#}")
}
