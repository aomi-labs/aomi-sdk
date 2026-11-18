use clap::{Parser, Subcommand};
use eyre::Result;

mod client;
mod icon;
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
    about = "Build Aomi packages: spec → client → tool → component"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Discover or fetch an OpenAPI spec for a platform and write it under the provider's spec path.
    GenSpecs(specs::GenSpecsArgs),
    /// Generate a Rust client from an OpenAPI spec into packages/<platform>/src/client/ or crates/clients/<platform>/.
    GenClient(client::GenClientArgs),
    /// Scaffold an Aomi package from a generated client into packages/<platform>/.
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
    /// Check a package folder's `[package] icon` against the Library icon rule the host enforces.
    CheckIcon(icon::CheckIconArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::GenSpecs(args) => specs::run(args),
        Cmd::GenClient(args) => client::run(args),
        Cmd::GenTool(args) => tool::run(args),
        Cmd::TestSchema(args) => test_schema::run(args),
        Cmd::NewPackage(args) => new_package::run(args),
        Cmd::TightenSpec(args) => tighten::run(args),
        Cmd::Init(args) => init::run(args),
        Cmd::CheckIcon(args) => icon::run(args),
    }
}
