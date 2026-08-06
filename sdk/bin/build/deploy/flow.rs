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

pub use super::backend::TokenCheck;
use super::backend::BackendClient;
use super::build_client::BuildClient;
use super::platform::Platform;
use super::types::DeploymentStatusResult;

/// Check whether an activation token works for `platform`. Returns
/// [`TokenCheck::Invalid`] for an empty/malformed token or an auth rejection,
/// and [`TokenCheck::Unreachable`] when the backend couldn't be reached (so a
/// network blip isn't mistaken for a bad token).
pub async fn validate_activation_token(
    backend_url: &str,
    token: &str,
    platform: &str,
) -> TokenCheck {
    match BackendClient::new(backend_url.to_string(), token.to_string()) {
        Ok(client) => client.check_token(&Platform::new(platform)).await,
        Err(_) => TokenCheck::Invalid,
    }
}

/// Terminal outcome of waiting on a deployment's release build.
pub enum DeployReady {
    Ready,
    Failed(String),
    TimedOut,
}

/// Consecutive status-poll failures tolerated before surfacing the error. Right
/// after deploy the status row can briefly 404 while it materializes (the portal
/// no longer masks that as `pending`), so we ride out a short window — but a
/// persistent error is surfaced, not silently waited out to the timeout.
const MAX_STATUS_FAILURES: u32 = 15;
/// GitHub can briefly report no runs immediately after the platform commit is
/// pushed. Do not turn that registration race into a terminal build failure.
const NO_CI_GRACE: Duration = Duration::from_secs(90);

pub async fn poll_build_deployment_ready(
    client: &BuildClient,
    platform: &str,
    deployment_id: &str,
    timeout: Duration,
    mut on_state: impl FnMut(&str),
) -> Result<DeployReady> {
    let started = Instant::now();
    let mut last_state: Option<String> = None;
    let mut failures: u32 = 0;
    loop {
        match client.status(platform, deployment_id).await {
            Ok(status) => {
                failures = 0;
                if last_state.as_deref() != Some(status.state.as_str()) {
                    on_state(&status.state);
                    last_state = Some(status.state.clone());
                }
                match status.state.as_str() {
                    "ready" => return Ok(DeployReady::Ready),
                    "failed" => {
                        return Ok(DeployReady::Failed(status_failure_detail(
                            &status,
                            "release build failed",
                        )));
                    }
                    "no_ci" if started.elapsed() >= NO_CI_GRACE => {
                        return Ok(DeployReady::Failed(status_failure_detail(
                            &status,
                            "no CI ran for this deployment commit",
                        )));
                    }
                    _ => {}
                }
            }
            Err(error) => {
                failures += 1;
                if failures >= MAX_STATUS_FAILURES {
                    return Err(error.context("deployment status polling failed repeatedly"));
                }
            }
        }
        if started.elapsed() >= timeout {
            return Ok(DeployReady::TimedOut);
        }
        tokio::time::sleep(Duration::from_secs(6)).await;
    }
}

fn status_failure_detail(status: &DeploymentStatusResult, fallback: &str) -> String {
    let mut detail = status
        .message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .unwrap_or(fallback)
        .to_string();
    if let Some(url) = status
        .ci
        .as_ref()
        .and_then(|ci| ci.url.as_deref())
        .map(str::trim)
        .filter(|url| !url.is_empty())
        && !detail.contains(url)
    {
        detail.push_str("\nBuild logs: ");
        detail.push_str(url);
    }
    detail
}

#[cfg(test)]
mod tests {
    use super::status_failure_detail;
    use crate::deploy::types::{DeploymentCiStatus, DeploymentStatusResult};

    #[test]
    fn failed_status_surfaces_build_logs() {
        let status = DeploymentStatusResult {
            state: "failed".to_string(),
            message: None,
            ci: Some(DeploymentCiStatus {
                url: Some("https://github.com/aomi-labs/community-apps/actions/runs/1".to_string()),
            }),
        };
        assert_eq!(
            status_failure_detail(&status, "release build failed"),
            "release build failed\nBuild logs: https://github.com/aomi-labs/community-apps/actions/runs/1"
        );
    }
}
