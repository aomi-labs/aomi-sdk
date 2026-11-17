use clap::Args;
use eyre::{Context, Result, bail};
use inquire::MultiSelect;
use std::path::PathBuf;

use crate::client::{Mode, detect_generated_deps, ensure_deps};
use crate::spec_load;
use crate::specs::workspace_root;

mod render;

#[derive(Args, Debug)]
pub struct GenToolArgs {
    /// Platform name.
    pub platform: String,

    /// Override the spec path. Defaults to packages/<platform>/openapi.yaml
    /// (or crates/clients/<platform>/openapi.yaml when --shared is set).
    #[arg(long)]
    pub spec: Option<PathBuf>,

    /// Override the output directory. Defaults to packages/<platform>/.
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Skip the interactive picker and include every operation.
    #[arg(long)]
    pub all: bool,

    /// Overwrite existing files (Cargo.toml, aomi.toml, src/lib.rs, src/tool.rs).
    #[arg(long)]
    pub force: bool,

    /// The client is the shared crate `<platform>-client` under crates/clients/.
    /// Auto-detected when omitted: a client module at
    /// `packages/<platform>/src/client/` implies package-local.
    #[arg(long)]
    pub shared: bool,
}

pub fn run(args: GenToolArgs) -> Result<()> {
    let root = workspace_root()?;
    // Detect mode: explicit --shared OR auto-detect from filesystem.
    let package_client_dir = root
        .join("packages")
        .join(&args.platform)
        .join("src")
        .join("client");
    let package_local = !args.shared && package_client_dir.exists();
    let mode = if package_local {
        Mode::PackageLocal
    } else {
        Mode::Shared
    };
    println!(
        "Mode: {}",
        match mode {
            Mode::PackageLocal => "package-local (client in packages/<platform>/src/client/)",
            Mode::Shared => "shared (client crate crates/clients/<platform>)",
        }
    );

    let spec_path = args
        .spec
        .clone()
        .unwrap_or_else(|| mode.spec_path(&root, &args.platform));
    let out_dir = args
        .out
        .clone()
        .unwrap_or_else(|| root.join("packages").join(&args.platform));

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
            && !o.params.iter().any(|p| p.is_auth && p.required)
            && !o
                .params
                .iter()
                .any(|p| matches!(p.kind, ParamKind::EnumString | ParamKind::Other))
    });
    for op in &skipped {
        let reason = if op.non_json_response {
            "non-JSON response"
        } else if op.params.iter().any(|p| p.is_auth && p.required) {
            "required credential parameter (the host injects keys; hand-write this tool)"
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
    let manifest_path = out_dir.join("aomi.toml");
    let lib_rs_path = src_dir.join("lib.rs");
    let tool_rs_path = src_dir.join("tool.rs");

    for p in [
        &cargo_toml_path,
        &manifest_path,
        &lib_rs_path,
        &tool_rs_path,
    ] {
        if p.exists() && !args.force {
            bail!("{} already exists. Pass --force to overwrite.", p.display());
        }
    }

    let cargo = render::cargo_toml(&args.platform, mode);
    let cargo = match mode {
        Mode::Shared => cargo,
        Mode::PackageLocal => {
            let client = std::fs::read_to_string(src_dir.join("client").join("client.rs"))
                .unwrap_or_default();
            ensure_deps(&cargo, &detect_generated_deps(&client))?
        }
    };

    std::fs::write(&cargo_toml_path, cargo)?;
    std::fs::write(&manifest_path, render::aomi_toml(&args.platform, &chosen))?;
    std::fs::write(&lib_rs_path, render::lib_rs(&args.platform, &chosen, mode))?;
    std::fs::write(
        &tool_rs_path,
        render::tool_rs(&args.platform, &chosen, mode),
    )?;

    println!("✓ wrote {}", cargo_toml_path.display());
    println!("✓ wrote {}", manifest_path.display());
    println!("✓ wrote {}", lib_rs_path.display());
    println!(
        "✓ wrote {} ({} tools)",
        tool_rs_path.display(),
        chosen.len()
    );
    println!();
    println!("Verify with:");
    println!(
        "  cargo build -p {} --target wasm32-wasip2 --release",
        args.platform
    );
    Ok(())
}

#[cfg(test)]
mod tests;

mod ops;

pub(crate) use ops::*;
