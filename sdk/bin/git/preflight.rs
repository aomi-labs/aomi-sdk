//! Online preflight checks per ADR 0009. Fetches the backend's
//! `GET /api/control/platforms` registry and resolves the user's intent
//! against it — surfacing whether the chosen branch matches the platform's
//! contractual `deployment_branch`.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::deployment_state::{Check, DeploymentState};

#[derive(Debug, Clone, Deserialize)]
pub struct RemotePlatform {
    pub name: String,
    pub github_repo: String,
    pub deployment_branch: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListResponse {
    platforms: Vec<RemotePlatform>,
}

#[derive(Debug, Deserialize)]
struct ServerTagsResponse {
    server_tags: Vec<String>,
}

/// Fetch the registered platforms from the backend's public control endpoint.
pub async fn fetch_platforms(backend_url: &str) -> Result<Vec<RemotePlatform>> {
    let base = backend_url.trim().trim_end_matches('/');
    let url = format!("{base}/api/control/platforms");
    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to GET {url}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("GET {url} returned {status}: {}", body.trim());
    }
    let parsed: ListResponse = response
        .json()
        .await
        .with_context(|| format!("failed to parse {url} response"))?;
    Ok(parsed.platforms)
}

/// Fetch the current backend instance's server tags from the public control endpoint.
pub async fn fetch_server_tags(backend_url: &str) -> Result<Vec<String>> {
    let base = backend_url.trim().trim_end_matches('/');
    let url = format!("{base}/api/control/server-tags");
    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .with_context(|| format!("failed to GET {url}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("GET {url} returned {status}: {}", body.trim());
    }
    let parsed: ServerTagsResponse = response
        .json()
        .await
        .with_context(|| format!("failed to parse {url} response"))?;
    Ok(normalize_tags(parsed.server_tags))
}

/// Augment a deployment state with online preflight results. Mutates `state`
/// in place. Always extends `state.checks[]`; only sets
/// `state.platform.resolved_deploy_branch` and `state.state.deployed` when a
/// matching platform row was found.
pub async fn run(state: &mut DeploymentState, backend_url: &str) -> Result<()> {
    let Some(declared) = state.app.platform.clone() else {
        state.checks.push(Check::fail(
            "platform_resolved",
            "aomi.toml does not declare [app].platform",
        ));
        state.touch();
        return Ok(());
    };

    let platforms = match fetch_platforms(backend_url).await {
        Ok(p) => p,
        Err(e) => {
            state.checks.push(Check::fail(
                "backend_reachable",
                format!("GET /api/control/platforms failed: {e}"),
            ));
            state.errors.push(e.to_string());
            state.touch();
            return Ok(());
        }
    };
    state.checks.push(Check::pass(
        "backend_reachable",
        format!("found {} platforms", platforms.len()),
    ));

    let normalized = declared.trim().to_ascii_lowercase();
    let matched = platforms
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(&normalized));

    let Some(remote) = matched else {
        state.checks.push(Check::fail(
            "platform_resolved",
            format!("platform `{declared}` not registered with backend"),
        ));
        state.touch();
        return Ok(());
    };

    // Update platform intent with remote facts.
    state.platform.resolved_deploy_branch = Some(remote.deployment_branch.clone());
    state.checks.push(Check::pass(
        "platform_resolved",
        format!("{} -> {}", remote.name, remote.github_repo),
    ));

    // Compare user's chosen branch to the contractual deployment_branch.
    let on_release_branch = state.target.branch == remote.deployment_branch;
    state.checks.push(if on_release_branch {
        Check::pass(
            "branch_matches_contract",
            format!("{} == {}", state.target.branch, remote.deployment_branch),
        )
    } else {
        Check::fail(
            "branch_matches_contract",
            format!(
                "{} != {} (push will not be auto-deployed)",
                state.target.branch, remote.deployment_branch
            ),
        )
    });

    // Recompute the deployed flag from the freshly resolved branch.
    state.recompute_deployed();

    // Optionally verify the user's declared git URL matches the backend's
    // record. Mismatch isn't fatal — the user might be using a fork — but
    // surface it as a warning check.
    if let Some(user_git) = state.app.git.as_deref() {
        let normalized_user = normalize_github_url(user_git);
        let normalized_remote = normalize_github_url(&remote.github_repo);
        if normalized_user == normalized_remote {
            state.checks.push(Check::pass(
                "git_url_matches_platform",
                remote.github_repo.clone(),
            ));
        } else {
            state.checks.push(Check::fail(
                "git_url_matches_platform",
                format!(
                    "aomi.toml git={user_git} != platform.github_repo={}",
                    remote.github_repo
                ),
            ));
        }
    }

    if !state.target.server_tags.is_empty() {
        match fetch_server_tags(backend_url).await {
            Ok(server_tags) => {
                let requested = normalize_tags(state.target.server_tags.clone());
                let matches = requested.iter().all(|tag| server_tags.contains(tag));
                state.checks.push(if matches {
                    Check::pass(
                        "server_tags_match",
                        format!(
                            "target [{}] subset of server [{}]",
                            requested.join(","),
                            server_tags.join(",")
                        ),
                    )
                } else {
                    let detail = format!(
                        "target [{}] is not a subset of server [{}]",
                        requested.join(","),
                        server_tags.join(",")
                    );
                    state.errors.push(detail.clone());
                    Check::fail("server_tags_match", detail)
                });
            }
            Err(e) => {
                let detail = format!("GET /api/control/server-tags failed: {e}");
                state.errors.push(detail.clone());
                state.checks.push(Check::fail("server_tags_match", detail));
            }
        }
    }

    state.touch();
    Ok(())
}

fn normalize_tags(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|tag| tag.trim().to_ascii_lowercase())
        .filter(|tag| !tag.is_empty())
        .fold(Vec::new(), |mut acc, tag| {
            if !acc.contains(&tag) {
                acc.push(tag);
            }
            acc
        })
}

fn normalize_github_url(value: &str) -> String {
    let mut repo = value
        .trim()
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .to_string();
    for prefix in [
        "git@github.com:",
        "ssh://git@github.com/",
        "https://github.com/",
        "http://github.com/",
        "github.com/",
    ] {
        if let Some(stripped) = repo.strip_prefix(prefix) {
            repo = stripped.to_string();
            break;
        }
    }
    repo.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_tags_trims_lowercases_and_deduplicates() {
        assert_eq!(
            normalize_tags(vec![
                " Prod ".to_string(),
                "platform-x".to_string(),
                "prod".to_string(),
                String::new(),
            ]),
            vec!["prod", "platform-x"]
        );
    }
}
