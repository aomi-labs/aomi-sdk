//! Cross-command helpers shared by the hosted CLI commands: env/credential
//! resolution and the git facts deploy/request read from the working tree.

use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use anyhow::{Context, Result, anyhow, bail};

use crate::deploy::config::AomiConfig;

pub(crate) const ACTIVATION_TOKEN_ENV: &str = "AOMI_APP_ACTIVATION_TOKEN";
pub(crate) const BACKEND_URL_ENV: &str = "AOMI_BACKEND_URL";
pub(crate) const APP_SOURCE_ID_ENV: &str = "AOMI_APP_SOURCE_ID";
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CredentialSource {
    Flag,
    Env,
    Config,
}

impl CredentialSource {
    pub(crate) fn label(self) -> &'static str {
        match self {
            CredentialSource::Flag => "--activation-token",
            CredentialSource::Env => ACTIVATION_TOKEN_ENV,
            CredentialSource::Config => "~/.config/aomi/config.toml",
        }
    }

    pub(crate) fn stale_hint(self) -> &'static str {
        match self {
            CredentialSource::Env => {
                "AOMI_APP_ACTIVATION_TOKEN overrides the saved connect token; unset it if it is stale."
            }
            CredentialSource::Flag | CredentialSource::Config => {
                "Run `aomi-build connect` with a valid activation token if this token is stale."
            }
        }
    }
}

/// `--backend` flag → `AOMI_BACKEND_URL` → saved `connect` config.
pub(crate) fn resolve_backend(flag: &Option<String>) -> Option<String> {
    flag.clone()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .or_else(|| env_value(BACKEND_URL_ENV))
        .or_else(|| AomiConfig::load().backend_url)
}

/// `--activation-token` flag → `AOMI_APP_ACTIVATION_TOKEN` → saved `connect`
/// config. Lets a connected user run deploy/activate with no env wiring.
pub(crate) fn resolve_activation_token(flag: &Option<String>) -> Option<String> {
    resolve_activation_token_with_source(flag).map(|(token, _)| token)
}

pub(crate) fn resolve_activation_token_with_source(
    flag: &Option<String>,
) -> Option<(String, CredentialSource)> {
    flag.clone()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .map(|token| (token, CredentialSource::Flag))
        .or_else(|| env_value(ACTIVATION_TOKEN_ENV).map(|token| (token, CredentialSource::Env)))
        .or_else(|| {
            AomiConfig::load()
                .activation_token
                .map(|token| (token, CredentialSource::Config))
        })
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
        "--admin-key-file <path-to-pkcs8-pem>",
        ADMIN_KEY_ENV,
        "<pkcs8-pem-or-path>",
        Some(
            "This is an out-of-band admin/service signing key, not an activation token.\n\
             It is never accepted as a command-line argument — process arguments are \n\
             visible to other users and recorded in shell history.",
        ),
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
