//! `aomi-build init <NAME>`: scaffold a package under `packages/<NAME>/`.
//!
//! Counterpart to `aomi-build new-package`, which is OpenAPI-driven. `init` is
//! the greenfield path: a guest `Tool`, `package!`, and an `aomi.toml`.

use std::fs;
use std::path::Path;

use clap::Args;
use eyre::{Result, bail};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, value};

use crate::specs::workspace_root;

#[derive(Args, Debug)]
pub struct InitArgs {
    /// New package name. Lowercase alphanumeric with hyphens (e.g. `my-package`).
    pub name: String,
}

const CARGO_TOML: &str = r#"[package]
name = "{name}"
version = "0.1.0"
edition.workspace = true
license.workspace = true
publish.workspace = true

[lib]
crate-type = ["cdylib"]

[dependencies]
aomi-sdk.workspace = true
schemars.workspace = true
serde.workspace = true
serde_json.workspace = true
"#;

const AOMI_TOML: &str = r#"[package]
name = "{name}"
description = "TODO: one line for the Library card."
icon = "icon.svg"
contract = 1

[network]
hosts = []
"#;

/// A placeholder package outline, drawn in `currentColor` like every icon.
const ICON_SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><path d="M12 2 3 7v10l9 5 9-5V7z"/><path d="m3 7 9 5 9-5M12 12v10"/></svg>
"#;

const LIB_RS: &str = r#"//! TODO: describe what this package does.

use aomi_sdk::{Ctx, Outcome, Result, Tool};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ExampleArgs {
    /// A sample input parameter.
    query: String,
}

pub struct Example;

impl Tool for Example {
    const NAME: &'static str = "example";
    const DESCRIPTION: &'static str = "TODO: describe this tool.";
    type Args = ExampleArgs;

    async fn run(args: ExampleArgs, _ctx: &Ctx) -> Result<Outcome> {
        Ok(Outcome::data(json!({ "echo": args.query })))
    }
}

aomi_sdk::package!(Example);
"#;

pub fn run(args: InitArgs) -> Result<()> {
    let name = &args.name;

    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        bail!("package name must be lowercase alphanumeric with hyphens (e.g. 'my-package')");
    }

    let root = workspace_root()?;
    let dir = root.join("packages").join(name);
    if dir.exists() {
        bail!("packages/{name} already exists");
    }
    fs::create_dir_all(dir.join("src"))?;
    fs::write(dir.join("Cargo.toml"), CARGO_TOML.replace("{name}", name))?;
    fs::write(dir.join("aomi.toml"), AOMI_TOML.replace("{name}", name))?;
    fs::write(dir.join("icon.svg"), ICON_SVG)?;
    fs::write(dir.join("src/lib.rs"), LIB_RS)?;
    list_in_project(&root.join("aomi.toml"), name)?;

    println!("created packages/{name}/ (Cargo.toml, aomi.toml, icon.svg, src/lib.rs)");
    println!();
    println!("next steps:");
    println!("  1. fill in aomi.toml (description, hosts)");
    println!(
        "  2. replace icon.svg with your own square SVG drawn in currentColor, or drop the icon line (aomi-build check-icon packages/{name})"
    );
    println!("  3. implement your tools in src/lib.rs");
    println!("  4. cargo build -p {name} --target wasm32-wasip2 --release");
    Ok(())
}

/// Adds `[[packages]] path = "packages/<name>"` to the root `aomi.toml`, if there is one.
fn list_in_project(path: &Path, name: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let mut doc: DocumentMut = fs::read_to_string(path)?.parse()?;
    let packages = doc
        .entry("packages")
        .or_insert(Item::ArrayOfTables(ArrayOfTables::new()))
        .as_array_of_tables_mut()
        .ok_or_else(|| eyre::eyre!("`packages` in aomi.toml is not an array of tables"))?;
    let mut entry = Table::new();
    entry["path"] = value(format!("packages/{name}"));
    packages.push(entry);
    fs::write(path, doc.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaffold_is_the_guest_api() {
        assert!(CARGO_TOML.contains(r#"crate-type = ["cdylib"]"#));
        assert!(LIB_RS.contains("impl Tool for Example"));
        assert!(LIB_RS.contains("aomi_sdk::package!(Example);"));
        assert!(AOMI_TOML.contains("contract = 1"));
    }

    #[test]
    fn placeholder_icon_passes_the_icon_rule() {
        assert!(AOMI_TOML.contains(r#"icon = "icon.svg""#));
        crate::icon::check_icon("icon.svg", ICON_SVG.as_bytes()).unwrap();
    }

    #[test]
    fn listing_appends_a_packages_entry() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("aomi.toml");
        fs::write(
            &file,
            "[project]\npublisher = \"aomi\"\n\n[[packages]]\npath = \"packages/a\"\n",
        )
        .unwrap();
        list_in_project(&file, "b").unwrap();
        let out = fs::read_to_string(&file).unwrap();
        assert!(out.contains("path = \"packages/a\"") && out.contains("path = \"packages/b\""));
    }
}
