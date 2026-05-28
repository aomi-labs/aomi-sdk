use clap::{Args, ValueEnum};
use eyre::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::specs::workspace_root;

#[derive(Args, Debug)]
pub struct TestSchemaArgs {
    /// Platform name (looks for ext/specs/<platform>.yaml).
    pub platform: String,

    /// Override the spec path. Defaults to ext/specs/<platform>.yaml.
    #[arg(long)]
    pub spec: Option<PathBuf>,

    /// Override the base URL. Defaults to spec's first `servers` entry.
    #[arg(long)]
    pub base_url: Option<String>,

    /// Add an HTTP header (e.g. `--header X-API-Key:abc`). Repeatable.
    #[arg(long = "header", short = 'H')]
    pub headers: Vec<String>,

    /// Inject `Authorization: Bearer $<ENV>` from this environment variable.
    #[arg(long)]
    pub bearer_env: Option<String>,

    /// Inject an API-key header. Pair with --api-key-header to choose the name
    /// (default `X-API-Key`). The value comes from the named env var.
    #[arg(long)]
    pub api_key_env: Option<String>,

    /// Header name for --api-key-env. Default `X-API-Key`.
    #[arg(long, default_value = "X-API-Key")]
    pub api_key_header: String,

    /// Comma-separated checks to run. Defaults to a fast subset; use `all` to enable everything.
    #[arg(
        long,
        default_value = "not_a_server_error,status_code_conformance,content_type_conformance,response_schema_conformance"
    )]
    pub checks: String,

    /// Cap on Hypothesis examples per operation. Default 25 keeps smoke tests fast.
    #[arg(long, default_value_t = 25)]
    pub max_examples: u32,

    /// Allow fuzzing of write endpoints (POST/PUT/PATCH/DELETE). Default is
    /// GET-only because random fuzzing of write endpoints against a live API
    /// can place orders, mutate state, etc. Opt in only when the target is a
    /// sandbox/testnet.
    #[arg(long)]
    pub write: bool,

    /// Which schemathesis runner to use. Auto-detect by default.
    #[arg(long, value_enum, default_value_t = Runner::Auto)]
    pub runner: Runner,

    /// Extra args passed verbatim to schemathesis after `--`.
    #[arg(last = true)]
    pub extra: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum Runner {
    /// Try `schemathesis` on PATH, then `uvx schemathesis`.
    Auto,
    /// `schemathesis run ...` (must be on PATH).
    Schemathesis,
    /// `uvx schemathesis run ...` (uv installs on demand).
    Uvx,
}

pub fn run(args: TestSchemaArgs) -> Result<()> {
    let root = workspace_root()?;
    let spec_path = args.spec.clone().unwrap_or_else(|| {
        root.join("ext")
            .join("specs")
            .join(format!("{}.yaml", args.platform))
    });
    if !spec_path.exists() {
        bail!("spec not found at {}", spec_path.display());
    }

    // schemathesis 4.x requires --url for file-based specs even when the spec
    // declares servers. So we always pass a base URL — but warn if the spec
    // has per-op overrides, since those won't be honored.
    let multi_server = has_multi_server_overrides(&spec_path).unwrap_or(false);
    let base_url = match args.base_url.clone() {
        Some(u) => u,
        None => extract_first_server_url(&spec_path)?,
    };

    let runner = pick_runner(args.runner)?;
    println!("Validating {} against {}", spec_path.display(), base_url);
    println!("  runner: {runner:?}");
    println!("  checks: {}", args.checks);
    if !args.write {
        println!("  methods: GET only (pass --write to fuzz POST/PUT/PATCH/DELETE)");
    } else {
        println!("  methods: ALL (write fuzzing enabled)");
    }
    if multi_server {
        println!(
            "  ⚠ spec declares per-operation `servers:` overrides — schemathesis 4.x can't honor those.\n    \
             Operations whose true host differs from {base_url} will fail.\n    \
             Run separately per host with --base-url, or accept partial coverage."
        );
    }

    let mut cmd = build_command(runner, &spec_path, Some(&base_url), &args)?;
    let status = cmd
        .status()
        .with_context(|| format!("failed to spawn {runner:?}"))?;

    if !status.success() {
        bail!(
            "schemathesis reported {} (exit {})",
            if status.code() == Some(1) {
                "violations"
            } else {
                "an error"
            },
            status.code().unwrap_or(-1)
        );
    }
    println!("✓ schema conforms");
    Ok(())
}

fn pick_runner(requested: Runner) -> Result<Runner> {
    if requested != Runner::Auto {
        return Ok(requested);
    }
    if which("schemathesis") {
        return Ok(Runner::Schemathesis);
    }
    if which("uvx") {
        return Ok(Runner::Uvx);
    }
    bail!(
        "schemathesis not found. Install with:\n  \
         pipx install schemathesis    (recommended — isolated venv)\n  \
         pip install schemathesis     (any Python env)\n  \
         brew install uv              (then `uvx schemathesis ...` works)"
    );
}

fn which(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn build_command(
    runner: Runner,
    spec_path: &Path,
    base_url: Option<&str>,
    args: &TestSchemaArgs,
) -> Result<Command> {
    let mut cmd = match runner {
        Runner::Schemathesis => {
            let mut c = Command::new("schemathesis");
            c.arg("run").arg(spec_path);
            c
        }
        Runner::Uvx => {
            let mut c = Command::new("uvx");
            c.arg("schemathesis").arg("run").arg(spec_path);
            c
        }
        Runner::Auto => unreachable!("auto resolved earlier"),
    };

    if let Some(u) = base_url {
        cmd.arg("--url").arg(u);
    }
    cmd.arg("--checks").arg(&args.checks);
    cmd.arg("--max-examples").arg(args.max_examples.to_string());

    if !args.write {
        cmd.arg("--include-method").arg("GET");
    }

    for h in &args.headers {
        cmd.arg("-H").arg(h);
    }
    if let Some(env) = &args.bearer_env {
        let token =
            std::env::var(env).with_context(|| format!("--bearer-env: ${env} is not set"))?;
        cmd.arg("-H").arg(format!("Authorization: Bearer {token}"));
    }
    if let Some(env) = &args.api_key_env {
        let key =
            std::env::var(env).with_context(|| format!("--api-key-env: ${env} is not set"))?;
        cmd.arg("-H").arg(format!("{}: {key}", args.api_key_header));
    }

    for extra in &args.extra {
        cmd.arg(extra);
    }
    Ok(cmd)
}

/// True iff any path or operation in the spec declares its own `servers:` block.
fn has_multi_server_overrides(spec_path: &Path) -> Result<bool> {
    let text = std::fs::read_to_string(spec_path)?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&text)?;
    let Some(paths) = doc.get("paths").and_then(|p| p.as_mapping()) else {
        return Ok(false);
    };
    for (_path_key, item) in paths {
        let Some(item_map) = item.as_mapping() else {
            continue;
        };
        if item_map.contains_key("servers") {
            return Ok(true);
        }
        for (op_key, op) in item_map {
            let Some(op_str) = op_key.as_str() else {
                continue;
            };
            if !matches!(
                op_str,
                "get" | "post" | "put" | "delete" | "patch" | "head" | "options" | "trace"
            ) {
                continue;
            }
            if let Some(op_map) = op.as_mapping() {
                if op_map.contains_key("servers") {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn extract_first_server_url(spec_path: &Path) -> Result<String> {
    let text = std::fs::read_to_string(spec_path)
        .with_context(|| format!("failed to read {}", spec_path.display()))?;
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).context("spec is not valid YAML")?;
    let url = doc
        .get("servers")
        .and_then(|s| s.as_sequence())
        .and_then(|s| s.first())
        .and_then(|s| s.get("url"))
        .and_then(|u| u.as_str())
        .ok_or_else(|| eyre::eyre!("spec has no `servers[0].url` — pass --base-url"))?
        .to_string();
    Ok(url)
}
