//! Wizard branch that deploys a repo: pick the source, deploy, wait for the
//! release build, activate.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, bail};
use inquire::{Confirm, Text};

use super::print_step_error;
use crate::deploy::cli::{ActivateArgs, DeployStepArgs, release, shared};
use crate::deploy::platform::{Platform, normalize_github_repo};
use crate::deploy::session::Session;
use crate::deploy::state::LocalDeployment;

pub(super) async fn existing_dir_flow(session: &Session, platform: &str) -> Result<()> {
    let dir = Text::new("Path to the app's source repo:")
        .with_default(".")
        .prompt()
        .context("wizard cancelled")?;
    let dir = PathBuf::from(dir.trim());

    let repo = match git_origin_slug(&dir) {
        Some(slug) => Text::new("Source repo (owner/name):")
            .with_default(&slug)
            .prompt()
            .context("wizard cancelled")?,
        None => Text::new("Source repo (owner/name):")
            .prompt()
            .context("wizard cancelled")?,
    };
    let repo = normalize_github_repo(repo.trim())?;

    deploy_then_activate(session, platform, &dir, &repo).await
}

/// Scaffold-from-example (bootstrap flow): open GitHub's "Use this template"
/// page so the user creates their own repo from the example, clone it, then run
/// the same resolve-and-deploy path as a local repo. Uses only the aomi-build
/// App — the repo is created by GitHub's template UI, not a backend call.
pub(super) async fn scaffold_flow(session: &Session, platform: &str) -> Result<()> {
    const GENERATE_URL: &str = "https://github.com/aomi-labs/playground-example/generate";
    println!("Create your repo from the example template:");
    println!("  {GENERATE_URL}");
    if let Err(e) = open::that(GENERATE_URL) {
        eprintln!("  (couldn't open a browser automatically: {e})");
        println!("  open the URL above.");
    } else {
        println!("  (opened in your browser)");
    }
    println!("  Name it whatever you like, create it in your account, then come back.\n");

    // Ask for the repo link and clone exactly that — preserving case (GitHub
    // owners are case-sensitive in the URL) and not reconstructing it. A pasted
    // URL is used verbatim; a bare `owner/name` becomes an https URL.
    let (clone_url, slug) = loop {
        let entered = Text::new("The repo you just created (link):")
            .prompt()
            .context("wizard cancelled")?;
        let entered = entered.trim();
        match normalize_github_repo(entered) {
            Ok(slug) => {
                let clone_url = if entered.contains("://") || entered.starts_with("git@") {
                    entered.trim_end_matches('/').to_string()
                } else {
                    format!("https://github.com/{}", entered.trim_end_matches('/'))
                };
                break (clone_url, slug);
            }
            Err(_) => {
                println!("  paste the repo link, e.g. https://github.com/you/your-repo")
            }
        }
    };

    // The CLI deploys from a local checkout (it derives the source ref from
    // git), so clone the new repo, then run the same path as a local repo.
    // Clone to an absolute path so the target never depends on a relative cwd.
    let name = slug.rsplit('/').next().unwrap_or("app").to_string();
    let cwd = std::env::current_dir().map_err(|_| {
        anyhow!(
            "this shell's working directory no longer exists (it was deleted or \
             recreated, so git can't run here). Open a fresh terminal — or `cd` into a \
             directory that exists — then re-run `aomi-build`."
        )
    })?;
    let target = cwd.join(&name);
    if target.exists() {
        bail!(
            "`{}` already exists — remove it or clone elsewhere, then pick \
             \"Deploy the app in a local directory\"",
            target.display()
        );
    }
    println!("Cloning {clone_url} → {} …", target.display());
    clone_repo(&clone_url, &target)?;

    deploy_then_activate(session, platform, &target, &slug).await
}

/// `git clone <repo_link> <dir>`. The example repo is public, so no auth.
fn clone_repo(repo_link: &str, dir: &Path) -> Result<()> {
    let status = Command::new("git")
        .args(["clone", repo_link])
        .arg(dir)
        .status()
        .context("failed to run git clone")?;
    if !status.success() {
        bail!(
            "git clone {repo_link} failed — if the repo is private make it public \
             (or clone it yourself); if it was just created, wait a moment and retry."
        );
    }
    Ok(())
}

async fn deploy_then_activate(
    session: &Session,
    platform: &str,
    dir: &Path,
    repo: &str,
) -> Result<()> {
    // Show what would ship and settle the SDK pin *before* asking for a yes —
    // a confirm is only meaningful when the facts are on screen, and an SDK
    // repin is a file rewrite the user should approve, not discover.
    let (git_root, _) = shared::git_context(dir)?;
    preview_source(&git_root, repo);
    ensure_sdk_interactive(session, &git_root).await?;

    let go = Confirm::new(&format!("Deploy `{platform}` from {}?", dir.display()))
        .with_default(true)
        .prompt()
        .context("wizard cancelled")?;
    if !go {
        println!("Stopped before deploy. Re-run `aomi-build` anytime.");
        return Ok(());
    }

    // Deploy with inline retry — a transient backend failure prints and offers
    // a retry instead of unwinding the whole wizard.
    loop {
        let result = DeployStepArgs {
            repo: Some(repo.to_string()),
            backend: session.backend_url().map(str::to_string),
            build_url: Some(session.build_url().to_string()),
            path: dir.to_path_buf(),
            ..Default::default()
        }
        .run_deploy_command()
        .await;
        match result {
            Ok(()) => break,
            Err(e) => {
                print_step_error(&e);
                if !retry("Retry the deploy?")? {
                    return Ok(());
                }
            }
        }
    }

    let wait = Confirm::new("Wait for CI to build the release, then activate?")
        .with_default(true)
        .prompt()
        .context("wizard cancelled")?;
    if !wait {
        // Omit --target-tag: the backend defaults to the deployment's own
        // server_tags, matching the portal (which sends no target_tags).
        println!(
            "When CI is green:\n  aomi-build activate --path {}",
            dir.display()
        );
        return Ok(());
    }

    // Gate activation on the release build, like the portal: poll the
    // deployment's status until it's `ready` before promoting. The deploy step
    // recorded the id in `.aomi/deployment.json`. A read failure here used to be
    // swallowed, so the wizard skipped the wait it had just promised and went
    // straight to an activation that could not succeed either — the same file is
    // what `activate` reads its release tags from.
    let deployment = local_deployment(dir)?;
    release::wait_via_build(
        &session.client,
        &Platform::new(platform),
        &deployment,
        "Release build",
        format!(
            "still building after 30 minutes. Activate once CI is green:\n  \
             aomi-build activate --path {}",
            dir.display()
        ),
    )
    .await?;
    println!();

    loop {
        let result = ActivateArgs {
            platform: Some(Platform::new(platform)),
            backend: session.backend_url().map(str::to_string),
            build_url: Some(session.build_url().to_string()),
            path: dir.to_path_buf(),
            ..Default::default()
        }
        .run()
        .await;
        match result {
            Ok(()) => return Ok(()),
            Err(e) => {
                print_step_error(&e);
                if !retry("Retry activation?")? {
                    return Ok(());
                }
            }
        }
    }
}

/// One line of git facts about what a deploy from this repo would ship.
fn preview_source(git_root: &Path, repo: &str) {
    let Ok(commit) = shared::head_commit(git_root) else {
        return;
    };
    let mut line = format!("{repo} @ {}", &commit[..commit.len().min(7)]);
    if let Some(branch) = shared::head_branch(git_root) {
        line.push_str(&format!(" · {branch}"));
    }
    match shared::commit_on_remote(git_root, &commit) {
        Some(true) => line.push_str(" · pushed ✓"),
        Some(false) => line.push_str(" · pushed ✗"),
        None => {}
    }
    match shared::worktree_dirty(git_root) {
        Some(false) => line.push_str(" · clean ✓"),
        Some(true) => line.push_str(" · uncommitted changes !"),
        None => {}
    }
    println!("  Source   {line}");
}

/// Check the app's aomi-sdk pin against the backend and, on mismatch, ask
/// before rewriting Cargo.toml/Cargo.lock. The deploy step re-verifies (and
/// refuses to ship a commit that lacks the repin), so saying no here just ends
/// this deploy attempt.
async fn ensure_sdk_interactive(session: &Session, git_root: &Path) -> Result<()> {
    let required =
        crate::sdk_guard::resolve_required_sdk_version(session.backend_url(), None).await?;
    let report = crate::sdk_guard::check_project_sdk(git_root, &required)?;
    if report.ok {
        println!("  SDK      aomi-sdk ={required} · matches backend ✓");
        return Ok(());
    }
    println!("  SDK      ✗ this backend requires aomi-sdk ={required}:");
    for message in report.blocking_messages() {
        println!("             {message}");
    }
    let repin = Confirm::new(&format!(
        "Repin Cargo.toml/Cargo.lock to aomi-sdk ={required} now?"
    ))
    .with_default(true)
    .prompt()
    .context("wizard cancelled")?;
    if !repin {
        bail!("fix the aomi-sdk pin first (aomi-build sdk fix), then deploy again");
    }
    let fixed = crate::sdk_guard::fix_project_sdk(git_root, &required)?;
    if !fixed.ok {
        bail!(
            "SDK repin did not produce a compatible manifest:\n{}",
            fixed.blocking_messages().join("\n")
        );
    }
    println!(
        "  ✓ repinned to aomi-sdk ={required} — commit and push this change; \
         the deploy ships your pushed commit, not the working tree"
    );
    Ok(())
}

fn local_deployment(dir: &Path) -> Result<LocalDeployment> {
    let (git_root, _) = crate::deploy::cli::shared::git_context(dir)?;
    LocalDeployment::read(&git_root)?.ok_or_else(|| {
        anyhow!(
            "the deploy did not leave a .aomi/deployment.json at {} — \
             without it there is no deployment id to wait on and no release tag to \
             activate. Re-run the deploy, or activate manually once CI is green.",
            git_root.display()
        )
    })
}

/// Small yes/no retry prompt; a cancel (Ctrl-C) propagates out to exit.
fn retry(question: &str) -> Result<bool> {
    Confirm::new(question)
        .with_default(true)
        .prompt()
        .context("wizard cancelled")
}

/// Best-effort `owner/name` from the repo's `origin` remote.
fn git_origin_slug(dir: &PathBuf) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8(out.stdout).ok()?;
    normalize_github_repo(url.trim()).ok()
}
