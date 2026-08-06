//! Cross-command helpers shared by the hosted CLI commands: env/credential
//! resolution and the git facts deploy/request read from the working tree.

use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, anyhow, bail};

use crate::deploy::config::AomiConfig;

pub(crate) const ACTIVATION_TOKEN_ENV: &str = "AOMI_APP_ACTIVATION_TOKEN";
pub(crate) const BACKEND_URL_ENV: &str = "AOMI_BACKEND_URL";
pub(crate) const BUILD_URL_ENV: &str = "AOMI_BUILD_URL";
pub(crate) const BUILD_TOKEN_ENV: &str = "AOMI_BUILD_TOKEN";
pub(crate) const APP_SOURCE_ID_ENV: &str = "AOMI_APP_SOURCE_ID";
pub(crate) const ADMIN_KEY_ENV: &str = "AOMI_ADMIN_KEY";
pub(crate) const ADMIN_KID_ENV: &str = "AOMI_ADMIN_KID";

pub(crate) fn env_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(crate) fn bin_name() -> String {
    std::env::args()
        .next()
        .and_then(|arg| {
            Path::new(&arg)
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
        })
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "aomi-build".to_string())
}

/// `--backend` flag → `AOMI_BACKEND_URL` → saved `connect` config.
pub(crate) fn resolve_backend(flag: &Option<String>) -> Option<String> {
    flag.clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| env_value(BACKEND_URL_ENV))
        .or_else(|| AomiConfig::load().backend_url)
}

/// `--build-url` flag → `AOMI_BUILD_URL` → saved login config → known backend
/// environment mapping.
pub(crate) fn resolve_build_url(
    flag: &Option<String>,
    backend_url: Option<&str>,
) -> Option<String> {
    flag.clone()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| env_value(BUILD_URL_ENV))
        .or_else(|| AomiConfig::load().build_url)
        .or_else(|| infer_build_url(backend_url?))
}

pub(crate) fn infer_build_url(backend_url: &str) -> Option<String> {
    let url = backend_url.trim().trim_end_matches('/');
    match url {
        "https://api-staging.aomi.dev" => Some("https://build-staging.aomi.dev".to_string()),
        "https://api.aomi.dev" => Some("https://build.aomi.dev".to_string()),
        _ => None,
    }
}

/// `--activation-token` flag → `AOMI_APP_ACTIVATION_TOKEN` → saved `connect`
/// config. Lets a connected user run deploy/activate with no env wiring.
pub(crate) fn resolve_activation_token(flag: &Option<String>) -> Option<String> {
    flag.clone()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .or_else(|| env_value(ACTIVATION_TOKEN_ENV))
        .or_else(|| AomiConfig::load().activation_token)
}

/// `--backend`/env + activation token resolution shared by the activation-token
/// bootstrap commands (source/apps/token list+revoke).
pub(crate) fn resolve_activation(
    backend: &Option<String>,
    token: &Option<String>,
) -> Result<(String, String)> {
    let url = resolve_backend(backend)
        .ok_or_else(|| anyhow!("needs a backend URL — set --backend or {BACKEND_URL_ENV}"))?;
    let tok = resolve_activation_token(token).ok_or_else(|| {
        anyhow!(
            "needs an activation token via --activation-token or {ACTIVATION_TOKEN_ENV} \
             (or run `aomi-build connect`)"
        )
    })?;
    Ok((url, tok))
}

pub(crate) fn git_context(start: impl AsRef<Path>) -> Result<(PathBuf, PathBuf)> {
    let start_dir = normalize_start_dir(start.as_ref())?;
    let root = git_output_at(&start_dir, ["rev-parse", "--show-toplevel"])
        .with_context(|| format!("failed to find git root from {}", start_dir.display()))?;
    let root = PathBuf::from(root.trim());
    let root = root.canonicalize().unwrap_or(root);
    Ok((root, start_dir))
}

pub(crate) fn head_commit(git_root: &Path) -> Result<String> {
    Ok(git_output_at(git_root, ["rev-parse", "HEAD"])?
        .trim()
        .to_string())
}

pub(crate) fn remote_origin(git_root: &Path) -> Result<String> {
    Ok(git_output_at(git_root, ["remote", "get-url", "origin"])?
        .trim()
        .to_string())
}

pub(crate) fn tracked_aomi_tomls(git_root: &Path) -> Result<Vec<String>> {
    let raw = git_output_at(
        git_root,
        ["ls-files", "-z", "--", "*aomi.toml", "aomi.toml"],
    )
    .with_context(|| format!("failed to list tracked files in {}", git_root.display()))?;
    let mut paths: Vec<String> = raw
        .split('\0')
        .filter(|entry| !entry.is_empty())
        .filter(|entry| entry.rsplit('/').next() == Some("aomi.toml"))
        .map(str::to_string)
        .collect();
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn normalize_start_dir(path: &Path) -> Result<PathBuf> {
    let path = if path.as_os_str().is_empty() {
        Path::new(".")
    } else {
        path
    };
    let path = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    Ok(if path.is_file() {
        path.parent()
            .ok_or_else(|| anyhow!("path has no parent: {}", path.display()))?
            .to_path_buf()
    } else {
        path
    })
}

fn git_output_at<const N: usize>(dir: &Path, args: [&str; N]) -> Result<String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("failed to run git in {}", dir.display()))?;
    if !output.status.success() {
        bail!(
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("git output was not valid UTF-8")
}
