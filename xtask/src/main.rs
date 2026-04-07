mod validate;

use aomi_sdk::DynFnHandle;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("build-aomi") => cmd_build_plugins(&args[1..]),
        Some("new-app") => cmd_new_app(&args[1..]),
        Some(other) => {
            eprintln!("unknown subcommand: {other}");
            print_usage();
            std::process::exit(1);
        }
        None => {
            print_usage();
            std::process::exit(1);
        }
    }
}

fn print_usage() {
    eprintln!("usage:");
    eprintln!("  cargo xtask build-aomi [--app NAME] [--release] [--target TRIPLE]");
    eprintln!("  cargo xtask new-app <NAME>");
}

fn cmd_build_plugins(args: &[String]) {
    let repo_root = repo_root();
    let apps_dir = repo_root.join("apps");
    let plugins_dir = repo_root.join("plugins");
    let target_dir = repo_root.join("target");
    let cargo_home = repo_root.join(".cargo-home");

    let mut app_filter: Option<String> = None;
    let mut release = false;
    let mut target_triple: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--app" => {
                i += 1;
                app_filter = Some(args.get(i).expect("--app requires a value").clone());
            }
            "--release" => {
                release = true;
            }
            "--target" => {
                i += 1;
                target_triple = Some(args.get(i).expect("--target requires a value").clone());
            }
            other => {
                eprintln!("unknown flag: {other}");
                std::process::exit(1);
            }
        }
        i += 1;
    }

    let profile = if release { "release" } else { "debug" };
    let cargo = env::var("CARGO").expect("CARGO must be set by cargo");

    fs::create_dir_all(&plugins_dir).expect("failed to create plugins");
    fs::create_dir_all(&cargo_home).expect("failed to create .cargo-home");

    let app_manifests = find_app_manifests(&apps_dir);
    let mut built = 0;
    let mut failed: Vec<String> = Vec::new();

    for manifest_path in app_manifests {
        let pkg_name = package_name(&manifest_path);

        if let Some(ref filter) = app_filter
            && pkg_name != *filter
        {
            continue;
        }

        if should_skip(&manifest_path) {
            println!("  [SKIP] {pkg_name} — skip = true in Cargo.toml");
            continue;
        }

        println!("building plugin: {pkg_name}");
        let ok = build_app(
            &cargo,
            &manifest_path,
            &target_dir,
            &cargo_home,
            profile,
            target_triple.as_deref(),
        );
        if !ok {
            eprintln!("  [SKIP] {pkg_name} — build failed");
            failed.push(pkg_name);
            continue;
        }

        let built_lib =
            built_library_path(&target_dir, profile, target_triple.as_deref(), &pkg_name);
        if !built_lib.is_file() {
            eprintln!(
                "  [SKIP] {pkg_name} — expected library at {}, not found",
                built_lib.display()
            );
            failed.push(pkg_name);
            continue;
        }
        let manifest_name = plugin_manifest_name(&built_lib);
        let dest = plugins_dir.join(library_file_name(&manifest_name, target_triple.as_deref()));
        fs::copy(&built_lib, &dest).unwrap_or_else(|err| {
            panic!(
                "failed to copy {} to {}: {err}",
                built_lib.display(),
                dest.display()
            )
        });

        if should_codesign(target_triple.as_deref()) {
            let status = Command::new("codesign")
                .args(["-s", "-", "-f"])
                .arg(&dest)
                .status()
                .unwrap_or_else(|err| {
                    panic!("failed to run codesign on {}: {err}", dest.display())
                });
            if !status.success() {
                panic!(
                    "codesign failed for {} with status {status}",
                    dest.display()
                );
            }
        }

        let validation_errors = validate::validate_plugin(&dest);
        if !validation_errors.is_empty() {
            for err in &validation_errors {
                eprintln!("  validation error: {err}");
            }
            eprintln!("  [SKIP] {pkg_name} — validation failed");
            let _ = fs::remove_file(&dest);
            failed.push(pkg_name);
            continue;
        }

        built += 1;
    }

    println!("built {built} plugin(s) -> {}", plugins_dir.display());
    if !failed.is_empty() {
        eprintln!("failed plugins: {}", failed.join(", "));
        std::process::exit(1);
    }
}

fn repo_root() -> PathBuf {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set"));
    let repo_root = manifest_dir
        .parent()
        .expect("xtask must live at <repo>/xtask")
        .to_path_buf()
        .canonicalize()
        .unwrap_or_else(|err| panic!("failed to resolve repo root: {err}"));

    if !repo_root.join("apps").is_dir() || !repo_root.join("sdk").is_dir() {
        panic!(
            "repo root {} must contain both apps/ and sdk/",
            repo_root.display()
        );
    }
    repo_root
}

fn find_app_manifests(apps_dir: &Path) -> Vec<PathBuf> {
    let mut manifests = tracked_app_manifests(apps_dir);
    let entries = fs::read_dir(apps_dir).unwrap_or_else(|err| {
        panic!(
            "failed to read apps directory {}: {err}",
            apps_dir.display()
        )
    });
    for entry in entries {
        let entry = entry.expect("failed to read apps entry");
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name == "src" || name == "target" || name == ".cargo-home" || name.starts_with('.') {
            continue;
        }
        let manifest = path.join("Cargo.toml");
        if manifest.exists() {
            manifests.push(manifest);
        }
    }
    manifests.sort();
    manifests.dedup();
    manifests
}

fn tracked_app_manifests(apps_dir: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .args(["ls-files", "apps/*/Cargo.toml"])
        .current_dir(repo_root())
        .output();

    let output = match output {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    let mut manifests = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| repo_root().join(line))
        .filter(|path| path.starts_with(apps_dir))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    manifests.sort();
    manifests
}

fn package_name(manifest_path: &Path) -> String {
    let contents = fs::read_to_string(manifest_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", manifest_path.display()));
    let value: toml::Value = toml::from_str(&contents)
        .unwrap_or_else(|err| panic!("invalid TOML in {}: {err}", manifest_path.display()));

    value
        .get("package")
        .and_then(|pkg| pkg.get("name"))
        .and_then(toml::Value::as_str)
        .map(ToString::to_string)
        .unwrap_or_else(|| panic!("missing [package].name in {}", manifest_path.display()))
}

fn should_skip(manifest_path: &Path) -> bool {
    let contents = fs::read_to_string(manifest_path).unwrap_or_default();
    let value: toml::Value = match toml::from_str(&contents) {
        Ok(v) => v,
        Err(_) => return false,
    };
    value
        .get("package")
        .and_then(|pkg| pkg.get("metadata"))
        .and_then(|meta| meta.get("aomi"))
        .and_then(|aomi| aomi.get("skip"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false)
}

fn build_app(
    cargo: &str,
    manifest_path: &Path,
    target_dir: &Path,
    cargo_home: &Path,
    profile: &str,
    target_triple: Option<&str>,
) -> bool {
    let mut command = Command::new(cargo);
    command.arg("build");
    command.arg("--lib");
    command.arg("--manifest-path").arg(manifest_path);
    command.arg("--target-dir").arg(target_dir);
    command.env("CARGO_HOME", cargo_home);
    if profile == "release" {
        command.arg("--release");
    }
    if let Some(target) = target_triple {
        command.arg("--target").arg(target);
    }
    let status = match command.status() {
        Ok(s) => s,
        Err(err) => {
            eprintln!(
                "  failed to run cargo for {}: {err}",
                manifest_path.display()
            );
            return false;
        }
    };
    if !status.success() {
        eprintln!("  cargo build failed for {}", manifest_path.display());
        return false;
    }
    true
}

fn built_library_path(
    target_dir: &Path,
    profile: &str,
    target_triple: Option<&str>,
    package_name: &str,
) -> PathBuf {
    let mut path = target_dir.to_path_buf();
    if let Some(target) = target_triple {
        path.push(target);
    }
    path.push(profile);
    let file_name = cargo_output_file_name(package_name, target_triple);
    path.push(file_name);
    path
}

fn library_file_name(package_name: &str, target_triple: Option<&str>) -> String {
    let base_name = package_name.replace('-', "_");
    let ext = shared_library_ext(target_triple);
    format!("{base_name}.{ext}")
}

fn cargo_output_file_name(package_name: &str, target_triple: Option<&str>) -> String {
    let base_name = package_name.replace('-', "_");
    let ext = shared_library_ext(target_triple);
    match ext {
        "dll" => format!("{base_name}.dll"),
        _ => format!("lib{base_name}.{ext}"),
    }
}

fn plugin_manifest_name(lib_path: &Path) -> String {
    let handle = unsafe {
        DynFnHandle::load(lib_path).unwrap_or_else(|err| {
            panic!("failed to load built plugin {}: {err}", lib_path.display())
        })
    };
    let manifest = handle
        .call_manifest()
        .unwrap_or_else(|err| panic!("failed to read manifest from {}: {err}", lib_path.display()));
    manifest.name
}

fn shared_library_ext(target_triple: Option<&str>) -> &'static str {
    match target_triple {
        Some(target) => {
            if target.contains("windows") {
                "dll"
            } else if target.contains("apple") || target.contains("darwin") {
                "dylib"
            } else if target.contains("linux") {
                "so"
            } else {
                panic!("unsupported target triple: {target}");
            }
        }
        None => {
            if cfg!(target_os = "windows") {
                "dll"
            } else if cfg!(target_os = "macos") {
                "dylib"
            } else if cfg!(target_os = "linux") {
                "so"
            } else {
                panic!("unsupported host OS for shared library naming");
            }
        }
    }
}

fn should_codesign(target_triple: Option<&str>) -> bool {
    cfg!(target_os = "macos") && shared_library_ext(target_triple) == "dylib"
}

// ── new-app scaffolding ─────────────────────────────────────────────────────

fn cmd_new_app(args: &[String]) {
    let name = args.first().unwrap_or_else(|| {
        eprintln!("usage: cargo xtask new-app <NAME>");
        std::process::exit(1);
    });

    // Validate: lowercase alphanumeric + hyphens
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        || name.is_empty()
    {
        eprintln!("app name must be lowercase alphanumeric with hyphens (e.g. 'my-app')");
        std::process::exit(1);
    }

    let repo_root = repo_root();
    let app_dir = repo_root.join("apps").join(name);

    if app_dir.exists() {
        eprintln!("apps/{name} already exists");
        std::process::exit(1);
    }

    let src_dir = app_dir.join("src");
    fs::create_dir_all(&src_dir)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", src_dir.display()));

    // Derive a Rust-friendly struct name from the app name
    let struct_name: String = name
        .split('-')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<String>()
        + "App";

    // Cargo.toml
    fs::write(
        app_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
aomi-sdk = {{ path = "../../sdk" }}
reqwest = {{ version = "0.12", default-features = false, features = ["json", "rustls-tls", "blocking"] }}
schemars = "1.0.4"
serde = {{ version = "1.0", features = ["derive"] }}
serde_json = "1.0"
"#
        ),
    )
    .expect("failed to write Cargo.toml");

    // src/lib.rs — built via push_str to avoid nested raw-string delimiter issues.
    let mut lib_rs = String::new();
    lib_rs.push_str("mod client;\nmod tool;\n\n");
    lib_rs.push_str("use aomi_sdk::dyn_aomi_app;\n\n");
    lib_rs.push_str("const PREAMBLE: &str = r#\"\n");
    lib_rs.push_str(&format!("## Role\nYou are the {name} app.\n\n"));
    lib_rs.push_str("## Purpose\nTODO: describe what this app does.\n");
    lib_rs.push_str("\"#;\n\n");
    lib_rs.push_str(&format!(
        "dyn_aomi_app!(\n\
         \x20   app = client::{struct_name},\n\
         \x20   name = \"{name}\",\n\
         \x20   version = \"0.1.0\",\n\
         \x20   preamble = PREAMBLE,\n\
         \x20   tools = [\n\
         \x20       tool::ExampleTool,\n\
         \x20   ],\n\
         \x20   namespaces = [\"common\"]\n\
         );\n"
    ));
    fs::write(src_dir.join("lib.rs"), lib_rs).expect("failed to write lib.rs");

    // src/client.rs
    fs::write(
        src_dir.join("client.rs"),
        format!(
            r#"#[derive(Clone, Default)]
pub(crate) struct {struct_name};
"#
        ),
    )
    .expect("failed to write client.rs");

    // src/tool.rs
    fs::write(
        src_dir.join("tool.rs"),
        format!(
            r#"use aomi_sdk::{{DynAomiTool, DynToolCallCtx}};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::client::{struct_name};

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ExampleToolArgs {{
    /// A sample input parameter.
    pub query: String,
}}

pub(crate) struct ExampleTool;

impl DynAomiTool for ExampleTool {{
    type App = {struct_name};
    type Args = ExampleToolArgs;

    const NAME: &'static str = "{name}_example";
    const DESCRIPTION: &'static str = "TODO: describe this tool.";

    fn run(
        _app: &{struct_name},
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {{
        Ok(serde_json::json!({{
            "echo": args.query,
        }}))
    }}
}}
"#,
            name = name.replace('-', "_")
        ),
    )
    .expect("failed to write tool.rs");

    // Register in workspace exclude list so Cargo doesn't error
    let workspace_toml_path = repo_root.join("Cargo.toml");
    let workspace_toml =
        fs::read_to_string(&workspace_toml_path).expect("failed to read workspace Cargo.toml");

    let exclude_entry = format!("\"apps/{name}\"");
    if !workspace_toml.contains(&exclude_entry) {
        // Insert before the closing bracket of the exclude array
        let new_toml = workspace_toml.replacen(
            "]\nresolver",
            &format!("    \"{}\",\n]\nresolver", format!("apps/{name}")),
            1,
        );
        fs::write(&workspace_toml_path, new_toml).expect("failed to update workspace Cargo.toml");
    }

    println!("created apps/{name}/");
    println!("  src/lib.rs    — app manifest + preamble");
    println!("  src/client.rs — app struct + HTTP client");
    println!("  src/tool.rs   — tool implementations");
    println!();
    println!("next steps:");
    println!("  1. edit the preamble in src/lib.rs");
    println!("  2. add your HTTP client in src/client.rs");
    println!("  3. implement your tools in src/tool.rs");
    println!("  4. cargo xtask build-aomi --app {name}");
}
