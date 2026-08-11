//! Reading (and repairing) an app repo's aomi-sdk pin.
//!
//! Everything here works off `Cargo.toml` and `Cargo.lock` on disk — no network
//! and no CLI. `mod.rs` owns the commands that decide what to do with the answer.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use toml_edit::{DocumentMut, value};

use super::{DependencyStatus, LockfileStatus, SdkCheckReport};

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

pub(super) fn manifest_path(path: &Path) -> Result<PathBuf> {
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

pub(super) fn dependency_status(doc: &DocumentMut, required: &str) -> DependencyStatus {
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

pub(super) fn requirement_status(raw: &str, required: &str) -> DependencyStatus {
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

pub(super) fn find_dependency_item(doc: &DocumentMut) -> Option<&toml_edit::Item> {
    doc.get("dependencies")
        .and_then(|deps| deps.get("aomi-sdk"))
        .or_else(|| {
            doc.get("workspace")
                .and_then(|workspace| workspace.get("dependencies"))
                .and_then(|deps| deps.get("aomi-sdk"))
        })
}

pub(super) fn rewrite_manifest_pin(manifest_path: &Path, required: &str) -> Result<()> {
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

pub(super) fn run_cargo_update(manifest_path: &Path, required: &str) -> Result<()> {
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

pub(super) fn find_lockfile(manifest_path: &Path) -> Option<PathBuf> {
    let dir = manifest_path.parent()?;
    let direct = dir.join("Cargo.lock");
    if direct.exists() {
        return Some(direct);
    }
    None
}

pub(super) fn lockfile_status(lockfile_path: Option<&Path>, required: &str) -> LockfileStatus {
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
    lockfile_status_text(&text, required)
}

pub(super) fn lockfile_status_text(text: &str, required: &str) -> LockfileStatus {
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

/// The content of `rel_path` as committed at `commit`, or `None` when the file
/// isn't in that commit (or git fails).
pub(super) fn file_at_commit(git_root: &Path, commit: &str, rel_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(git_root)
        .arg("show")
        .arg(format!("{commit}:{}", rel_path.display()))
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

pub(super) fn describe_dependency(status: &DependencyStatus) -> String {
    match status {
        DependencyStatus::Exact { version } => format!("exact {version}"),
        DependencyStatus::Loose { requirement } => format!("loose `{requirement}`"),
        DependencyStatus::Stale { version } => format!("stale `{version}`"),
        DependencyStatus::PathDependency { path } => format!("path dependency `{path}`"),
        DependencyStatus::Missing => "missing".to_string(),
        DependencyStatus::Unsupported { detail } => format!("unsupported ({detail})"),
    }
}

pub(super) fn describe_lockfile(status: &LockfileStatus) -> String {
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
