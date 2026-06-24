//! Operation core shared by the clap subcommands and (later) the interactive
//! wizard. Functions here are front-end-agnostic: they take resolved inputs and
//! return typed results, doing no argument parsing and no TTY prompting. The
//! TTY/wizard layers own user interaction; this layer owns "what to call".
//!
//! M1 seeds the core with the `connect` primitives (GitHub App install URL +
//! activation-token validation). Deploy/activate/sync/scaffold get extracted
//! here as the wizard is built.

use std::time::{Duration, Instant};

use anyhow::Result;

use super::backend::{self, BackendClient};
use super::platform::Platform;
use super::types::{OAuthStart, SyncSourceInput};

/// Fetch the GitHub App install URL for `platform` (optionally scoped to a
/// single `repo`). The returned `state` (when present) is the poll handle for
/// the future result endpoint.
pub async fn oauth_install_url(
    backend_url: &str,
    platform: &str,
    repo: Option<&str>,
) -> Result<OAuthStart> {
    backend::oauth_start(backend_url, platform, repo).await
}

/// Light check that an activation token works for `platform` — a successful
/// `apps` read means the token is valid and scoped to that platform.
pub async fn validate_activation_token(backend_url: &str, token: &str, platform: &str) -> bool {
    match BackendClient::new(backend_url.to_string(), token.to_string()) {
        Ok(client) => client.list_apps(&Platform::new(platform)).await.is_ok(),
        Err(_) => false,
    }
}

/// Resolve-or-bind an installed source repo, returning its `app_source_id`.
pub async fn sync_source(
    backend_url: &str,
    token: &str,
    platform: &str,
    repo: &str,
) -> Result<i64> {
    let client = BackendClient::new(backend_url.to_string(), token.to_string())?;
    let result = client
        .sync_installed(
            &Platform::new(platform),
            &SyncSourceInput {
                repo: repo.to_string(),
            },
        )
        .await?;
    Ok(result.source.id)
}

/// Poll `oauth/result?state=…` until the browser install resolves an
/// installation id, or `timeout` elapses. `Ok(None)` means it timed out (the
/// caller falls back to manual entry); transient poll errors are ignored so a
/// blip doesn't abort the wait.
pub async fn poll_installation(
    backend_url: &str,
    state: &str,
    timeout: Duration,
) -> Result<Option<i64>> {
    let started = Instant::now();
    loop {
        if let Ok(result) = backend::oauth_result(backend_url, state).await {
            if result.status == "installed" {
                if let Some(id) = result.installation_id {
                    return Ok(Some(id));
                }
            }
        }
        if started.elapsed() >= timeout {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
