use clap::Args;
use eyre::{Context, Result, bail};
use progenitor::{GenerationSettings, Generator, InterfaceStyle};
use std::path::PathBuf;

use crate::spec_load;
use crate::specs::workspace_root;

#[derive(Args, Debug)]
pub struct GenClientArgs {
    /// Platform name.
    pub platform: String,

    /// Override the spec path. Defaults to apps/<platform>/openapi.yaml
    /// (or ext/specs/<platform>.yaml when --shared is set).
    #[arg(long)]
    pub spec: Option<PathBuf>,

    /// Override the output directory. Defaults to apps/<platform>/src/client/
    /// (or ext/src/<platform>/ when --shared is set).
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Overwrite existing module at the output directory.
    #[arg(long)]
    pub force: bool,

    /// Treat as a shared library (lives under ext/). Default is app-local.
    #[arg(long)]
    pub shared: bool,
}

pub fn run(args: GenClientArgs) -> Result<()> {
    let root = workspace_root()?;
    let spec_path = args.spec.clone().unwrap_or_else(|| {
        if args.shared {
            root.join("ext")
                .join("specs")
                .join(format!("{}.yaml", args.platform))
        } else {
            root.join("apps").join(&args.platform).join("openapi.yaml")
        }
    });
    let out_dir = args.out.clone().unwrap_or_else(|| {
        if args.shared {
            root.join("ext").join("src").join(&args.platform)
        } else {
            root.join("apps")
                .join(&args.platform)
                .join("src")
                .join("client")
        }
    });

    if out_dir.exists() && !args.force {
        bail!(
            "{} already exists. Pass --force to overwrite.",
            out_dir.display()
        );
    }

    println!("Reading spec: {}", spec_path.display());
    let spec = spec_load::load_and_preprocess(&spec_path)?;

    println!("Running progenitor...");
    let mut generator = Generator::new(
        GenerationSettings::default()
            .with_interface(InterfaceStyle::Positional)
            .with_tag(progenitor::TagStyle::Merged),
    );
    let tokens = generator
        .generate_tokens(&spec)
        .map_err(|e| eyre::eyre!("progenitor failed: {e}"))?;

    let formatted = format_tokens(tokens.to_string())?;

    std::fs::create_dir_all(&out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;

    let client_path = out_dir.join("client.rs");
    std::fs::write(&client_path, &formatted)
        .with_context(|| format!("failed to write {}", client_path.display()))?;

    let mod_path = out_dir.join("mod.rs");
    let mod_body = generated_mod_rs(&args.platform);
    std::fs::write(&mod_path, mod_body)
        .with_context(|| format!("failed to write {}", mod_path.display()))?;

    println!("✓ wrote {}", client_path.display());
    println!("✓ wrote {}", mod_path.display());

    let detected = detect_generated_deps(&formatted);
    if args.shared {
        let updated = wire_into_ext(&root, &args.platform, &detected)?;
        if updated.cargo_toml {
            println!(
                "✓ updated ext/Cargo.toml feature `{}` ({})",
                args.platform,
                detected.feature_deps().join(", ")
            );
        }
        if updated.lib_rs {
            println!("✓ added `pub mod {}` to ext/src/lib.rs", args.platform);
        }
    } else {
        let updated = wire_into_app(&root, &args.platform, &detected)?;
        if updated {
            println!(
                "✓ updated apps/{}/Cargo.toml deps ({})",
                args.platform,
                detected
                    .feature_deps()
                    .into_iter()
                    .map(|d| d.trim_start_matches("dep:").to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }
    Ok(())
}

/// Ensure apps/<platform>/Cargo.toml has the runtime deps the generated
/// client needs: progenitor-client, chrono, uuid, regress (per detection),
/// reqwest, serde, etc. Returns true iff the file was modified.
fn wire_into_app(root: &std::path::Path, platform: &str, deps: &DetectedDeps) -> Result<bool> {
    let cargo_path = root.join("apps").join(platform).join("Cargo.toml");
    if !cargo_path.exists() {
        // No app crate yet — gen-tool will create the Cargo.toml with the
        // right deps already. Nothing to wire.
        return Ok(false);
    }
    let mut text = std::fs::read_to_string(&cargo_path)?;
    let mut changed = false;
    let lines_to_ensure = build_app_dep_lines(deps);
    for line in &lines_to_ensure {
        let key = line.split('=').next().unwrap().trim();
        let already_present = text
            .lines()
            .any(|l| l.trim_start().starts_with(&format!("{key} =")));
        if !already_present {
            text.push('\n');
            text.push_str(line);
            text.push('\n');
            changed = true;
        }
    }
    if changed {
        std::fs::write(&cargo_path, text)?;
    }
    Ok(changed)
}

fn build_app_dep_lines(deps: &DetectedDeps) -> Vec<String> {
    let mut out = vec![
        "progenitor-client = \"0.14\"".to_string(),
        "reqwest = { version = \"0.13\", default-features = false, features = [\"json\", \"rustls\"] }".to_string(),
    ];
    if deps.needs_chrono {
        out.push("chrono = { version = \"0.4\", features = [\"serde\"] }".to_string());
    }
    if deps.needs_uuid {
        out.push("uuid = { version = \"1\", features = [\"serde\", \"v4\"] }".to_string());
    }
    if deps.needs_regress {
        out.push("regress = \"0.10\"".to_string());
    }
    out
}

/// Per-provider dep set detected by scanning the generated client source.
#[derive(Default, Debug)]
struct DetectedDeps {
    needs_chrono: bool,
    needs_uuid: bool,
    needs_regress: bool,
}

impl DetectedDeps {
    fn feature_deps(&self) -> Vec<String> {
        // progenitor-client is always needed for generated clients.
        let mut v = vec!["dep:progenitor-client".to_string()];
        if self.needs_chrono {
            v.push("dep:chrono".to_string());
        }
        if self.needs_uuid {
            v.push("dep:uuid".to_string());
        }
        if self.needs_regress {
            v.push("dep:regress".to_string());
        }
        v
    }

    fn render_feature_line(&self, platform: &str) -> String {
        let deps = self
            .feature_deps()
            .into_iter()
            .map(|d| format!("\"{d}\""))
            .collect::<Vec<_>>()
            .join(", ");
        format!("{platform} = [{deps}]")
    }
}

/// Scan the generated client source for dep markers. progenitor pulls in
/// `chrono` for date/time formats, `uuid` for `format: uuid` strings, and
/// `regress` for `pattern:` constraints. Each shows up as a `use` or path
/// reference in the generated code.
fn detect_generated_deps(src: &str) -> DetectedDeps {
    DetectedDeps {
        needs_chrono: src.contains("chrono::") || src.contains(": chrono "),
        needs_uuid: src.contains("uuid::") || src.contains(": uuid "),
        needs_regress: src.contains("regress::"),
    }
}

/// Pull `dep:foo` strings out of an existing feature line like:
///   `bybit = ["dep:progenitor-client", "dep:chrono", "dep:hmac", "dep:sha2"]`
fn parse_feature_deps(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'"' {
                j += 1;
            }
            if let Ok(s) = std::str::from_utf8(&bytes[start..j]) {
                out.push(s.to_string());
            }
            i = j + 1;
        } else {
            i += 1;
        }
    }
    out
}

#[derive(Default)]
struct WireUpdate {
    cargo_toml: bool,
    lib_rs: bool,
}

/// Ensure ext/Cargo.toml declares the platform's feature with the right deps,
/// and ext/src/lib.rs declares `pub mod <platform>;` gated by the feature.
fn wire_into_ext(
    root: &std::path::Path,
    platform: &str,
    deps: &DetectedDeps,
) -> Result<WireUpdate> {
    let mut up = WireUpdate::default();

    let cargo_path = root.join("ext").join("Cargo.toml");
    let cargo = std::fs::read_to_string(&cargo_path)?;
    let stub_re = format!("{platform} = [");
    // Merge: union of (existing line's deps) + (newly detected deps). This
    // preserves hand-added deps (like `dep:hmac` for HMAC-signing platforms)
    // when re-running gen-client.
    let merged_line = if let Some(existing) = cargo.lines().find(|l| l.trim().starts_with(&stub_re))
    {
        let mut all: Vec<String> = parse_feature_deps(existing)
            .into_iter()
            .chain(deps.feature_deps())
            .collect();
        all.sort();
        all.dedup();
        format!(
            "{platform} = [{}]",
            all.into_iter()
                .map(|d| format!("\"{d}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        deps.render_feature_line(platform)
    };

    if !cargo.contains(&merged_line) {
        let new_cargo = if cargo.lines().any(|l| l.trim().starts_with(&stub_re)) {
            cargo
                .lines()
                .map(|l| {
                    if l.trim().starts_with(&stub_re) {
                        merged_line.clone()
                    } else {
                        l.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("\n")
                + "\n"
        } else {
            cargo.replacen("default = []", &format!("default = []\n{merged_line}"), 1)
        };
        std::fs::write(&cargo_path, new_cargo)?;
        up.cargo_toml = true;
    }

    let lib_path = root.join("ext").join("src").join("lib.rs");
    let lib = std::fs::read_to_string(&lib_path)?;
    let pub_mod_line = format!("#[cfg(feature = \"{platform}\")]\npub mod {platform};");
    let one_liner = format!(r#"#[cfg(feature = "{platform}")] pub mod {platform};"#);
    if !lib.contains(&pub_mod_line) && !lib.contains(&one_liner) {
        let appended = format!("{}\n{pub_mod_line}\n", lib.trim_end());
        std::fs::write(&lib_path, appended)?;
        up.lib_rs = true;
    }
    Ok(up)
}

fn format_tokens(raw: String) -> Result<String> {
    let parsed: syn::File = syn::parse_str(&raw).with_context(
        || "progenitor produced unparseable Rust — this is a bug in the spec or the generator",
    )?;
    Ok(prettyplease::unparse(&parsed))
}

fn generated_mod_rs(platform: &str) -> String {
    format!(
        "//! Generated by `aomi-build gen-client {platform}`.\n\
         //! Do not edit by hand — re-run with --force to regenerate.\n\
         //!\n\
         //! Hand-written companions (signing helpers, etc.) live next to this file\n\
         //! in `auth.rs` or other sibling modules.\n\n\
         #[allow(clippy::all, dead_code, unused_imports)]\n\
         pub mod client;\n\n\
         pub use client::*;\n"
    )
}
