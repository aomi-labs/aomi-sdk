//! Online preflight checks per ADR 0009. Fetches the backend's
//! `GET /api/control/platforms` registry and resolves the user's intent
//! against it — surfacing whether the chosen branch matches the platform's
//! contractual `deployment_branch`.

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::json;

use crate::deployment_state::{Check, DeploymentState, Stage, StageId};
use crate::platform::{Platform, normalize_github_repo};

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
    /// The backend host's compiled `aomi-sdk` version. Optional so an older
    /// backend that doesn't report it degrades to a non-blocking warning.
    #[serde(default)]
    sdk_version: Option<String>,
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

/// Fetch the current backend instance's server tags and host `aomi-sdk`
/// version from the public control endpoint. The version is `None` when the
/// backend doesn't report it (older deploy).
pub async fn fetch_server_tags(backend_url: &str) -> Result<(Vec<String>, Option<String>)> {
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
    let sdk = parsed
        .sdk_version
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    Ok((normalize_tags(parsed.server_tags), sdk))
}

/// Resolve `platform` to its GitHub repo URL via the backend registry. Used
/// when neither `--git` nor `aomi.toml [app].git` was provided.
pub async fn lookup_platform_git(backend_url: &str, platform: &Platform) -> Result<String> {
    let platforms = fetch_platforms(backend_url).await?;
    let needle = platform.as_str().trim().to_ascii_lowercase();
    let matched = platforms
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(&needle))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "backend does not know about platform `{needle}` — declare [app].git in aomi.toml \
                 or pass --git <URL>"
            )
        })?;
    normalize_github_repo(&matched.github_repo)
}

/// Augment a deployment state with online preflight results. Mutates `state`
/// in place by replacing the platform/backend stages seeded by `to_state`.
pub async fn run(state: &mut DeploymentState, backend_url: &str) -> Result<()> {
    let Some(declared) = state.app.platform.clone() else {
        state.replace_stage(Stage::new(
            StageId::Platform,
            vec![Check::fail(
                "platform_resolved",
                "aomi.toml does not declare [app].platform",
            )],
        ));
        state.replace_stage(Stage::skipped(StageId::Backend, "platform did not resolve"));
        state.touch();
        return Ok(());
    };

    let platforms = match fetch_platforms(backend_url).await {
        Ok(p) => p,
        Err(e) => {
            state.replace_stage(Stage::new(
                StageId::Platform,
                vec![Check::fail(
                    "backend_reachable",
                    format!("GET /api/control/platforms failed: {e}"),
                )],
            ));
            state.replace_stage(Stage::skipped(
                StageId::Backend,
                "platform registry was unreachable",
            ));
            state.errors.push(e.to_string());
            state.touch();
            return Ok(());
        }
    };

    let platform_count = platforms.len();
    let normalized = declared.trim().to_ascii_lowercase();
    let matched = platforms
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(&normalized));

    // The fetch succeeded, so the backend is reachable regardless of whether
    // the platform resolves. Record that as the first check of the Platform
    // stage so `backend_reachable` always lives in one place — the gate that
    // opens platform resolution (stage 3) and, transitively, the backend
    // checks (stage 4).
    let backend_reachable = Check::pass(
        "backend_reachable",
        format!("found {platform_count} platforms"),
    );

    let Some(remote) = matched else {
        state.replace_stage(Stage::new(
            StageId::Platform,
            vec![
                backend_reachable,
                Check::fail(
                    "platform_resolved",
                    format!("platform `{declared}` not registered with backend"),
                ),
            ],
        ));
        state.replace_stage(Stage::skipped(StageId::Backend, "platform did not resolve"));
        state.touch();
        return Ok(());
    };

    state.platform.resolved_deploy_branch = Some(remote.deployment_branch.clone());
    let mut platform_checks = vec![
        backend_reachable,
        Check::pass(
            "platform_resolved",
            format!("{} -> {}", remote.name, remote.github_repo),
        ),
    ];

    let on_release_branch = state.target.branch == remote.deployment_branch;
    platform_checks.push(if on_release_branch {
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

    if let Some(user_git) = state.app.git.as_deref() {
        match (
            normalize_github_repo(user_git),
            normalize_github_repo(&remote.github_repo),
        ) {
            (Ok(normalized_user), Ok(normalized_remote))
                if normalized_user == normalized_remote =>
            {
                // Advisory (fork-tolerant): always warn-severity so a reader
                // sees a consistent severity whether it passes or fails.
                platform_checks.push(Check::warn(
                    "git_url_matches_platform",
                    true,
                    remote.github_repo.clone(),
                ));
            }
            (Ok(_), Ok(_)) => {
                platform_checks.push(Check::warn(
                    "git_url_matches_platform",
                    false,
                    format!(
                        "aomi.toml git={user_git} != platform.github_repo={}",
                        remote.github_repo
                    ),
                ));
            }
            (Err(e), _) => {
                platform_checks.push(Check::warn(
                    "git_url_matches_platform",
                    false,
                    format!("aomi.toml git={user_git} is invalid: {e}"),
                ));
            }
            (_, Err(e)) => {
                platform_checks.push(Check::warn(
                    "git_url_matches_platform",
                    false,
                    format!(
                        "platform.github_repo={} is invalid: {e}",
                        remote.github_repo
                    ),
                ));
            }
        }
    }

    state.replace_stage(
        Stage::new(StageId::Platform, platform_checks)
            .with_resolved("name", json!(remote.name))
            .with_resolved("github_repo", json!(remote.github_repo))
            .with_resolved("deployment_branch", json!(remote.deployment_branch)),
    );
    state.recompute_deployed();

    // Stage 4 — backend acceptance. With no declared tags there is nothing for
    // the backend to gate on, so the stage is genuinely skipped rather than
    // vacuously passing.
    if state.target.server_tags.is_empty() {
        state.replace_stage(Stage::skipped(
            StageId::Backend,
            "aomi.toml declares no server_tags",
        ));
        state.touch();
        return Ok(());
    }

    let mut backend_checks = match fetch_server_tags(backend_url).await {
        Ok((server_tags, host_sdk)) => {
            let requested = normalize_tags(state.target.server_tags.clone());
            let matches = requested.iter().all(|tag| server_tags.contains(tag));
            let server_tags_subset = if matches {
                Check::pass(
                    "server_tags_subset",
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
                Check::fail("server_tags_subset", detail)
            };

            // Block a deploy whose pinned aomi-sdk won't match the host — the
            // bundle would otherwise build, push, then fail at activation.
            if let (Some(app), Some(host)) = (state.app_sdk_version.as_deref(), host_sdk.as_deref())
                && app != host
            {
                state
                    .errors
                    .push(format!("aomi-sdk {app} (app) != {host} (backend host)"));
            }
            let sdk_matches_host =
                sdk_compat_check(state.app_sdk_version.as_deref(), host_sdk.as_deref());

            vec![server_tags_subset, sdk_matches_host]
        }
        Err(e) => {
            let detail = format!("GET /api/control/server-tags failed: {e}");
            state.errors.push(detail.clone());
            vec![Check::fail("server_tags_subset", detail)]
        }
    };

    state.replace_stage(Stage::new(
        StageId::Backend,
        std::mem::take(&mut backend_checks),
    ));
    state.touch();
    Ok(())
}

/// Compare the app's declared `aomi-sdk` against the backend host's version.
/// A mismatch is a blocking error (activation enforces an exact match); when
/// either side is unknown, degrade to a non-blocking advisory so an older
/// backend or an unreadable Cargo.toml never falsely blocks a deploy.
fn sdk_compat_check(app_sdk: Option<&str>, host_sdk: Option<&str>) -> Check {
    match (app_sdk, host_sdk) {
        (Some(app), Some(host)) if app == host => Check::pass(
            "sdk_version_matches_host",
            format!("aomi-sdk {app} matches backend host"),
        ),
        (Some(app), Some(host)) => Check::fail(
            "sdk_version_matches_host",
            format!(
                "app pins aomi-sdk {app} but the backend host runs {host}; \
                 rebuild against ={host} (update Cargo.toml and the platform's \
                 required_sdk_version) or activation will reject the bundle"
            ),
        ),
        (Some(_), None) => Check::warn(
            "sdk_version_matches_host",
            true,
            "backend did not report its SDK version; cannot verify aomi-sdk compatibility",
        ),
        (None, _) => Check::warn(
            "sdk_version_matches_host",
            true,
            "could not read the app's aomi-sdk version from Cargo.toml; \
             skipping SDK compatibility check",
        ),
    }
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

    use crate::deployment_state::Severity;

    #[test]
    fn sdk_check_passes_when_versions_match() {
        let c = sdk_compat_check(Some("0.1.21"), Some("0.1.21"));
        assert_eq!(c.name, "sdk_version_matches_host");
        assert!(c.passed);
        assert_eq!(c.severity, Severity::Error);
    }

    #[test]
    fn sdk_check_blocks_on_mismatch() {
        let c = sdk_compat_check(Some("0.1.20"), Some("0.1.21"));
        assert!(!c.passed);
        assert_eq!(c.severity, Severity::Error); // blocking
        assert!(c.detail.unwrap().contains("0.1.21"));
    }

    #[test]
    fn sdk_check_is_advisory_when_either_side_unknown() {
        // Old backend that doesn't report its version.
        let no_host = sdk_compat_check(Some("0.1.21"), None);
        assert!(no_host.passed);
        assert_eq!(no_host.severity, Severity::Warn);
        // App version couldn't be parsed from Cargo.toml.
        let no_app = sdk_compat_check(None, Some("0.1.21"));
        assert!(no_app.passed);
        assert_eq!(no_app.severity, Severity::Warn);
    }
}
