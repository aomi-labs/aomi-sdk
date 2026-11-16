use aomi_progenitor_impl::Generator;
use clap::Args;
use eyre::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, Item, Table, value};

use crate::spec_load;
use crate::specs::workspace_root;

#[cfg(test)]
mod tests;

#[derive(Args, Debug)]
pub struct GenClientArgs {
    /// Provider name.
    pub platform: String,

    /// Override the spec path. Defaults to packages/<platform>/openapi.yaml
    /// (or crates/clients/<platform>/openapi.yaml with --shared).
    #[arg(long)]
    pub spec: Option<PathBuf>,

    /// Override the output directory. Defaults to packages/<platform>/src/client/
    /// (or the crate directory crates/clients/<platform>/ with --shared).
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Overwrite generated files that already exist.
    #[arg(long)]
    pub force: bool,

    /// Generate a shared client crate under crates/clients/. Default is a
    /// module inside the package.
    #[arg(long)]
    pub shared: bool,

    /// Write nothing; fail if any generated file or manifest edit differs from disk.
    #[arg(long)]
    pub check: bool,

    /// Also write the preprocessed spec (the OpenAPI after the patches) here.
    #[arg(long)]
    pub preprocessed: Option<PathBuf>,
}

/// Where a generated client lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// A module in `packages/<platform>/src/client/`; the tool layer uses `crate::client`.
    AppLocal,
    /// A crate `crates/clients/<platform>` named `<platform>-client`.
    Shared,
}

impl Mode {
    pub(crate) fn spec_path(self, root: &Path, platform: &str) -> PathBuf {
        match self {
            Self::AppLocal => root.join("packages").join(platform).join("openapi.yaml"),
            Self::Shared => self.out_dir(root, platform).join("openapi.yaml"),
        }
    }

    fn out_dir(self, root: &Path, platform: &str) -> PathBuf {
        match self {
            Self::AppLocal => root
                .join("packages")
                .join(platform)
                .join("src")
                .join("client"),
            Self::Shared => root.join("crates").join("clients").join(platform),
        }
    }
}

/// Every file a run would write, so `--check` and a normal run share one computation.
#[derive(Default)]
struct Plan(Vec<(PathBuf, String)>);

impl Plan {
    fn add(&mut self, path: PathBuf, text: String) {
        self.0.push((path, text));
    }

    fn drifted(&self) -> Vec<(&Path, &'static str)> {
        self.0
            .iter()
            .filter_map(|(path, text)| match std::fs::read_to_string(path) {
                Ok(on_disk) if on_disk == *text => None,
                Ok(_) => Some((path.as_path(), "differs")),
                Err(_) => Some((path.as_path(), "missing")),
            })
            .collect()
    }

    fn write(&self) -> Result<()> {
        for (path, text) in &self.0 {
            if std::fs::read_to_string(path).is_ok_and(|on_disk| on_disk == *text) {
                continue;
            }
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)
                    .with_context(|| format!("failed to create {}", dir.display()))?;
            }
            std::fs::write(path, text)
                .with_context(|| format!("failed to write {}", path.display()))?;
            println!("✓ wrote {}", path.display());
        }
        Ok(())
    }
}

pub fn run(args: GenClientArgs) -> Result<()> {
    run_in(&workspace_root()?, args)
}

fn run_in(root: &Path, args: GenClientArgs) -> Result<()> {
    let mode = if args.shared {
        Mode::Shared
    } else {
        Mode::AppLocal
    };
    let platform = args.platform.as_str();
    let spec_path = args
        .spec
        .clone()
        .unwrap_or_else(|| mode.spec_path(root, platform));
    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| mode.out_dir(root, platform));
    let (src_dir, mod_name) = match mode {
        Mode::AppLocal => (out_dir.clone(), "mod.rs"),
        Mode::Shared => (out_dir.join("src"), "lib.rs"),
    };
    let client_path = src_dir.join("client.rs");
    let mod_path = src_dir.join(mod_name);

    if !args.check && !args.force {
        for path in [&client_path, &mod_path] {
            if path.exists() {
                bail!(
                    "{} already exists. Pass --force to overwrite.",
                    path.display()
                );
            }
        }
    }

    println!("Reading spec: {}", spec_path.display());
    let spec_bytes = std::fs::read(&spec_path)
        .with_context(|| format!("failed to read {}", spec_path.display()))?;
    let spec = spec_load::load_and_preprocess(&spec_path)?;

    let tokens = Generator::default()
        .generate_tokens(&spec)
        .map_err(|e| eyre::eyre!("generation failed: {e}"))?;
    let generated = format_tokens(tokens.to_string())?;
    let detected = detect_generated_deps(&generated);
    let file_name = spec_path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let client_src = format!(
        "// @generated by `aomi-build gen-client` {} from {file_name}\n// spec sha256: {}\n{generated}",
        env!("CARGO_PKG_VERSION"),
        hex(&Sha256::digest(&spec_bytes)),
    );

    let mut plan = Plan::default();
    plan.add(client_path, client_src);
    plan.add(mod_path, generated_mod_rs(platform));
    if let Some(path) = &args.preprocessed {
        plan.add(path.clone(), spec_load::preprocessed_yaml(&spec)?);
    }
    wire_manifests(root, &out_dir, mode, platform, &detected, &mut plan)?;

    if args.check {
        let drift = plan.drifted();
        for (path, why) in &drift {
            println!("drift: {} {why}", path.display());
        }
        if !drift.is_empty() {
            bail!(
                "{} generated file(s) are out of date; re-run without --check",
                drift.len()
            );
        }
        println!("✓ {platform} client is up to date");
        return Ok(());
    }
    plan.write()
}

/// Plans the manifest edits: the workspace dependency table, and either the
/// client crate's manifest (shared) or the package's (app-local).
fn wire_manifests(
    root: &Path,
    out_dir: &Path,
    mode: Mode,
    platform: &str,
    deps: &DetectedDeps,
    plan: &mut Plan,
) -> Result<()> {
    let root_path = root.join("Cargo.toml");
    let root_text = std::fs::read_to_string(&root_path)
        .with_context(|| format!("failed to read {}", root_path.display()))?;
    plan.add(root_path, ensure_workspace(&root_text, mode)?);

    let (path, text) = match mode {
        Mode::Shared => {
            let path = out_dir.join("Cargo.toml");
            let text = std::fs::read_to_string(&path).unwrap_or_else(|_| {
                format!(
                    "[package]\nname = \"{platform}-client\"\nversion = \"0.1.0\"\nedition = \"2024\"\npublish = false\n"
                )
            });
            (path, text)
        }
        Mode::AppLocal => {
            let path = root.join("packages").join(platform).join("Cargo.toml");
            // No package crate yet: gen-tool writes the manifest with these deps.
            let Ok(text) = std::fs::read_to_string(&path) else {
                return Ok(());
            };
            (path, text)
        }
    };
    plan.add(path, ensure_deps(&text, deps)?);
    Ok(())
}

const SDK_REPO: &str = "https://github.com/aomi-labs/aomi-sdk";
const SDK_BRANCH: &str = "feat/unified-packages";

/// Makes sure the workspace declares `aomi-sdk` and `aomi-progenitor-client`
/// once, and (for shared crates) that `crates/clients/*` is a member.
fn ensure_workspace(text: &str, mode: Mode) -> Result<String> {
    let mut doc: DocumentMut = text
        .parse()
        .context("workspace Cargo.toml is not valid TOML")?;
    let ws = doc
        .get_mut("workspace")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| eyre::eyre!("Cargo.toml has no [workspace] table"))?;
    let deps = table_in(ws, "dependencies");
    for (name, off) in [("aomi-sdk", true), ("aomi-progenitor-client", false)] {
        if !deps.contains_key(name) {
            let mut git = toml_edit::InlineTable::new();
            git.insert("git", SDK_REPO.into());
            git.insert("branch", SDK_BRANCH.into());
            if off {
                git.insert("default-features", false.into());
            }
            deps.insert(name, value(git));
        }
    }
    if mode == Mode::Shared {
        const GLOB: &str = "crates/clients/*";
        let members = ws
            .entry("members")
            .or_insert_with(|| value(toml_edit::Array::new()))
            .as_array_mut()
            .ok_or_else(|| eyre::eyre!("workspace.members is not an array"))?;
        if !members.iter().any(|m| m.as_str() == Some(GLOB)) {
            members.push(GLOB);
        }
    }
    Ok(doc.to_string())
}

/// Adds the dependencies the generated client needs to `[dependencies]`,
/// leaving any entry the author already wrote alone.
pub(crate) fn ensure_deps(text: &str, deps: &DetectedDeps) -> Result<String> {
    let mut doc: DocumentMut = text.parse().context("Cargo.toml is not valid TOML")?;
    let table = table_in(doc.as_table_mut(), "dependencies");
    let mut want: Vec<(&str, &str)> = vec![
        ("aomi-sdk", "{ workspace = true }"),
        ("aomi-progenitor-client", "{ workspace = true }"),
        ("serde", "{ version = \"1.0\", features = [\"derive\"] }"),
        ("serde_json", "\"1.0\""),
    ];
    if deps.needs_chrono {
        want.push((
            "chrono",
            "{ version = \"0.4\", default-features = false, features = [\"alloc\", \"serde\"] }",
        ));
    }
    if deps.needs_uuid {
        want.push((
            "uuid",
            "{ version = \"1\", default-features = false, features = [\"serde\"] }",
        ));
    }
    if deps.needs_regress {
        want.push(("regress", "\"0.10\""));
    }
    for (name, spec) in want {
        if !table.contains_key(name) {
            let item: toml_edit::Value = spec.parse().expect("dependency literals are valid TOML");
            table.insert(name, value(item));
        }
    }
    Ok(doc.to_string())
}

fn table_in<'a>(parent: &'a mut Table, key: &str) -> &'a mut Table {
    parent
        .entry(key)
        .or_insert_with(|| Item::Table(Table::new()))
        .as_table_mut()
        .expect("dependency tables are tables")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Dependencies detected by scanning the generated client source: `chrono`
/// for date-time formats, `uuid` for `format: uuid`, `regress` for `pattern:`.
#[derive(Default, Debug)]
pub(crate) struct DetectedDeps {
    needs_chrono: bool,
    needs_uuid: bool,
    needs_regress: bool,
}

pub(crate) fn detect_generated_deps(src: &str) -> DetectedDeps {
    DetectedDeps {
        needs_chrono: src.contains("chrono::") || src.contains(": chrono "),
        needs_uuid: src.contains("uuid::") || src.contains(": uuid "),
        needs_regress: src.contains("regress::"),
    }
}

fn format_tokens(raw: String) -> Result<String> {
    let parsed: syn::File = syn::parse_str(&raw).with_context(
        || "the generator produced unparseable Rust — this is a bug in the spec or the generator",
    )?;
    Ok(prettyplease::unparse(&parsed))
}

fn generated_mod_rs(platform: &str) -> String {
    format!(
        "//! Generated by `aomi-build gen-client {platform}`.\n\
         //! Do not edit by hand; re-run with --force to regenerate, --check to verify.\n\n\
         #[allow(clippy::all, dead_code, unused_imports)]\n\
         pub mod client;\n\n\
         pub use client::*;\n"
    )
}
