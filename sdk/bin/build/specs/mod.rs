use clap::{Args, ValueEnum};
use eyre::{Context, Result, bail};
use std::path::{Path, PathBuf};

mod apis_guru;
mod github;
mod meta;
mod postman;
mod well_known;

pub use meta::SpecMeta;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum Source {
    /// Try every source in order: well-known → apis-guru → github → postman.
    All,
    /// Well-known paths (e.g. https://api.<platform>.com/openapi.json).
    WellKnown,
    /// APIs.guru curated directory (https://apis.guru).
    ApisGuru,
    /// Search GitHub for openapi.yaml/swagger.json files.
    Github,
    /// Postman public collections (not yet implemented).
    Postman,
}

#[derive(Args, Debug)]
pub struct GenSpecsArgs {
    /// Platform name (e.g. "binance", "dune", "okx").
    pub platform: String,

    /// Where to look for a spec.
    #[arg(long, value_enum, default_value_t = Source::All)]
    pub source: Source,

    /// Output path for the YAML spec. Defaults to apps/<platform>/openapi.yaml
    /// (or ext/specs/<platform>.yaml when --shared is set), resolved against
    /// the workspace root.
    #[arg(long)]
    pub out: Option<PathBuf>,

    /// Overwrite an existing spec at the output path.
    #[arg(long)]
    pub force: bool,

    /// Skip discovery and fetch directly from this URL. Implies --source well-known.
    #[arg(long)]
    pub from_url: Option<String>,

    /// Treat this provider as a shared library (lives under ext/). Default is
    /// app-local — the spec, generated client, and tool layer all live inside
    /// `apps/<platform>/`. Use --shared for big platforms (Binance, Bybit, …)
    /// where the same client is reused by multiple Aomi apps.
    #[arg(long)]
    pub shared: bool,
}

impl GenSpecsArgs {
    pub fn default_spec_path(&self, root: &Path) -> PathBuf {
        if self.shared {
            root.join("ext")
                .join("specs")
                .join(format!("{}.yaml", self.platform))
        } else {
            root.join("apps").join(&self.platform).join("openapi.yaml")
        }
    }
}

/// One candidate spec returned by a discovery source.
#[derive(Debug, Clone)]
pub struct SpecHit {
    /// Raw spec body (either YAML or JSON text).
    pub body: String,
    /// What format `body` is in. The cascade always normalises to YAML on disk.
    pub format: SpecFormat,
    /// Source URL the body was fetched from (provenance).
    pub source_url: String,
    /// Human-friendly source label ("apis-guru", "github", "postman").
    pub source_kind: &'static str,
    /// Optional API version string from upstream metadata.
    pub version: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum SpecFormat {
    Yaml,
    Json,
}

pub fn run(args: GenSpecsArgs) -> Result<()> {
    let out_path = resolve_out_path(&args)?;
    if out_path.exists() && !args.force {
        bail!(
            "spec already exists at {}. Pass --force to overwrite.",
            out_path.display()
        );
    }

    println!("Searching for OpenAPI spec for `{}`...", args.platform);
    let hit = cascade(&args.platform, args.source, args.from_url.as_deref())?;

    let yaml_body = normalise_to_yaml(&hit)?;

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(&out_path, &yaml_body)
        .with_context(|| format!("failed to write {}", out_path.display()))?;

    let meta = SpecMeta::from_hit(&args.platform, &hit);
    let meta_path = out_path.with_extension("meta.json");
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)
        .with_context(|| format!("failed to write {}", meta_path.display()))?;

    println!("✓ wrote {}", out_path.display());
    println!("✓ wrote {}", meta_path.display());
    println!("  source: {} ({})", hit.source_kind, hit.source_url);
    Ok(())
}

fn cascade(platform: &str, source: Source, from_url: Option<&str>) -> Result<SpecHit> {
    let try_well_known = matches!(source, Source::All | Source::WellKnown) || from_url.is_some();
    let try_apis_guru = from_url.is_none() && matches!(source, Source::All | Source::ApisGuru);
    let try_github = from_url.is_none() && matches!(source, Source::All | Source::Github);
    let try_postman = from_url.is_none() && matches!(source, Source::All | Source::Postman);

    if try_well_known {
        match well_known::find(platform, from_url) {
            Ok(Some(hit)) => return Ok(hit),
            Ok(None) => println!("  well-known: no match"),
            Err(e) => println!("  well-known: error: {e:#}"),
        }
    }
    if try_apis_guru {
        match apis_guru::find(platform) {
            Ok(Some(hit)) => return Ok(hit),
            Ok(None) => println!("  apis-guru: no match"),
            Err(e) => println!("  apis-guru: error: {e:#}"),
        }
    }
    if try_github {
        match github::find(platform) {
            Ok(Some(hit)) => return Ok(hit),
            Ok(None) => println!("  github: no match"),
            Err(e) => println!("  github: error: {e:#}"),
        }
    }
    if try_postman {
        match postman::find(platform) {
            Ok(Some(hit)) => return Ok(hit),
            Ok(None) => println!("  postman: no match"),
            Err(e) => println!("  postman: error: {e:#}"),
        }
    }

    bail!(skill_handoff_message(platform));
}

fn skill_handoff_message(platform: &str) -> String {
    format!(
        "no spec found for `{platform}` from any source.\n\n\
         If you know the spec's URL, retry with --from-url:\n\n  \
         aomi-build gen-specs {platform} --from-url https://example.com/openapi.json\n\n\
         Otherwise, draft one from docs via the `aomi-app-client-api-gen` skill in Claude Code:\n\n  \
         /aomi-app-client-api-gen {platform} <docs-url>\n\n\
         The skill will produce ext/specs/{platform}.yaml from the platform's docs."
    )
}

fn normalise_to_yaml(hit: &SpecHit) -> Result<String> {
    match hit.format {
        SpecFormat::Yaml => Ok(hit.body.clone()),
        SpecFormat::Json => {
            let value: serde_json::Value =
                serde_json::from_str(&hit.body).context("upstream JSON spec is invalid")?;
            serde_yaml::to_string(&value).context("failed to serialise spec to YAML")
        }
    }
}

fn resolve_out_path(args: &GenSpecsArgs) -> Result<PathBuf> {
    if let Some(p) = &args.out {
        return Ok(p.clone());
    }
    let root = workspace_root()?;
    Ok(args.default_spec_path(&root))
}

pub(crate) fn workspace_root() -> Result<PathBuf> {
    // Walk up from CWD until we find a Cargo.toml that declares a [workspace].
    let mut cur = std::env::current_dir()?;
    loop {
        let candidate = cur.join("Cargo.toml");
        if candidate.exists() {
            let body = std::fs::read_to_string(&candidate)?;
            if body.contains("[workspace]") {
                return Ok(cur);
            }
        }
        if !cur.pop() {
            bail!(
                "could not locate workspace root from {:?}",
                std::env::current_dir()?
            );
        }
    }
}

#[allow(dead_code)]
pub(crate) fn fetch_text(url: &str) -> Result<String> {
    let client = http_client()?;
    let resp = client
        .get(url)
        .send()
        .with_context(|| format!("GET {url}"))?;
    let status = resp.status();
    let text = resp.text()?;
    if !status.is_success() {
        bail!("GET {url} returned {status}: {text}");
    }
    Ok(text)
}

pub(crate) fn http_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent("aomi-build/0.1")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build HTTP client")
}

pub(crate) fn detect_format(body: &str) -> SpecFormat {
    let trimmed = body.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        SpecFormat::Json
    } else {
        SpecFormat::Yaml
    }
}

pub(crate) fn _path_to_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}
