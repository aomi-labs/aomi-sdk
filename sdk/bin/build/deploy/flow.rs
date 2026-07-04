//! Operation core shared by the clap subcommands and (later) the interactive
//! wizard. Functions here are front-end-agnostic: they take resolved inputs and
//! return typed results, doing no argument parsing and no TTY prompting. The
//! TTY/wizard layers own user interaction; this layer owns "what to call".
//!
//! M1 seeds the core with the `connect` primitives (GitHub App install URL +
//! activation-token validation). Deploy/activate/sync/scaffold get extracted
//! here as the wizard is built.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use reqwest::header::{ACCEPT, USER_AGENT};

pub use super::backend::TokenCheck;
use super::backend::{self, BackendClient};
use super::platform::Platform;
use super::types::{OAuthStart, SyncSourceInput};

/// Fetch the aomi-build GitHub App install URL for `platform` (optionally scoped
/// to a single `repo`). `mode` is `"install"` for a fresh install or
/// `"authorize"` to re-consent an existing one.
pub async fn oauth_install_url(
    backend_url: &str,
    platform: &str,
    repo: Option<&str>,
    mode: &str,
) -> Result<OAuthStart> {
    backend::oauth_start(backend_url, platform, repo, mode).await
}

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
const GITHUB_STATUS_INTERVAL: Duration = Duration::from_secs(30);

/// Poll `deployments/:id/status` until the release build reaches a terminal
/// state or `timeout` elapses, calling `on_state` whenever the reported state
/// changes (for progress output). Transient errors are tolerated for the brief
/// post-deploy window where the status row hasn't materialized, but a persistent
/// error is surfaced rather than masked (matching the portal, which stopped
/// turning status 404s into a fake `pending`). Mirrors the portal gating
/// activation on `ready`.
pub async fn poll_deployment_ready(
    backend_url: &str,
    token: &str,
    platform: &str,
    deployment_id: &str,
    timeout: Duration,
    mut on_state: impl FnMut(&str),
) -> Result<DeployReady> {
    poll_deployment_ready_with_pr(
        backend_url,
        token,
        platform,
        deployment_id,
        None,
        timeout,
        &mut on_state,
    )
    .await
}

pub async fn poll_deployment_ready_with_pr(
    backend_url: &str,
    token: &str,
    platform: &str,
    deployment_id: &str,
    pr_url: Option<&str>,
    timeout: Duration,
    mut on_state: impl FnMut(&str),
) -> Result<DeployReady> {
    let client = BackendClient::new(backend_url.to_string(), token.to_string())?;
    let platform = Platform::new(platform);
    let github_pr = pr_url.and_then(parse_github_pr_url);
    let github = GithubStatusClient::new();
    let started = Instant::now();
    let mut last_state: Option<String> = None;
    let mut last_github_state: Option<String> = None;
    let mut last_github_poll: Option<Instant> = None;
    let mut failures: u32 = 0;
    loop {
        match client.deployment_status(&platform, deployment_id).await {
            Ok(status) => {
                failures = 0;
                if last_state.as_deref() != Some(status.state.as_str()) {
                    on_state(&status.state);
                    last_state = Some(status.state.clone());
                }
                match status.state.as_str() {
                    "ready" => return Ok(DeployReady::Ready),
                    "failed" => {
                        return Ok(DeployReady::Failed(
                            status
                                .message
                                .unwrap_or_else(|| "release build failed".to_string()),
                        ));
                    }
                    "no_ci" => {
                        return Ok(DeployReady::Failed(status.message.unwrap_or_else(|| {
                            "no CI ran for this deployment commit".to_string()
                        })));
                    }
                    _ => {}
                }
            }
            Err(e) => {
                failures += 1;
                if failures >= MAX_STATUS_FAILURES {
                    return Err(e.context("deployment status polling failed repeatedly"));
                }
            }
        }
        if let Some(pr) = &github_pr {
            let due = last_github_poll
                .map(|last| last.elapsed() >= GITHUB_STATUS_INTERVAL)
                .unwrap_or(true);
            if due {
                last_github_poll = Some(Instant::now());
                match github.checks(pr).await {
                    Ok(GithubCheckState::Passed) => {
                        if last_github_state.as_deref() != Some("github_checks_passed") {
                            on_state("github_checks_passed");
                        }
                        return Ok(DeployReady::Ready);
                    }
                    Ok(GithubCheckState::Pending) => {
                        if last_github_state.as_deref() != Some("github_checks_pending") {
                            on_state("github_checks_pending");
                            last_github_state = Some("github_checks_pending".to_string());
                        }
                    }
                    Ok(GithubCheckState::Failed(detail)) => {
                        return Ok(DeployReady::Failed(format!(
                            "GitHub checks failed: {detail}"
                        )));
                    }
                    Err(err) => {
                        if last_github_state.as_deref() != Some("github_status_unavailable") {
                            on_state("github_status_unavailable");
                            last_github_state = Some("github_status_unavailable".to_string());
                        }
                        let _ = err;
                    }
                }
            }
        }
        if started.elapsed() >= timeout {
            return Ok(DeployReady::TimedOut);
        }
        tokio::time::sleep(Duration::from_secs(6)).await;
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct GithubPrRef {
    owner: String,
    repo: String,
    number: u64,
}

pub(crate) fn parse_github_pr_url(value: &str) -> Option<GithubPrRef> {
    let path = value
        .trim()
        .strip_prefix("https://github.com/")
        .or_else(|| value.trim().strip_prefix("http://github.com/"))?;
    let mut parts = path.trim_matches('/').split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    if parts.next()? != "pull" {
        return None;
    }
    let number = parts.next()?.parse().ok()?;
    Some(GithubPrRef {
        owner,
        repo,
        number,
    })
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) enum GithubCheckState {
    Pending,
    Passed,
    Failed(String),
}

#[derive(Clone)]
struct GithubStatusClient {
    http: reqwest::Client,
    token: Option<String>,
}

impl GithubStatusClient {
    fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
            token: std::env::var("GITHUB_TOKEN")
                .ok()
                .map(|token| token.trim().to_string())
                .filter(|token| !token.is_empty()),
        }
    }

    async fn checks(&self, pr: &GithubPrRef) -> Result<GithubCheckState> {
        let pull = self
            .get_json(&format!(
                "https://api.github.com/repos/{}/{}/pulls/{}",
                pr.owner, pr.repo, pr.number
            ))
            .await?;
        let sha = pull
            .pointer("/head/sha")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| anyhow!("GitHub PR response omitted head.sha"))?;
        let runs = self
            .get_json(&format!(
                "https://api.github.com/repos/{}/{}/commits/{sha}/check-runs?filter=latest&per_page=100",
                pr.owner, pr.repo
            ))
            .await?;
        let run_state = check_runs_state(&runs);
        if run_count(&runs) == 0 {
            let statuses = self
                .get_json(&format!(
                    "https://api.github.com/repos/{}/{}/commits/{sha}/status",
                    pr.owner, pr.repo
                ))
                .await?;
            combined_status_state(&statuses)
        } else {
            Ok(run_state)
        }
    }

    async fn get_json(&self, url: &str) -> Result<serde_json::Value> {
        let mut req = self
            .http
            .get(url)
            .header(USER_AGENT, "aomi-build")
            .header(ACCEPT, "application/vnd.github+json");
        if let Some(token) = &self.token {
            req = req.bearer_auth(token);
        }
        let response = req
            .send()
            .await
            .with_context(|| format!("failed to read GitHub status {url}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read GitHub status response body")?;
        if !status.is_success() {
            return Err(anyhow!(
                "GitHub status endpoint {url} returned {status}: {}",
                text.trim()
            ));
        }
        serde_json::from_str(&text).context("GitHub status response was not valid JSON")
    }
}

fn run_count(value: &serde_json::Value) -> usize {
    value
        .get("check_runs")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

pub(crate) fn check_runs_state(value: &serde_json::Value) -> GithubCheckState {
    let Some(runs) = value
        .get("check_runs")
        .and_then(serde_json::Value::as_array)
    else {
        return GithubCheckState::Pending;
    };
    if runs.is_empty() {
        return GithubCheckState::Pending;
    }
    let mut pending = false;
    for run in runs {
        let name = run
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("check");
        let status = run
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if status != "completed" {
            pending = true;
            continue;
        }
        match run.get("conclusion").and_then(serde_json::Value::as_str) {
            Some("success" | "skipped" | "neutral") => {}
            Some(other) => {
                return GithubCheckState::Failed(format!("{name} concluded {other}"));
            }
            None => pending = true,
        }
    }
    if pending {
        GithubCheckState::Pending
    } else {
        GithubCheckState::Passed
    }
}

pub(crate) fn combined_status_state(value: &serde_json::Value) -> Result<GithubCheckState> {
    let state = value
        .get("state")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow!("GitHub combined status response omitted state"))?;
    match state {
        "success" => Ok(GithubCheckState::Passed),
        "pending" => Ok(GithubCheckState::Pending),
        "failure" | "error" => Ok(GithubCheckState::Failed(format!(
            "combined status is {state}"
        ))),
        other => Err(anyhow!("unknown GitHub combined status `{other}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_pr_urls() {
        assert_eq!(
            parse_github_pr_url("https://github.com/aomi-labs/community-apps/pull/83"),
            Some(GithubPrRef {
                owner: "aomi-labs".into(),
                repo: "community-apps".into(),
                number: 83,
            })
        );
        assert!(parse_github_pr_url("https://github.com/aomi-labs/community-apps").is_none());
        assert!(parse_github_pr_url("https://example.com/a/b/pull/1").is_none());
    }

    #[test]
    fn interprets_check_runs() {
        assert_eq!(
            check_runs_state(&serde_json::json!({
                "check_runs": [
                    { "name": "build", "status": "completed", "conclusion": "success" }
                ]
            })),
            GithubCheckState::Passed
        );
        assert_eq!(
            check_runs_state(&serde_json::json!({
                "check_runs": [
                    { "name": "build", "status": "queued", "conclusion": null }
                ]
            })),
            GithubCheckState::Pending
        );
        assert_eq!(
            check_runs_state(&serde_json::json!({
                "check_runs": [
                    { "name": "build", "status": "completed", "conclusion": "failure" }
                ]
            })),
            GithubCheckState::Failed("build concluded failure".into())
        );
    }

    #[test]
    fn interprets_combined_status() {
        assert_eq!(
            combined_status_state(&serde_json::json!({ "state": "success" })).unwrap(),
            GithubCheckState::Passed
        );
        assert_eq!(
            combined_status_state(&serde_json::json!({ "state": "pending" })).unwrap(),
            GithubCheckState::Pending
        );
        assert_eq!(
            combined_status_state(&serde_json::json!({ "state": "failure" })).unwrap(),
            GithubCheckState::Failed("combined status is failure".into())
        );
    }
}
