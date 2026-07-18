//! `aomi-build compile` — build every app's cdylib, copy validated plugins
//! into `plugins/`, codesign on macOS.
//!
//! Ported from the old `cargo xtask build-aomi` subcommand. Public API:
//! [`CompileArgs`] (clap) + [`run`] (eyre).

pub(crate) mod validate;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use aomi_sdk::DynFnHandle;
use clap::Args;
use eyre::{Result, bail};
use toml::Value;

use crate::specs::workspace_root;

#[derive(Args, Debug)]
pub struct CompileArgs {
    /// Build only this app (matches the app's `[package].name`).
    #[arg(long, value_name = "NAME")]
    pub app: Option<String>,

    /// Build in release mode (`cargo build --release`).
    #[arg(long)]
    pub release: bool,

    /// Cargo target triple (e.g. `aarch64-apple-darwin`).
    #[arg(long, value_name = "TRIPLE")]
    pub target: Option<String>,
}

pub fn run(args: CompileArgs) -> Result<()> {
    let repo_root = workspace_root()?;
    if !repo_root.join("apps").is_dir() {
        bail!(
            "workspace root {} does not contain an apps/ directory",
            repo_root.display()
        );
    }
    let apps_dir = repo_root.join("apps");
    let plugins_dir = repo_root.join("plugins");
    let target_dir = repo_root.join("target");
    let cargo_home = repo_root.join(".cargo-home");

    let profile = if args.release { "release" } else { "debug" };
    // `CARGO` is set when invoked via `cargo run`; fall back to the literal
    // `cargo` so the installed binary still works on PATH.
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let target_triple = args.target.as_deref();

    fs::create_dir_all(&plugins_dir)?;
    fs::create_dir_all(&cargo_home)?;

    let app_manifests = find_app_manifests(&apps_dir, &repo_root);
    let mut built = 0;
    let mut failed: Vec<String> = Vec::new();

    for manifest_path in app_manifests {
        let manifest = AppManifest::load(manifest_path);
        let pkg_name = manifest.package_name.as_str();

        if let Some(ref filter) = args.app
            && pkg_name != *filter
        {
            continue;
        }

        if manifest.skip {
            println!("  [SKIP] {pkg_name} — skip = true in Cargo.toml");
            continue;
        }

        println!("building plugin: {pkg_name}");
        let ok = build_app(
            &cargo,
            &manifest.path,
            &target_dir,
            &cargo_home,
            profile,
            target_triple,
        );
        if !ok {
            eprintln!("  [SKIP] {pkg_name} — build failed");
            failed.push(manifest.package_name);
            continue;
        }

        let built_lib =
            built_library_path(&target_dir, profile, target_triple, &manifest.library_name);
        if !built_lib.is_file() {
            eprintln!(
                "  [SKIP] {pkg_name} — expected library at {}, not found",
                built_lib.display()
            );
            failed.push(manifest.package_name);
            continue;
        }
        let manifest_name = plugin_manifest_name(&built_lib);
        let dest = plugins_dir.join(library_file_name(&manifest_name, target_triple));
        fs::copy(&built_lib, &dest).unwrap_or_else(|err| {
            panic!(
                "failed to copy {} to {}: {err}",
                built_lib.display(),
                dest.display()
            )
        });

        if should_codesign(target_triple) {
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

        let _plugin_manifest = match validate::inspect_plugin(&dest) {
            Ok(manifest) => manifest,
            Err(validation_errors) => {
                for err in &validation_errors {
                    eprintln!("  validation error: {err}");
                }
                eprintln!("  [SKIP] {pkg_name} — validation failed");
                let _ = fs::remove_file(&dest);
                failed.push(manifest.package_name);
                continue;
            }
        };

        if let Err(err) = sync_pricing_sidecar(&manifest.path, &manifest_name, &plugins_dir) {
            eprintln!("  pricing sidecar error: {err}");
            eprintln!("  [SKIP] {pkg_name} — invalid pricing.toml");
            let _ = fs::remove_file(&dest);
            failed.push(manifest.package_name);
            continue;
        }

        built += 1;
    }

    println!("built {built} plugin(s) -> {}", plugins_dir.display());
    if !failed.is_empty() {
        bail!("failed plugins: {}", failed.join(", "));
    }
    Ok(())
}

// ── App manifest parsing ─────────────────────────────────────────────────────

struct AppManifest {
    path: PathBuf,
    package_name: String,
    library_name: String,
    skip: bool,
}

impl AppManifest {
    fn load(path: PathBuf) -> Self {
        let contents = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        Self::parse(&path, &contents)
    }

    fn parse(path: &Path, contents: &str) -> Self {
        let value: Value = toml::from_str(contents)
            .unwrap_or_else(|err| panic!("invalid TOML in {}: {err}", path.display()));
        let package_name = value
            .get("package")
            .and_then(|pkg| pkg.get("name"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| panic!("missing [package].name in {}", path.display()));
        let library_name = value
            .get("lib")
            .and_then(|lib| lib.get("name"))
            .and_then(Value::as_str)
            .map(ToString::to_string)
            .unwrap_or_else(|| package_name.clone());
        let skip = value
            .get("package")
            .and_then(|pkg| pkg.get("metadata"))
            .and_then(|meta| meta.get("aomi"))
            .and_then(|aomi| aomi.get("skip"))
            .and_then(Value::as_bool)
            .unwrap_or(false);

        Self {
            path: path.to_path_buf(),
            package_name,
            library_name,
            skip,
        }
    }
}

// ── App discovery ────────────────────────────────────────────────────────────

fn find_app_manifests(apps_dir: &Path, repo_root: &Path) -> Vec<PathBuf> {
    let mut manifests = tracked_app_manifests(apps_dir, repo_root);
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

fn tracked_app_manifests(apps_dir: &Path, repo_root: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .args(["ls-files", "apps/*/Cargo.toml"])
        .current_dir(repo_root)
        .output();

    let output = match output {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    let mut manifests = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| repo_root.join(line))
        .filter(|path| path.starts_with(apps_dir))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    manifests.sort();
    manifests
}

// ── Build + layout helpers ───────────────────────────────────────────────────

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

/// Ship the app's pricing sidecar into the bundle: `apps/<dir>/pricing.toml`
/// → `plugins/<manifest-name>.pricing.toml` (the host resolves sidecars by
/// the dylib's manifest name). Missing file = free app. A stale sidecar in
/// `plugins/` is removed, so deleting the source file un-prices the app on
/// the next release. The sidecar must parse as TOML and declare `version`,
/// so a malformed file fails the build here instead of refusing the app
/// load on the host.
fn sync_pricing_sidecar(
    app_manifest_path: &Path,
    manifest_name: &str,
    plugins_dir: &Path,
) -> Result<()> {
    let src = app_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("pricing.toml");
    let dest = plugins_dir.join(format!("{manifest_name}.pricing.toml"));
    if !src.is_file() {
        if dest.exists() {
            fs::remove_file(&dest)?;
        }
        return Ok(());
    }
    let raw = fs::read_to_string(&src)?;
    let value: Value = toml::from_str(&raw)
        .map_err(|err| eyre::eyre!("malformed {}: {err}", src.display()))?;
    if value.get("version").and_then(Value::as_integer).is_none() {
        bail!("{} must declare an integer `version`", src.display());
    }
    fs::copy(&src, &dest)?;
    println!("  shipped pricing sidecar -> {}", dest.display());
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pricing_sidecar_ships_validates_and_removes_stale() {
        let app_dir = tempfile::tempdir().expect("app dir");
        let plugins = tempfile::tempdir().expect("plugins dir");
        let manifest = app_dir.path().join("Cargo.toml");
        let dest = plugins.path().join("demo.pricing.toml");

        // No source file, no dest: nothing to do.
        sync_pricing_sidecar(&manifest, "demo", plugins.path()).expect("no-op");
        assert!(!dest.exists());

        // Valid sidecar ships under the manifest name.
        fs::write(app_dir.path().join("pricing.toml"), "version = 1").unwrap();
        sync_pricing_sidecar(&manifest, "demo", plugins.path()).expect("ship");
        assert!(dest.is_file());

        // Malformed / version-less files fail the build.
        fs::write(app_dir.path().join("pricing.toml"), "version = ").unwrap();
        assert!(sync_pricing_sidecar(&manifest, "demo", plugins.path()).is_err());
        fs::write(app_dir.path().join("pricing.toml"), "flat = 1.0").unwrap();
        assert!(sync_pricing_sidecar(&manifest, "demo", plugins.path()).is_err());

        // Deleting the source removes the shipped copy.
        fs::remove_file(app_dir.path().join("pricing.toml")).unwrap();
        sync_pricing_sidecar(&manifest, "demo", plugins.path()).expect("prune");
        assert!(!dest.exists());
    }

    #[test]
    fn app_manifest_uses_explicit_lib_name_when_present() {
        let manifest = AppManifest::parse(
            Path::new("apps/khalani/Cargo.toml"),
            r#"
[package]
name = "dyn-khalani"

[lib]
name = "khalani"
crate-type = ["cdylib"]
"#,
        );

        assert_eq!(manifest.package_name, "dyn-khalani");
        assert_eq!(manifest.library_name, "khalani");
        assert!(!manifest.skip);
    }

    #[test]
    fn app_manifest_falls_back_to_package_name_for_library_name() {
        let manifest = AppManifest::parse(
            Path::new("apps/example/Cargo.toml"),
            r#"
[package]
name = "example-app"
"#,
        );

        assert_eq!(manifest.package_name, "example-app");
        assert_eq!(manifest.library_name, "example-app");
    }
}
