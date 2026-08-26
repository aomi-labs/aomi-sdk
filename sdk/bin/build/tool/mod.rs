use clap::Args;
use eyre::{Context, Result, bail};
use inquire::MultiSelect;
use std::path::PathBuf;

use crate::spec_load::{self, pascal_case};
use crate::specs::workspace_root;

mod render;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Generated client lives in apps/<platform>/src/client/.
    /// Tool layer imports from `crate::client`.
    AppLocal,
    /// Generated client lives in ext/src/<platform>/.
    /// Tool layer imports from `aomi_ext::<platform>`.
    Shared,
}

#[derive(Args, Debug)]
pub struct GenToolArgs {
    /// Platform name.
    pub platform: String,

    /// Override the spec path. Defaults to apps/<platform>/openapi.yaml
    /// (or ext/specs/<platform>.yaml when --shared is set).
    #[arg(long)]
    pub spec: Option<PathBuf>,

    /// Override the output directory. Defaults to apps/<platform>/.
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Skip the interactive picker and include every operation.
    #[arg(long)]
    pub all: bool,

    /// Overwrite existing files (Cargo.toml, src/lib.rs, src/tool.rs).
    #[arg(long)]
    pub force: bool,

    /// Treat as a shared library (client lives under ext/, tool layer imports
    /// from `aomi_ext::<platform>`). Default is app-local (client at
    /// `apps/<platform>/src/client/`, tool layer imports from `crate::client`).
    /// Auto-detected when omitted: presence of `apps/<platform>/src/client/`
    /// implies app-local; otherwise --shared is assumed.
    #[arg(long)]
    pub shared: bool,
}

pub fn run(args: GenToolArgs) -> Result<()> {
    let root = workspace_root()?;
    // Detect mode: explicit --shared OR auto-detect from filesystem.
    let app_client_dir = root
        .join("apps")
        .join(&args.platform)
        .join("src")
        .join("client");
    let app_local = !args.shared && app_client_dir.exists();
    let mode = if app_local {
        Mode::AppLocal
    } else {
        Mode::Shared
    };
    println!(
        "Mode: {}",
        match mode {
            Mode::AppLocal => "app-local (client in apps/<platform>/src/client/)",
            Mode::Shared => "shared (client in ext/src/<platform>/)",
        }
    );

    let spec_path = args.spec.clone().unwrap_or_else(|| match mode {
        Mode::AppLocal => root.join("apps").join(&args.platform).join("openapi.yaml"),
        Mode::Shared => root
            .join("ext")
            .join("specs")
            .join(format!("{}.yaml", args.platform)),
    });
    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| root.join("apps").join(&args.platform));

    println!("Reading spec: {}", spec_path.display());
    let spec = spec_load::load_and_preprocess(&spec_path)?;

    let ops = collect_ops(&spec);
    if ops.is_empty() {
        bail!("spec has no operations");
    }

    let chosen: Vec<Op> = if args.all {
        ops
    } else {
        let labels: Vec<String> = ops
            .iter()
            .map(|o| {
                format!(
                    "{} {} → {}_{}",
                    o.method.to_uppercase(),
                    o.path,
                    args.platform,
                    o.operation_id
                )
            })
            .collect();
        let picked = MultiSelect::new("Select operations to expose as Aomi tools:", labels.clone())
            .with_page_size(20)
            .prompt()
            .context("operation picker failed or was cancelled")?;
        picked
            .iter()
            .map(|p| {
                let idx = labels
                    .iter()
                    .position(|l| l == p)
                    .expect("picked label exists");
                ops[idx].clone()
            })
            .collect()
    };

    if chosen.is_empty() {
        bail!("no operations selected");
    }

    let (chosen, skipped): (Vec<_>, Vec<_>) = chosen.into_iter().partition(|o| {
        !o.non_json_response
            && !o.has_request_body
            && !o
                .params
                .iter()
                .any(|p| matches!(p.kind, ParamKind::EnumString | ParamKind::Other))
    });
    for op in &skipped {
        let reason = if op.non_json_response {
            "non-JSON response"
        } else if op.has_request_body {
            "has typed request body (synthesize manually)"
        } else if op.params.iter().any(|p| p.kind == ParamKind::EnumString) {
            "has enum-typed params (progenitor generates Rust enums we can't synth from String)"
        } else {
            "has array/object-typed params (synthesize manually)"
        };
        println!(
            "  skipped {} {} ({})",
            op.method.to_uppercase(),
            op.path,
            reason
        );
    }
    if chosen.is_empty() {
        bail!("all selected operations were skipped");
    }

    let src_dir = out_dir.join("src");
    std::fs::create_dir_all(&src_dir)
        .with_context(|| format!("failed to create {}", src_dir.display()))?;

    let cargo_toml_path = out_dir.join("Cargo.toml");
    let lib_rs_path = src_dir.join("lib.rs");
    let tool_rs_path = src_dir.join("tool.rs");

    for p in [&cargo_toml_path, &lib_rs_path, &tool_rs_path] {
        if p.exists() && !args.force {
            bail!("{} already exists. Pass --force to overwrite.", p.display());
        }
    }

    let app_struct = format!("{}App", pascal_case(&args.platform));
    let preamble = render::preamble_default(&args.platform);

    std::fs::write(&cargo_toml_path, render::cargo_toml(&args.platform, mode))?;
    std::fs::write(
        &lib_rs_path,
        render::lib_rs(&args.platform, &app_struct, &chosen, &preamble, mode),
    )?;
    std::fs::write(
        &tool_rs_path,
        render::tool_rs(&args.platform, &app_struct, &chosen, mode),
    )?;

    println!("✓ wrote {}", cargo_toml_path.display());
    println!("✓ wrote {}", lib_rs_path.display());
    println!(
        "✓ wrote {} ({} tools)",
        tool_rs_path.display(),
        chosen.len()
    );

    if ensure_workspace_excludes(&root, &args.platform)? {
        println!("✓ added `apps/{}` to workspace exclude list", args.platform);
    }

    println!();
    println!("Verify with:");
    println!("  cd apps/{} && cargo build", args.platform);
    Ok(())
}

/// Ensure the workspace root Cargo.toml lists `apps/<platform>` under
/// `[workspace] exclude = [...]`. The existing convention in this repo is to
/// exclude every app crate (each is a cdylib that compiles independently).
/// Returns true iff the file was modified.
fn ensure_workspace_excludes(root: &std::path::Path, platform: &str) -> Result<bool> {
    let cargo_path = root.join("Cargo.toml");
    let cargo = std::fs::read_to_string(&cargo_path)?;
    let needle = format!(r#""apps/{platform}""#);
    if cargo.contains(&needle) {
        return Ok(false);
    }
    // Insert before the closing `]` of the exclude block. Best-effort string
    // manipulation: find `exclude = [` and the next `]`.
    if let Some(start) = cargo.find("exclude = [")
        && let Some(end_offset) = cargo[start..].find(']')
    {
        let end = start + end_offset;
        let prefix = &cargo[..end];
        let suffix = &cargo[end..];
        let new_cargo = format!("{prefix}    {needle},\n{suffix}");
        std::fs::write(&cargo_path, new_cargo)?;
        return Ok(true);
    }
    // No exclude block — silently skip rather than fail the gen.
    Ok(false)
}

mod ops;

pub(crate) use ops::*;
