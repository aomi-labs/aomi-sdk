//! SDK compatibility checks for deployable app repositories.
//!
//! Hosted backends load dynamic apps only when the app was built with the exact
//! `aomi-sdk` version compiled into the backend. These helpers make that
//! contract visible before deploy/activate, and can repair simple Cargo pins.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};
use serde::Serialize;

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
    let report = fix_project_sdk(&args.path, &required)?;
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

/// Rewrite the aomi-sdk pin to `={required}`, update the lockfile, and re-check.
pub fn fix_project_sdk(path: &Path, required: &str) -> Result<SdkCheckReport> {
    let manifest_path = manifest_path(path)?;
    rewrite_manifest_pin(&manifest_path, required)?;
    run_cargo_update(&manifest_path, required)?;
    check_project_sdk(&manifest_path, required)
}

/// What the pre-deploy/activate SDK gate established, for the caller's summary.
pub struct SdkGate {
    /// The version the app must pin — the backend's answer, or this binary's
    /// own SDK version when no backend URL was resolvable.
    pub required: String,
    pub from_backend: bool,
    /// Whether the gate rewrote Cargo.toml/Cargo.lock during this run.
    pub repinned: bool,
    pub manifest_path: PathBuf,
}

pub async fn ensure_project_sdk(
    project_path: &Path,
    backend_url: Option<&str>,
    fix: bool,
) -> Result<SdkGate> {
    let required = resolve_required_sdk_version(backend_url, None)
        .await
        .context("failed to resolve backend-required aomi-sdk version")?;
    let from_backend = backend_url.is_some_and(|url| !url.trim().is_empty());
    if from_backend && required != aomi_sdk::AOMI_SDK_VERSION {
        eprintln!(
            "  ! this aomi-build was built with SDK {} but the backend runs {required} — \
             update aomi-build if checks or codegen misbehave",
            aomi_sdk::AOMI_SDK_VERSION
        );
    }
    let report = check_project_sdk(project_path, &required)?;
    if report.ok {
        return Ok(SdkGate {
            required,
            from_backend,
            repinned: false,
            manifest_path: report.manifest_path,
        });
    }
    if !fix {
        bail!(
            "SDK check failed before deploy/activate:\n{}\n\nRun:\n  aomi-build sdk fix --path {} --backend <backend-url>",
            report.blocking_messages().join("\n"),
            report.manifest_path.display()
        );
    }
    // Announce the repair — this rewrites the user's files, and an unannounced
    // rewrite is how a mismatch used to stay invisible until activation failed.
    println!("  ! aomi-sdk pin mismatch:");
    for message in report.blocking_messages() {
        println!("      {message}");
    }
    let rerun = fix_project_sdk(&report.manifest_path, &required)?;
    if !rerun.ok {
        bail!(
            "SDK auto-fix did not resolve all issues:\n{}",
            rerun.blocking_messages().join("\n")
        );
    }
    println!("  ✓ repinned Cargo.toml/Cargo.lock to aomi-sdk ={required} (not yet committed)");
    Ok(SdkGate {
        required,
        from_backend,
        repinned: true,
        manifest_path: rerun.manifest_path,
    })
}

/// Verify the aomi-sdk pin in the *committed* manifest a deploy actually ships.
///
/// The deploy sends a commit SHA and the backend syncs that commit from GitHub,
/// so a working-tree check (or `--fix-sdk` repair) that isn't committed changes
/// nothing about what gets built. Without this guard that ended as
/// `loaded=false` twenty minutes later, with nothing connecting the failure
/// back to the version mismatch.
pub fn ensure_committed_pin(git_root: &Path, commit: &str, gate: &SdkGate) -> Result<()> {
    let short = &commit[..commit.len().min(7)];
    let required = &gate.required;
    let rel = gate
        .manifest_path
        .strip_prefix(git_root)
        .unwrap_or(Path::new("Cargo.toml"));
    let Some(manifest) = file_at_commit(git_root, commit, rel) else {
        bail!(
            "the deploy ships commit {short}, but `{}` is not committed there — \
             commit and push it, then re-deploy",
            rel.display()
        );
    };
    let doc = manifest
        .parse::<toml_edit::DocumentMut>()
        .with_context(|| format!("failed to parse `{}` at commit {short}", rel.display()))?;
    let dependency = inspect::dependency_status(&doc, required);
    let dependency_ok =
        matches!(dependency, DependencyStatus::Exact { ref version } if version == required);
    let lock_rel = rel.with_file_name("Cargo.lock");
    // A lockfile missing from the commit mirrors the working-tree rule: only a
    // committed-but-stale lock blocks.
    let lockfile_ok = match file_at_commit(git_root, commit, &lock_rel) {
        None => true,
        Some(text) => matches!(
            inspect::lockfile_status_text(&text, required),
            LockfileStatus::Matches { ref version } if version == required
        ),
    };
    if dependency_ok && lockfile_ok {
        return Ok(());
    }
    let found = describe_dependency(&dependency);
    if gate.repinned {
        bail!(
            "the working tree was repinned to aomi-sdk ={required}, but the deploy ships \
             commit {short}, whose committed pin is still {found}.\n\
             Commit and push the repin, then re-deploy:\n  \
             git add {} {} && git commit -m \"pin aomi-sdk ={required}\" && git push",
            rel.display(),
            lock_rel.display()
        );
    }
    bail!(
        "the deploy ships commit {short}, whose committed Cargo.toml/Cargo.lock does not \
         pin aomi-sdk ={required} ({found}), even though the working tree does.\n\
         Commit and push {} and {}, then re-deploy.",
        rel.display(),
        lock_rel.display()
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

mod inspect;

pub use inspect::check_project_sdk;
use inspect::{
    describe_dependency, describe_lockfile, file_at_commit, manifest_path, rewrite_manifest_pin,
    run_cargo_update,
};

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;

    fn git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed");
    }

    fn committed_repo(pin: &str) -> (tempfile::TempDir, PathBuf, String) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.email", "t@example.test"]);
        git(&root, &["config", "user.name", "Test"]);
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n\n[dependencies]\naomi-sdk = \"{pin}\"\n"
            ),
        )
        .unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-q", "-m", "init"]);
        let head = String::from_utf8(
            Command::new("git")
                .current_dir(&root)
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("rev-parse")
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        (tmp, root, head)
    }

    fn gate(root: &Path, required: &str, repinned: bool) -> SdkGate {
        SdkGate {
            required: required.to_string(),
            from_backend: true,
            repinned,
            manifest_path: root.join("Cargo.toml"),
        }
    }

    #[test]
    fn committed_pin_passes_when_head_matches() {
        let (_tmp, root, head) = committed_repo("=3.0.1");
        ensure_committed_pin(&root, &head, &gate(&root, "3.0.1", false)).unwrap();
    }

    #[test]
    fn committed_pin_blocks_a_repin_that_is_not_committed() {
        let (_tmp, root, head) = committed_repo("=3.0.0");
        // Simulate `--fix-sdk`: the working tree is repaired, HEAD is not.
        rewrite_manifest_pin(&root.join("Cargo.toml"), "3.0.1").unwrap();
        let err = ensure_committed_pin(&root, &head, &gate(&root, "3.0.1", true)).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("git add"), "unhelpful message: {message}");
        assert!(
            message.contains("3.0.0"),
            "should name the stale pin: {message}"
        );
    }

    #[test]
    fn committed_pin_blocks_when_manifest_is_not_in_the_commit() {
        let (_tmp, root, head) = committed_repo("=3.0.1");
        // A commit that predates the manifest: point at a tree without it.
        fs::remove_file(root.join("Cargo.toml")).unwrap();
        git(&root, &["add", "."]);
        git(&root, &["commit", "-q", "-m", "drop manifest"]);
        let head2 = String::from_utf8(
            Command::new("git")
                .current_dir(&root)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        assert_ne!(head, head2);
        let err = ensure_committed_pin(&root, &head2, &gate(&root, "3.0.1", false)).unwrap_err();
        assert!(format!("{err:#}").contains("not committed"));
    }
}
