//! SDK compatibility checks for deployable app repositories.
//!
//! Hosted backends load dynamic apps only when the app was built with the exact
//! `aomi-sdk` version compiled into the backend. These helpers make that
//! contract visible before deploy/activate, and can repair simple Cargo pins.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Args, Subcommand};
use serde::Serialize;
use toml_edit::{DocumentMut, value};

#[derive(Debug, Args, Clone)]
pub struct SdkArgs {
    #[command(subcommand)]
    pub cmd: SdkCmd,
}

#[derive(Debug, Subcommand, Clone)]
pub enum SdkCmd {
    /// Check whether Cargo.toml/Cargo.lock match the backend-required SDK.
    Check(SdkCheckArgs),
    /// Rewrite Cargo.toml to the backend-required exact SDK pin and update Cargo.lock.
    Fix(SdkFixArgs),
}

#[derive(Debug, Args, Clone)]
pub struct SdkCheckArgs {
    /// App repo or Cargo.toml to check.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Backend base URL. Defaults to AOMI_BACKEND_URL or saved aomi-build config.
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Override the required SDK version; mainly for CI/tests.
    #[arg(long, value_name = "VERSION")]
    pub required_version: Option<String>,

    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args, Clone)]
pub struct SdkFixArgs {
    /// App repo or Cargo.toml to fix.
    #[arg(long, default_value = ".")]
    pub path: PathBuf,

    /// Backend base URL. Defaults to AOMI_BACKEND_URL or saved aomi-build config.
    #[arg(long, value_name = "URL")]
    pub backend: Option<String>,

    /// Override the required SDK version; mainly for CI/tests.
    #[arg(long, value_name = "VERSION")]
    pub required_version: Option<String>,

    /// Print machine-readable JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub struct SdkCheckReport {
    pub manifest_path: PathBuf,
    pub lockfile_path: Option<PathBuf>,
    pub required_version: String,
    pub dependency: DependencyStatus,
    pub lockfile: LockfileStatus,
    pub ok: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DependencyStatus {
    Exact { version: String },
    Loose { requirement: String },
    Stale { version: String },
    PathDependency { path: String },
    Missing,
    Unsupported { detail: String },
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LockfileStatus {
    Matches { version: String },
    Stale { version: String },
    MissingPackage,
    MissingLockfile,
    Unreadable { detail: String },
}

impl SdkCheckReport {
    pub fn blocking_messages(&self) -> Vec<String> {
        let mut messages = Vec::new();
        match &self.dependency {
            DependencyStatus::Exact { .. } => {}
            DependencyStatus::Loose { requirement } => messages.push(format!(
                "Cargo.toml uses a loose aomi-sdk requirement `{requirement}`; pin it exactly to `={}`.",
                self.required_version
            )),
            DependencyStatus::Stale { version } => messages.push(format!(
                "Cargo.toml pins aomi-sdk {version}, but this backend requires {}.",
                self.required_version
            )),
            DependencyStatus::PathDependency { path } => messages.push(format!(
                "Cargo.toml uses a path aomi-sdk dependency `{path}`; deployable apps must pin `aomi-sdk = \"={}\"`.",
                self.required_version
            )),
            DependencyStatus::Missing => messages.push(
                "Cargo.toml does not declare an aomi-sdk dependency.".to_string(),
            ),
            DependencyStatus::Unsupported { detail } => messages.push(format!(
                "Cargo.toml has an unsupported aomi-sdk dependency shape: {detail}."
            )),
        }
        match &self.lockfile {
            LockfileStatus::Matches { .. } => {}
            LockfileStatus::Stale { version } => messages.push(format!(
                "Cargo.lock resolves aomi-sdk {version}, but this backend requires {}.",
                self.required_version
            )),
            LockfileStatus::MissingPackage => {
                messages.push("Cargo.lock does not contain aomi-sdk.".to_string())
            }
            LockfileStatus::MissingLockfile | LockfileStatus::Unreadable { .. } => {}
        }
        messages
    }

    fn print_text(&self) {
        println!("Required aomi-sdk: {}", self.required_version);
        println!("Manifest: {}", self.manifest_path.display());
        println!("Dependency: {}", describe_dependency(&self.dependency));
        println!("Lockfile: {}", describe_lockfile(&self.lockfile));
        if self.ok {
            println!("SDK check passed.");
        } else {
            println!("SDK check failed.");
            for message in self.blocking_messages() {
                println!("  - {message}");
            }
            println!();
            println!(
                "Fix: aomi-build sdk fix --path {}",
                self.manifest_path.display()
            );
        }
    }
}

pub async fn run(args: SdkArgs) -> eyre::Result<()> {
    match args.cmd {
        SdkCmd::Check(args) => run_check(args).await,
        SdkCmd::Fix(args) => run_fix(args).await,
    }
    .map_err(crate::git_error)
}

pub async fn run_check(args: SdkCheckArgs) -> Result<()> {
    let required = resolve_required_sdk_version(args.backend.as_deref(), args.required_version)
        .await
        .context("failed to resolve required aomi-sdk version")?;
    let report = check_project_sdk(&args.path, &required)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        report.print_text();
    }
    if report.ok {
        Ok(())
    } else {
        bail!("SDK check failed")
    }
}

pub async fn run_fix(args: SdkFixArgs) -> Result<()> {
    let required = resolve_required_sdk_version(args.backend.as_deref(), args.required_version)
        .await
        .context("failed to resolve required aomi-sdk version")?;
    let manifest_path = manifest_path(&args.path)?;
    rewrite_manifest_pin(&manifest_path, &required)?;
    run_cargo_update(&manifest_path, &required)?;
    let report = check_project_sdk(&manifest_path, &required)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        report.print_text();
    }
    if report.ok {
        Ok(())
    } else {
        bail!("SDK fix did not produce a compatible manifest")
    }
}

pub async fn ensure_project_sdk(
    project_path: &Path,
    backend_url: Option<&str>,
    fix: bool,
) -> Result<()> {
    let required = resolve_required_sdk_version(backend_url, None)
        .await
        .context("failed to resolve backend-required aomi-sdk version")?;
    let report = check_project_sdk(project_path, &required)?;
    if report.ok {
        return Ok(());
    }
    if fix {
        rewrite_manifest_pin(&report.manifest_path, &required)?;
        run_cargo_update(&report.manifest_path, &required)?;
        let rerun = check_project_sdk(&report.manifest_path, &required)?;
        if rerun.ok {
            return Ok(());
        }
        bail!(
            "SDK auto-fix did not resolve all issues:\n{}",
            rerun.blocking_messages().join("\n")
        );
    }
    bail!(
        "SDK check failed before deploy/activate:\n{}\n\nRun:\n  aomi-build sdk fix --path {} --backend <backend-url>",
        report.blocking_messages().join("\n"),
        report.manifest_path.display()
    )
}

pub async fn resolve_required_sdk_version(
    backend_url: Option<&str>,
    explicit: Option<String>,
) -> Result<String> {
    if let Some(version) = explicit
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return Ok(version);
    }
    if let Some(url) = backend_url.map(str::trim).filter(|v| !v.is_empty()) {
        return fetch_backend_sdk_version(url)
            .await
            .with_context(|| format!("failed to fetch backend SDK version from {url}"));
    }
    Ok(aomi_sdk::AOMI_SDK_VERSION.to_string())
}

async fn fetch_backend_sdk_version(base_url: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct ServerTags {
        sdk_version: String,
    }

    let base = base_url.trim().trim_end_matches('/');
    let endpoint = format!("{base}/api/platforms/server-tags");
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .connect_timeout(Duration::from_secs(4))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let response = http
        .get(&endpoint)
        .send()
        .await
        .with_context(|| format!("failed to call {endpoint}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read server-tags response")?;
    if !status.is_success() {
        bail!("server-tags endpoint returned {status}: {}", text.trim());
    }
    let parsed: ServerTags =
        serde_json::from_str(&text).context("server-tags response was not valid JSON")?;
    let version = parsed.sdk_version.trim().to_string();
    if version.is_empty() {
        bail!("server-tags response did not include sdk_version");
    }
    Ok(version)
}

pub fn check_project_sdk(path: &Path, required: &str) -> Result<SdkCheckReport> {
    let manifest_path = manifest_path(path)?;
    let text = fs::read_to_string(&manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    let dependency = dependency_status(&doc, required);
    let lockfile_path = find_lockfile(&manifest_path);
    let lockfile = lockfile_status(lockfile_path.as_deref(), required);
    let dependency_ok =
        matches!(dependency, DependencyStatus::Exact { ref version } if version == required);
    let lockfile_ok = match &lockfile {
        LockfileStatus::Matches { version } => version == required,
        LockfileStatus::MissingLockfile => true,
        _ => false,
    };
    let ok = dependency_ok && lockfile_ok;
    Ok(SdkCheckReport {
        manifest_path,
        lockfile_path,
        required_version: required.to_string(),
        dependency,
        lockfile,
        ok,
    })
}

fn manifest_path(path: &Path) -> Result<PathBuf> {
    let path = if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    };
    let candidate = if path.is_file() {
        path.to_path_buf()
    } else {
        path.join("Cargo.toml")
    };
    if candidate.exists() {
        Ok(candidate.canonicalize().unwrap_or(candidate))
    } else {
        bail!("no Cargo.toml found at {}", candidate.display())
    }
}

fn dependency_status(doc: &DocumentMut, required: &str) -> DependencyStatus {
    let Some(item) = find_dependency_item(doc) else {
        return DependencyStatus::Missing;
    };
    if let Some(raw) = item.as_str() {
        return requirement_status(raw, required);
    }
    if let Some(table) = item.as_inline_table() {
        if let Some(version) = table.get("version").and_then(|v| v.as_str()) {
            return requirement_status(version, required);
        }
        if let Some(path) = table.get("path").and_then(|v| v.as_str()) {
            return DependencyStatus::PathDependency {
                path: path.to_string(),
            };
        }
        return DependencyStatus::Unsupported {
            detail: "inline table without version or path".to_string(),
        };
    }
    if let Some(table) = item.as_table_like() {
        if let Some(version) = table.get("version").and_then(|v| v.as_str()) {
            return requirement_status(version, required);
        }
        if let Some(path) = table.get("path").and_then(|v| v.as_str()) {
            return DependencyStatus::PathDependency {
                path: path.to_string(),
            };
        }
        return DependencyStatus::Unsupported {
            detail: "table without version or path".to_string(),
        };
    }
    DependencyStatus::Unsupported {
        detail: item.type_name().to_string(),
    }
}

fn requirement_status(raw: &str, required: &str) -> DependencyStatus {
    let trimmed = raw.trim();
    if let Some(exact) = trimmed.strip_prefix('=') {
        let version = exact.trim().to_string();
        if version == required {
            DependencyStatus::Exact { version }
        } else {
            DependencyStatus::Stale { version }
        }
    } else if trimmed == required {
        DependencyStatus::Loose {
            requirement: trimmed.to_string(),
        }
    } else {
        DependencyStatus::Stale {
            version: trimmed.to_string(),
        }
    }
}

fn find_dependency_item(doc: &DocumentMut) -> Option<&toml_edit::Item> {
    doc.get("dependencies")
        .and_then(|deps| deps.get("aomi-sdk"))
        .or_else(|| {
            doc.get("workspace")
                .and_then(|workspace| workspace.get("dependencies"))
                .and_then(|deps| deps.get("aomi-sdk"))
        })
}

fn rewrite_manifest_pin(manifest_path: &Path, required: &str) -> Result<()> {
    let text = fs::read_to_string(manifest_path)
        .with_context(|| format!("failed to read {}", manifest_path.display()))?;
    let mut doc = text
        .parse::<DocumentMut>()
        .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
    if doc
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(|deps| deps.get("aomi-sdk"))
        .is_some()
    {
        doc["workspace"]["dependencies"]["aomi-sdk"] = value(format!("={required}"));
    } else {
        if !doc.as_table().contains_key("dependencies") {
            doc["dependencies"] = toml_edit::Item::Table(toml_edit::Table::new());
        }
        doc["dependencies"]["aomi-sdk"] = value(format!("={required}"));
    }
    fs::write(manifest_path, doc.to_string())
        .with_context(|| format!("failed to write {}", manifest_path.display()))
}

fn run_cargo_update(manifest_path: &Path, required: &str) -> Result<()> {
    let dir = manifest_path
        .parent()
        .ok_or_else(|| anyhow!("manifest path has no parent: {}", manifest_path.display()))?;
    if !dir.join("Cargo.lock").exists() {
        return Ok(());
    }
    let status = Command::new("cargo")
        .arg("update")
        .arg("-p")
        .arg("aomi-sdk")
        .arg("--precise")
        .arg(required)
        .current_dir(dir)
        .status()
        .with_context(|| format!("failed to run cargo update in {}", dir.display()))?;
    if status.success() {
        Ok(())
    } else {
        bail!("cargo update -p aomi-sdk --precise {required} failed")
    }
}

fn find_lockfile(manifest_path: &Path) -> Option<PathBuf> {
    let dir = manifest_path.parent()?;
    let direct = dir.join("Cargo.lock");
    if direct.exists() {
        return Some(direct);
    }
    None
}

fn lockfile_status(lockfile_path: Option<&Path>, required: &str) -> LockfileStatus {
    let Some(path) = lockfile_path else {
        return LockfileStatus::MissingLockfile;
    };
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            return LockfileStatus::Unreadable {
                detail: err.to_string(),
            };
        }
    };
    let parsed: toml::Value = match text.parse() {
        Ok(value) => value,
        Err(err) => {
            return LockfileStatus::Unreadable {
                detail: err.to_string(),
            };
        }
    };
    let packages = parsed
        .get("package")
        .and_then(|package| package.as_array())
        .into_iter()
        .flatten();
    for package in packages {
        let name = package.get("name").and_then(|v| v.as_str());
        if name == Some("aomi-sdk") {
            let version = package
                .get("version")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            return if version == required {
                LockfileStatus::Matches { version }
            } else {
                LockfileStatus::Stale { version }
            };
        }
    }
    LockfileStatus::MissingPackage
}

fn describe_dependency(status: &DependencyStatus) -> String {
    match status {
        DependencyStatus::Exact { version } => format!("exact {version}"),
        DependencyStatus::Loose { requirement } => format!("loose `{requirement}`"),
        DependencyStatus::Stale { version } => format!("stale `{version}`"),
        DependencyStatus::PathDependency { path } => format!("path dependency `{path}`"),
        DependencyStatus::Missing => "missing".to_string(),
        DependencyStatus::Unsupported { detail } => format!("unsupported ({detail})"),
    }
}

fn describe_lockfile(status: &LockfileStatus) -> String {
    match status {
        LockfileStatus::Matches { version } => format!("matches {version}"),
        LockfileStatus::Stale { version } => format!("stale {version}"),
        LockfileStatus::MissingPackage => "missing aomi-sdk package".to_string(),
        LockfileStatus::MissingLockfile => "missing Cargo.lock".to_string(),
        LockfileStatus::Unreadable { detail } => format!("unreadable ({detail})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_flags_loose_requirement() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = temp.path().join("Cargo.toml");
        fs::write(
            &manifest,
            r#"[package]
name = "demo"
version = "0.1.0"
edition = "2024"

[dependencies]
aomi-sdk = "3.0.1"
"#,
        )
        .unwrap();

        let report = check_project_sdk(temp.path(), "3.0.1").unwrap();
        assert!(matches!(
            report.dependency,
            DependencyStatus::Loose { ref requirement } if requirement == "3.0.1"
        ));
        assert!(!report.ok);
    }

    #[test]
    fn fix_rewrites_workspace_dependency_to_exact() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = temp.path().join("Cargo.toml");
        fs::write(
            &manifest,
            r#"[workspace]
members = []

[workspace.dependencies]
aomi-sdk = "3.0.1"
"#,
        )
        .unwrap();

        rewrite_manifest_pin(&manifest, "3.0.1").unwrap();
        let report = check_project_sdk(&manifest, "3.0.1").unwrap();
        assert!(matches!(
            report.dependency,
            DependencyStatus::Exact { ref version } if version == "3.0.1"
        ));
        assert!(report.ok);
    }
}
