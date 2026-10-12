//! The deployment attempt: Manager's GitHub-owned run that validates, builds,
//! publishes, activates and verifies a Project's deployment. Aomi Build starts
//! it for the portal's Deploy button; the CLI starts and follows the same one,
//! so a human deploy cannot diverge between the two surfaces.

use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use super::build_client::BuildClient;
use super::types::{Attempt, AttemptJob};

/// Consecutive read failures tolerated before surfacing the error. GitHub can
/// lag a few seconds behind the dispatch that returned the run id.
const MAX_READ_FAILURES: u32 = 15;
const POLL_INTERVAL: Duration = Duration::from_secs(6);

/// Jobs that must succeed for an attempt to count as live. Names are the
/// platform workflow contract (`deploy-project.yml`).
const ACTIVATE_JOB: &str = "Activate";
const VERIFY_JOB: &str = "Verify runtime";

#[derive(Debug, Eq, PartialEq)]
pub enum AttemptOutcome {
    /// Built, published, activated, and the runtime was verified.
    Live,
    Failed(String),
    TimedOut,
}

/// Follow an attempt to a terminal state, reporting stage changes to `on_state`.
pub async fn follow(
    client: &BuildClient,
    project_id: i64,
    run_id: u64,
    timeout: Duration,
    on_state: impl FnMut(&str),
) -> Result<AttemptOutcome> {
    follow_every(client, project_id, run_id, timeout, POLL_INTERVAL, on_state).await
}

pub(crate) async fn follow_every(
    client: &BuildClient,
    project_id: i64,
    run_id: u64,
    timeout: Duration,
    interval: Duration,
    mut on_state: impl FnMut(&str),
) -> Result<AttemptOutcome> {
    let started = Instant::now();
    let mut last_state: Option<String> = None;
    let mut failures: u32 = 0;
    loop {
        match client.attempt(project_id, run_id).await {
            Ok(result) => {
                failures = 0;
                let attempt = result.attempt;
                let state = stage(&attempt);
                if last_state.as_deref() != Some(state.as_str()) {
                    on_state(&state);
                    last_state = Some(state);
                }
                if attempt.status == "completed" {
                    return Ok(match verdict(&attempt) {
                        Ok(()) => AttemptOutcome::Live,
                        Err(detail) => AttemptOutcome::Failed(detail),
                    });
                }
            }
            Err(error) => {
                failures += 1;
                if failures >= MAX_READ_FAILURES {
                    return Err(error.context("deployment attempt polling failed repeatedly"));
                }
                on_state(&format!(
                    "status unavailable, retrying ({failures}/{MAX_READ_FAILURES}): {error}"
                ));
                last_state = None;
            }
        }
        if started.elapsed() >= timeout {
            return Ok(AttemptOutcome::TimedOut);
        }
        tokio::time::sleep(interval).await;
    }
}

/// The stage a human cares about: the job running now, or the run's status.
pub(crate) fn stage(attempt: &Attempt) -> String {
    if attempt.status == "completed" {
        return match attempt.conclusion.as_deref() {
            Some("success") => "completed".to_string(),
            other => format!("completed ({})", other.unwrap_or("no conclusion")),
        };
    }
    attempt
        .jobs
        .iter()
        .find(|job| job.status.as_deref() == Some("in_progress"))
        .map(|job| job.name.clone())
        .unwrap_or_else(|| attempt.status.clone())
}

/// `Ok` only when the run succeeded *and* activation and runtime verification
/// both ran. A green run that skipped them is not a live deployment.
pub(crate) fn verdict(attempt: &Attempt) -> std::result::Result<(), String> {
    if attempt.conclusion.as_deref() != Some("success") {
        return Err(failure_detail(attempt));
    }
    for required in [ACTIVATE_JOB, VERIFY_JOB] {
        let succeeded = attempt
            .jobs
            .iter()
            .any(|job| job.name.starts_with(required) && is(job, "success"));
        if !succeeded {
            return Err(format!(
                "the deployment attempt finished without a successful `{required}` job, so \
                 the release is not confirmed live{}",
                link(attempt)
            ));
        }
    }
    Ok(())
}

/// Name the failed stage and step, then the bounded diagnostics, then the run.
pub(crate) fn failure_detail(attempt: &Attempt) -> String {
    let mut detail = match attempt.conclusion.as_deref() {
        Some(conclusion) => format!("deployment attempt {conclusion}"),
        None => "deployment attempt ended without a conclusion".to_string(),
    };
    for job in attempt.jobs.iter().filter(|job| is_failed(job)) {
        detail.push_str(&format!("\n  stage `{}` {}", job.name, outcome(job)));
        if let Some(step) = job.steps.iter().find(|step| {
            matches!(
                step.conclusion.as_deref(),
                Some("failure" | "cancelled" | "timed_out")
            )
        }) {
            detail.push_str(&format!(" at step `{}`", step.name));
        }
    }
    for line in attempt.diagnostics.iter().take(8) {
        detail.push_str(&format!("\n  {line}"));
    }
    detail.push_str(&link(attempt));
    detail
}

fn is(job: &AttemptJob, conclusion: &str) -> bool {
    job.conclusion.as_deref() == Some(conclusion)
}

fn is_failed(job: &AttemptJob) -> bool {
    matches!(
        job.conclusion.as_deref(),
        Some("failure" | "cancelled" | "timed_out")
    )
}

fn outcome(job: &AttemptJob) -> &str {
    job.conclusion.as_deref().unwrap_or("did not finish")
}

fn link(attempt: &Attempt) -> String {
    attempt
        .url
        .as_deref()
        .map(|url| format!("\nRun: {url}"))
        .unwrap_or_default()
}

/// Pick the attempt to follow after a start request. A start can answer with an
/// attempt that was already running; following it is right only when it builds
/// the commit this deploy is shipping.
pub(crate) fn adopt(started: &Attempt, existing: bool, commit: &str) -> Result<()> {
    let builds = started
        .commit
        .as_deref()
        .is_some_and(|built| !built.is_empty() && (built == commit || commit.starts_with(built)));
    if builds {
        return Ok(());
    }
    let built = started.commit.as_deref().unwrap_or("an unknown commit");
    if existing {
        bail!(
            "another deployment attempt is already running for this Project (building {built}){}\n\
             Wait for it to finish, or cancel it in Aomi Build, then deploy again.",
            link(started)
        );
    }
    bail!(
        "the deployment attempt is building {built}, not the commit being deployed ({commit}){}",
        link(started)
    );
}
