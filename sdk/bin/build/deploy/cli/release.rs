//! Gating activation on a deployment's release build.
//!
//! Two transports reach the same status: the Builder session (what a human
//! deploy uses) and a privileged activation token (admin/CI, which additionally
//! watches the platform PR's GitHub checks). They differ only in how they poll,
//! so the terminal-state handling lives in one place.

use std::io::{IsTerminal, Write as _};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Result, bail};

use crate::deploy::build_client::BuildClient;
use crate::deploy::flow::{self, DeployReady};
use crate::deploy::platform::Platform;
use crate::deploy::state::LocalDeployment;

const RELEASE_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Wait for the release build using the Builder session.
pub(crate) async fn wait_via_build(
    client: &BuildClient,
    platform: &Platform,
    deployment: &LocalDeployment,
    label: &str,
    timeout_message: String,
) -> Result<()> {
    let progress = Progress::start(label, pr_url(deployment));
    let outcome = flow::poll_build_deployment_ready(
        client,
        platform.as_str(),
        &deployment.deployment.id,
        RELEASE_TIMEOUT,
        |state| progress.update(state),
    )
    .await?;
    progress.finish(&outcome);
    settle(outcome, deployment, timeout_message)
}

/// Wait for the release build using a privileged activation token.
pub(crate) async fn wait_via_backend(
    backend_url: &str,
    token: &str,
    platform: &Platform,
    deployment: &LocalDeployment,
    label: &str,
    timeout_message: String,
) -> Result<()> {
    let progress = Progress::start(label, pr_url(deployment));
    let outcome = flow::poll_deployment_ready_with_pr(
        backend_url,
        token,
        platform.as_str(),
        &deployment.deployment.id,
        pr_url(deployment),
        RELEASE_TIMEOUT,
        |state| progress.update(state),
    )
    .await?;
    progress.finish(&outcome);
    settle(outcome, deployment, timeout_message)
}

/// The long wait, made visible: on a TTY a spinner line redrawn in place with
/// the current build state and elapsed time; otherwise one line per state
/// change. Either way the PR link stays on screen for the whole wait — it is
/// the thing worth clicking while CI runs.
struct Progress {
    label: String,
    state: Arc<Mutex<String>>,
    started: Instant,
    ticker: Option<tokio::task::JoinHandle<()>>,
    interactive: bool,
}

impl Progress {
    fn start(label: &str, pr: Option<&str>) -> Self {
        let interactive = std::io::stdout().is_terminal();
        println!("{label}   waiting for the release build (up to 30 min, Ctrl-C to stop)");
        if let Some(pr) = pr {
            println!("        watching {pr}");
        }
        let state = Arc::new(Mutex::new(String::from("pending")));
        let started = Instant::now();
        let ticker = interactive.then(|| {
            let state = Arc::clone(&state);
            tokio::spawn(async move {
                const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                for frame in (0..FRAMES.len()).cycle() {
                    print!(
                        "\r\x1b[2K        {} {} · {}",
                        FRAMES[frame],
                        state.lock().expect("progress state poisoned"),
                        fmt_elapsed(started.elapsed())
                    );
                    let _ = std::io::stdout().flush();
                    tokio::time::sleep(Duration::from_millis(120)).await;
                }
            })
        });
        Self {
            label: label.to_string(),
            state,
            started,
            ticker,
            interactive,
        }
    }

    fn update(&self, state: &str) {
        if self.interactive {
            *self.state.lock().expect("progress state poisoned") = state.to_string();
        } else {
            println!("        build: {state}");
        }
    }

    fn finish(mut self, outcome: &DeployReady) {
        self.stop();
        let elapsed = fmt_elapsed(self.started.elapsed());
        match outcome {
            DeployReady::Ready => println!("{}   ✓ ready in {elapsed}", self.label),
            DeployReady::Failed(_) => println!("{}   ✗ failed after {elapsed}", self.label),
            DeployReady::NoCi(_) => println!("{}   ✗ no CI ran ({elapsed})", self.label),
            DeployReady::TimedOut => {
                println!("{}   ✗ still building after {elapsed}", self.label)
            }
        }
    }

    fn stop(&mut self) {
        if let Some(ticker) = self.ticker.take() {
            ticker.abort();
            print!("\r\x1b[2K");
            let _ = std::io::stdout().flush();
        }
    }
}

/// Polling can error out mid-wait; never leave a detached ticker drawing over
/// the error report.
impl Drop for Progress {
    fn drop(&mut self) {
        self.stop();
    }
}

fn fmt_elapsed(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m{:02}s", secs / 60, secs % 60)
    }
}

fn pr_url(deployment: &LocalDeployment) -> Option<&str> {
    deployment.deployment.platform.pr_url.as_deref()
}

fn settle(
    outcome: DeployReady,
    deployment: &LocalDeployment,
    timeout_message: String,
) -> Result<()> {
    match outcome {
        DeployReady::Ready => Ok(()),
        DeployReady::Failed(msg) => bail!("deployment failed before activation: {msg}"),
        DeployReady::NoCi(msg) => bail!("{}", no_ci_message(&msg, deployment)),
        DeployReady::TimedOut => bail!("{timeout_message}"),
    }
}

/// `no_ci` is not a build failure — nothing broke, nothing ran. Point at the
/// deploy PR, since a missing or unpicked-up release workflow on the platform
/// repo is the usual cause.
pub(crate) fn no_ci_message(detail: &str, deployment: &LocalDeployment) -> String {
    format!(
        "no CI ran for this deployment, so there is no release to activate: {detail}\n\
         Check that the platform repo's deploy PR has a release workflow: {}",
        pr_url(deployment).unwrap_or("(no PR URL recorded)")
    )
}
