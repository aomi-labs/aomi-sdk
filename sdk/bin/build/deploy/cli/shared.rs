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
pub(crate) const PROJECT_ID_ENV: &str = "AOMI_PROJECT_ID";
pub(crate) const ADMIN_KEY_ENV: &str = "AOMI_ADMIN_KEY";
pub(crate) const ADMIN_KID_ENV: &str = "AOMI_ADMIN_KID";

pub(crate) fn env_value(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(crate) fn clean_list(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
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

/// `--build-url` flag → `AOMI_BUILD_URL` → known backend environment mapping
/// → saved login config. An explicitly selected staging/production backend must
/// not accidentally reuse a saved Build URL from the other environment.
///
/// The env var still wins over the inferred URL — pointing at a local Build is a
/// legitimate thing to do — but silently pairing a staging backend with the
/// production Builder is not something a user ever means, so a stale exported
/// `AOMI_BUILD_URL` that contradicts the chosen backend is called out.
pub(crate) fn resolve_build_url(
    flag: &Option<String>,
    backend_url: Option<&str>,
) -> Option<String> {
    if let Some(flag) = flag
        .clone()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
    {
        return Some(flag);
    }
    if let Some(from_env) = env_value(BUILD_URL_ENV) {
        let from_env = from_env.trim_end_matches('/').to_string();
        if let Some(inferred) = backend_url.and_then(infer_build_url)
            && inferred != from_env
        {
            warn_once(&format!(
                "{BUILD_URL_ENV}={from_env} overrides the Aomi Build URL for the selected \
                 backend ({inferred}). If that is not deliberate, unset {BUILD_URL_ENV} \
                 or pass --build-url {inferred}."
            ));
        }
        return Some(from_env);
    }
    backend_url
        .and_then(infer_build_url)
        .or_else(|| AomiConfig::load().build_url)
}

/// Emit a warning at most once per process — the resolvers run on every
/// command step, and a warning repeated four times per deploy is noise.
fn warn_once(message: &str) {
    static WARNED: std::sync::Once = std::sync::Once::new();
    WARNED.call_once(|| eprintln!("  warning: {message}"));
}

pub(crate) fn infer_build_url(backend_url: &str) -> Option<String> {
    match backend_url.trim().trim_end_matches('/') {
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
    command: &str,
    backend: &Option<String>,
    token: &Option<String>,
) -> Result<(String, String)> {
    let url = resolve_backend(backend).ok_or_else(|| missing_backend(command))?;
    let tok = resolve_activation_token(token).ok_or_else(|| missing_activation_token(command))?;
    Ok((url, tok))
}

pub(crate) fn missing_backend(command: &str) -> anyhow::Error {
    missing_flag_or_env(
        command,
        "a backend URL",
        "--backend <url>",
        BACKEND_URL_ENV,
        "<url>",
        None,
    )
}

pub(crate) fn missing_activation_token(command: &str) -> anyhow::Error {
    missing_flag_or_env(
        command,
        "an activation token",
        "--activation-token <token>",
        ACTIVATION_TOKEN_ENV,
        "<token>",
        Some(&format!(
            "Or save it once:\n  {} connect --activation-token <token>",
            bin_name()
        )),
    )
}

pub(crate) fn missing_admin_key(command: &str) -> anyhow::Error {
    missing_flag_or_env(
        command,
        "the privileged admin signing key",
        "--admin-key <pkcs8-pem-or-path>",
        ADMIN_KEY_ENV,
        "<pkcs8-pem-or-path>",
        Some("This is an out-of-band admin/service signing key, not an activation token."),
    )
}

pub(crate) fn missing_admin_kid(command: &str) -> anyhow::Error {
    missing_flag_or_env(
        command,
        "the admin issuer key id",
        "--admin-kid <kid>",
        ADMIN_KID_ENV,
        "<kid>",
        Some("Example kid: aomi-admin-staging-1"),
    )
}

fn missing_flag_or_env(
    command: &str,
    need: &str,
    flag_example: &str,
    env_name: &str,
    env_example: &str,
    extra: Option<&str>,
) -> anyhow::Error {
    let bin = bin_name();
    let mut message = format!(
        "{command} needs {need}.\n\n\
         Pass it for this run:\n  {bin} {command} {flag_example}\n\n\
         Or export it:\n  export {env_name}={env_example}"
    );
    if let Some(extra) = extra {
        message.push_str("\n\n");
        message.push_str(extra);
    }
    anyhow!(message)
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

/// Current branch name, or `None` when detached (or git fails).
pub(crate) fn head_branch(git_root: &Path) -> Option<String> {
    let name = git_output_at(git_root, ["rev-parse", "--abbrev-ref", "HEAD"]).ok()?;
    let name = name.trim();
    (!name.is_empty() && name != "HEAD").then(|| name.to_string())
}

/// Whether tracked files differ from HEAD. Untracked files are ignored — they
/// wouldn't ship either, but counting them would flag every repo forever once
/// the CLI writes its own (usually untracked) `.aomi/deployment.json`.
/// `None` when git can't say.
pub(crate) fn worktree_dirty(git_root: &Path) -> Option<bool> {
    git_output_at(git_root, ["status", "--porcelain", "--untracked-files=no"])
        .ok()
        .map(|out| !out.trim().is_empty())
}

/// Whether `commit` is reachable from any remote-tracking ref — i.e. whether a
/// deploy that ships this commit can be synced by the backend from GitHub.
/// `None` when git can't say (no remotes fetched, shallow clone, …).
pub(crate) fn commit_on_remote(git_root: &Path, commit: &str) -> Option<bool> {
    git_output_at(git_root, ["branch", "-r", "--contains", commit])
        .ok()
        .map(|out| !out.trim().is_empty())
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
