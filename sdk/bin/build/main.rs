use clap::{Parser, Subcommand};
use eyre::Result;

use deploy::cli;

mod client;
mod deploy;
mod init;
mod new_package;
mod spec_load;
mod specs;
mod test_schema;
mod tighten;
mod tool;

#[derive(Parser)]
#[command(
    name = "aomi-build",
    version,
    about = "Build, deploy, and activate Aomi apps: spec → client → tool → backend"
)]
struct Cli {
    /// No subcommand launches the interactive wizard (login → deploy → activate).
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Discover or fetch an OpenAPI spec for a platform and write it under the provider's spec path.
    GenSpecs(specs::GenSpecsArgs),
    /// Generate a Rust client from an OpenAPI spec into packages/<platform>/src/client/ or crates/clients/<platform>/.
    GenClient(client::GenClientArgs),
    /// Scaffold an Aomi app from a generated client into packages/<platform>/.
    GenTool(tool::GenToolArgs),
    /// Validate a spec against the live API using Schemathesis (auto-detected runner).
    TestSchema(test_schema::TestSchemaArgs),
    /// Orchestrator: gen-specs → gen-client → gen-tool → cargo build.
    NewPackage(new_package::NewPackageArgs),
    /// Tighten a spec's `additionalProperties: true` response bodies by inferring
    /// schemas from real captured JSON samples in the spec's `.samples/` directory.
    TightenSpec(tighten::TightenSpecArgs),
    /// Scaffold a package under `packages/<NAME>/` (guest `Tool`, `aomi.toml`). Greenfield counterpart
    /// to `new-package` (which is OpenAPI-driven).
    Init(init::InitArgs),
    /// Deploy lifecycle: preflight, run, activate, and status.
    Deploy(Box<cli::DeployArgs>),
    /// Alias for `deploy status` for one release cycle.
    Status(cli::StatusArgs),
    /// Alias for `deploy activate` for one release cycle.
    Activate(cli::ActivateArgs),
    /// Connect: install the Aomi GitHub App and save your activation token.
    Connect(cli::ConnectArgs),
    /// Log in with GitHub and link this CLI to your Aomi Builder account.
    Login(cli::LoginArgs),
    /// Mint an activation token. Operator-only and hidden from `--help`: app
    /// developers are given a token, they never mint one. Hiding is not the
    /// authorization boundary — `token mint` needs the admin signing key, and
    /// the backend verifies that signature.
    #[command(hide = true)]
    Token(cli::TokenArgs),
    /// Create a platform-bound Project from an installed GitHub repository.
    Project(cli::ProjectArgs),
    /// List a platform's apps.
    Apps(cli::AppsArgs),
    /// Ask platform ops for legacy onboarding details.
    Request(cli::RequestArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let Some(cmd) = cli.cmd else {
        return deploy::wizard::run().await.map_err(git_error);
    };
    match cmd {
        Cmd::GenSpecs(args) => specs::run(args),
        Cmd::GenClient(args) => client::run(args),
        Cmd::GenTool(args) => tool::run(args),
        Cmd::TestSchema(args) => test_schema::run(args),
        Cmd::NewPackage(args) => new_package::run(args),
        Cmd::TightenSpec(args) => tighten::run(args),
        Cmd::Init(args) => init::run(args),
        Cmd::Deploy(args) => args.run().await.map_err(git_error),
        Cmd::Status(args) => args.run().await.map_err(git_error),
        Cmd::Activate(args) => cli::deploy::run_activate_step(args)
            .await
            .map_err(git_error),
        Cmd::Connect(args) => args.run().await.map_err(git_error),
        Cmd::Login(args) => args.run().await.map_err(git_error),
        Cmd::Token(args) => args.run().await.map_err(git_error),
        Cmd::Project(args) => args.run().await.map_err(git_error),
        Cmd::Apps(args) => args.run().await.map_err(git_error),
        Cmd::Request(args) => args.run().await.map_err(git_error),
    }
}

fn git_error(err: anyhow::Error) -> eyre::Report {
    eyre::eyre!("{err:#}")
}
