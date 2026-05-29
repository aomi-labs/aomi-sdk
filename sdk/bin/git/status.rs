//! `aomi-git status` — publication observability for a deployed app.
//!
//! After `aomi-git deploy` pushes source to the platform repo, the contributor
//! has two questions the old "Next steps" block answered with bare URLs:
//!
//!   1. Has CI finished building the release? (the `publish` workflow)
//!   2. Is the release tarball available yet? (i.e. is it activatable)
//!
//! This module answers both by polling the GitHub REST API for the source
//! repo — the workflow run on the publish branch, and the release keyed on the
//! deploy's release tag. No auth is needed for public platform repos; a token
//! (`--access-token`, `$ENV` form supported) is used for private ones.
//!
//! All network failures are non-fatal: a probe that can't reach GitHub renders
//! as `unknown` rather than aborting, so `status` stays useful offline (it can
//! still print the local `.aomi/deployment.json` flags).

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

const GITHUB_API: &str = "https://api.github.com";
const UA: &str = concat!("aomi-git/", env!("CARGO_PKG_VERSION"));

/// Inputs for a status report, pre-resolved by the CLI layer.
pub struct StatusRequest {
    /// `owner/repo` (already normalized).
    pub repo: String,
    /// Release tag this deploy created (`apps-{slug}-{shortcommit}`).
    pub release_tag: String,
    /// Publish branch CI runs on.
    pub branch: String,
    /// Optional GitHub token for private-repo API reads.
    pub github_token: Option<String>,
    /// Local state flags carried straight through from `.aomi/deployment.json`.
    pub local: LocalState,
}

/// Echo of the three `.aomi/deployment.json` state flags.
#[derive(Debug, Clone, Serialize)]
pub struct LocalState {
    pub pushed: bool,
    pub deployed: bool,
    pub activated: bool,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub repo: String,
    pub release_tag: String,
    pub branch: String,
    pub local: LocalState,
    pub ci: CiStatus,
    pub release: ReleaseStatus,
}

/// Rolled-up CI signal derived from the latest workflow run on the branch.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum CiStatus {
    /// No workflow run observed yet for the branch.
    NoRuns,
    /// A run is queued or in progress.
    Running { name: Option<String>, url: String },
    /// The latest run completed successfully.
    Success { name: Option<String>, url: String },
    /// The latest run completed but did not succeed.
    Failed {
        name: Option<String>,
        conclusion: String,
        url: String,
    },
    /// GitHub was unreachable / returned an unexpected response.
    Unknown { detail: String },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ReleaseStatus {
    /// The release tag exists with at least one asset — activatable.
    Available { url: String, assets: usize },
    /// The release tag has no GitHub release yet.
    Pending,
    /// GitHub was unreachable / returned an unexpected response.
    Unknown { detail: String },
}

impl StatusReport {
    /// Produce a status report by polling GitHub. Best-effort: GitHub failures
    /// surface as `Unknown` variants rather than erroring the whole command.
    pub async fn collect(req: StatusRequest) -> Self {
        let client = reqwest::Client::new();
        let ci = fetch_ci(&client, &req).await;
        let release = fetch_release(&client, &req).await;
        StatusReport {
            repo: req.repo,
            release_tag: req.release_tag,
            branch: req.branch,
            local: req.local,
            ci,
            release,
        }
    }

    /// Whether the release is published and ready to activate.
    pub fn ready_to_activate(&self) -> bool {
        matches!(self.release, ReleaseStatus::Available { .. })
    }

    /// Human-readable multi-line summary for a TTY.
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let _ = writeln!(out, "Publication status");
        let _ = writeln!(out, "  repo          : {}", self.repo);
        let _ = writeln!(out, "  release_tag   : {}", self.release_tag);
        let _ = writeln!(out, "  branch        : {}", self.branch);
        let _ = writeln!(
            out,
            "  local state   : pushed={} deployed={} activated={}",
            self.local.pushed, self.local.deployed, self.local.activated
        );

        let _ = write!(out, "  ci            : ");
        match &self.ci {
            CiStatus::NoRuns => {
                let _ = writeln!(out, "no runs yet on `{}` (push may still be propagating)", self.branch);
            }
            CiStatus::Running { name, url } => {
                let _ = writeln!(out, "\u{23f3} running{}", fmt_name(name));
                let _ = writeln!(out, "                  {url}");
            }
            CiStatus::Success { name, url } => {
                let _ = writeln!(out, "\u{2713} green{}", fmt_name(name));
                let _ = writeln!(out, "                  {url}");
            }
            CiStatus::Failed { name, conclusion, url } => {
                let _ = writeln!(out, "\u{2717} {conclusion}{}", fmt_name(name));
                let _ = writeln!(out, "                  {url}");
            }
            CiStatus::Unknown { detail } => {
                let _ = writeln!(out, "unknown ({detail})");
            }
        }

        let _ = write!(out, "  release       : ");
        match &self.release {
            ReleaseStatus::Available { url, assets } => {
                let _ = writeln!(out, "\u{2713} published ({assets} asset(s)) — ready to activate");
                let _ = writeln!(out, "                  {url}");
            }
            ReleaseStatus::Pending => {
                let _ = writeln!(out, "pending (not built yet)");
            }
            ReleaseStatus::Unknown { detail } => {
                let _ = writeln!(out, "unknown ({detail})");
            }
        }

        if self.ready_to_activate() && !self.local.activated {
            let _ = writeln!(out);
            let _ = writeln!(out, "  Release is ready. Request activation from platform ops");
            let _ = writeln!(out, "  (contributors don't hold the activation token).");
        }
        out
    }
}

fn fmt_name(name: &Option<String>) -> String {
    match name {
        Some(n) => format!(" — {n}"),
        None => String::new(),
    }
}

#[derive(Debug, Deserialize)]
struct RunsResponse {
    #[serde(default)]
    workflow_runs: Vec<WorkflowRun>,
}

#[derive(Debug, Deserialize)]
struct WorkflowRun {
    name: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
    html_url: String,
}

async fn fetch_ci(client: &reqwest::Client, req: &StatusRequest) -> CiStatus {
    let url = format!(
        "{GITHUB_API}/repos/{}/actions/runs?branch={}&per_page=10",
        req.repo, req.branch
    );
    let parsed: RunsResponse = match get_json(client, &url, req.github_token.as_deref()).await {
        Ok(v) => v,
        Err(e) => return CiStatus::Unknown { detail: e.to_string() },
    };
    // Runs come newest-first. The first run on the branch is the latest.
    let Some(run) = parsed.workflow_runs.into_iter().next() else {
        return CiStatus::NoRuns;
    };
    match run.status.as_deref() {
        Some("completed") => match run.conclusion.as_deref() {
            Some("success") => CiStatus::Success { name: run.name, url: run.html_url },
            Some(other) => CiStatus::Failed {
                name: run.name,
                conclusion: other.to_string(),
                url: run.html_url,
            },
            None => CiStatus::Unknown {
                detail: "run completed with no conclusion".to_string(),
            },
        },
        // queued | in_progress | waiting | requested | pending | None
        _ => CiStatus::Running { name: run.name, url: run.html_url },
    }
}

#[derive(Debug, Deserialize)]
struct ReleaseInfo {
    html_url: String,
    #[serde(default)]
    assets: Vec<serde_json::Value>,
}

async fn fetch_release(client: &reqwest::Client, req: &StatusRequest) -> ReleaseStatus {
    let url = format!(
        "{GITHUB_API}/repos/{}/releases/tags/{}",
        req.repo, req.release_tag
    );
    let request = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, UA)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json");
    let request = match req.github_token.as_deref() {
        Some(t) if !t.is_empty() => request.bearer_auth(t),
        _ => request,
    };
    let response = match request.send().await {
        Ok(r) => r,
        Err(e) => return ReleaseStatus::Unknown { detail: e.to_string() },
    };
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return ReleaseStatus::Pending;
    }
    if !response.status().is_success() {
        return ReleaseStatus::Unknown {
            detail: format!("GitHub returned {}", response.status()),
        };
    }
    match response.json::<ReleaseInfo>().await {
        Ok(info) => ReleaseStatus::Available {
            url: info.html_url,
            assets: info.assets.len(),
        },
        Err(e) => ReleaseStatus::Unknown { detail: e.to_string() },
    }
}

async fn get_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    token: Option<&str>,
) -> Result<T> {
    let request = client
        .get(url)
        .header(reqwest::header::USER_AGENT, UA)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json");
    let request = match token {
        Some(t) if !t.is_empty() => request.bearer_auth(t),
        _ => request,
    };
    let response = request.send().await.map_err(|e| anyhow!("{e}"))?;
    if !response.status().is_success() {
        return Err(anyhow!("GitHub returned {}", response.status()));
    }
    response.json::<T>().await.map_err(|e| anyhow!("{e}"))
}
