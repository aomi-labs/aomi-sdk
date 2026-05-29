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
    /// App slug — used to find the app's row in the backend registry.
    pub app_name: String,
    /// `owner/repo` (already normalized).
    pub repo: String,
    /// Release tag this deploy created (`apps-{slug}-{shortcommit}`).
    pub release_tag: String,
    /// Publish branch CI runs on.
    pub branch: String,
    /// Optional GitHub token for private-repo API reads.
    pub github_token: Option<String>,
    /// Backend base URL. When present and CI has finished, status also reports
    /// the app's backend registry row + runtime health. None ⇒ skip the
    /// backend probe entirely.
    pub backend_url: Option<String>,
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
    pub backend: BackendStatus,
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

impl CiStatus {
    /// CI reached a terminal state (won't change without a re-push). Used to
    /// decide whether it's worth probing the backend for the activated row.
    fn is_terminal(&self) -> bool {
        matches!(self, CiStatus::Success { .. } | CiStatus::Failed { .. })
    }
}

/// The app's state on the backend: its `applications` registry row (DB) plus
/// whether the runtime has the plugin loaded (server health). Sourced from the
/// unauthenticated `GET /api/control/apps/status` endpoint.
#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BackendStatus {
    /// Not probed — no backend URL was resolvable, or CI hasn't finished yet.
    NotChecked,
    /// Backend reachable, but no registry row for this app — not activated yet.
    NotRegistered { backend: String },
    /// App found. Mirrors the DB row + runtime-loaded flag.
    Found {
        backend: String,
        /// Has an `applications` row.
        registered: bool,
        /// DB row `is_active`.
        is_active: Option<bool>,
        /// `public` / `private`, derived from the DB row.
        visibility: Option<String>,
        /// Runtime has the plugin loaded and serving (server health).
        loaded: bool,
    },
    /// Backend unreachable / returned an unexpected response.
    Unknown { detail: String },
}

impl StatusReport {
    /// Produce a status report by polling GitHub (and, once CI is done, the
    /// backend). Best-effort: any network failure surfaces as an `Unknown`
    /// variant rather than erroring the whole command.
    pub async fn collect(req: StatusRequest) -> Self {
        let client = reqwest::Client::new();
        let ci = fetch_ci(&client, &req).await;
        let release = fetch_release(&client, &req).await;

        // Only probe the backend once CI has finished — before that the app
        // can't be activated, so a registry lookup would only ever say "not
        // registered". `ci.is_terminal()` covers a green build whose release
        // job is still uploading; `release` Available is the clearest signal.
        let ci_done = ci.is_terminal() || matches!(release, ReleaseStatus::Available { .. });
        let backend = match (&req.backend_url, ci_done) {
            (Some(url), true) => fetch_backend(&client, url, &req.app_name).await,
            _ => BackendStatus::NotChecked,
        };

        StatusReport {
            repo: req.repo,
            release_tag: req.release_tag,
            branch: req.branch,
            local: req.local,
            ci,
            release,
            backend,
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

        match &self.backend {
            BackendStatus::NotChecked => {}
            BackendStatus::NotRegistered { backend } => {
                let _ = writeln!(out, "  backend       : not activated yet on {backend}");
            }
            BackendStatus::Unknown { detail } => {
                let _ = writeln!(out, "  backend       : unknown ({detail})");
            }
            BackendStatus::Found {
                backend,
                registered,
                is_active,
                visibility,
                loaded,
            } => {
                let _ = writeln!(out, "  backend       : {backend}");
                let _ = writeln!(out, "      db row    : registered={registered} active={} visibility={}",
                    opt_bool(is_active),
                    visibility.as_deref().unwrap_or("?"),
                );
                let health = if *loaded {
                    "\u{2713} loaded — serving on this backend"
                } else {
                    "\u{2717} not loaded (registered but runtime hasn't picked it up)"
                };
                let _ = writeln!(out, "      server    : {health}");
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

fn opt_bool(value: &Option<bool>) -> String {
    match value {
        Some(b) => b.to_string(),
        None => "?".to_string(),
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

/// Probe `GET {backend}/api/control/apps/status` (unauthenticated) and pull out
/// this app's registry row + runtime-loaded flag. The endpoint returns every
/// app; we match on the (lowercased) slug.
async fn fetch_backend(
    client: &reqwest::Client,
    backend_url: &str,
    app_name: &str,
) -> BackendStatus {
    let base = backend_url.trim().trim_end_matches('/');
    let url = format!("{base}/api/control/apps/status");
    let value: serde_json::Value = match get_json(client, &url, None).await {
        Ok(v) => v,
        Err(e) => return BackendStatus::Unknown { detail: e.to_string() },
    };

    let needle = app_name.trim().to_ascii_lowercase();
    let app = value
        .get("apps")
        .and_then(|a| a.as_array())
        .and_then(|apps| {
            apps.iter().find(|row| {
                row.get("name")
                    .and_then(|n| n.as_str())
                    .map(|n| n.eq_ignore_ascii_case(&needle))
                    .unwrap_or(false)
            })
        });

    let Some(app) = app else {
        return BackendStatus::NotRegistered {
            backend: base.to_string(),
        };
    };

    BackendStatus::Found {
        backend: base.to_string(),
        registered: app.get("registered").and_then(|v| v.as_bool()).unwrap_or(false),
        is_active: app.get("is_active").and_then(|v| v.as_bool()),
        visibility: app
            .get("visibility")
            .and_then(|v| v.as_str())
            .map(str::to_string),
        loaded: app.get("loaded").and_then(|v| v.as_bool()).unwrap_or(false),
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
