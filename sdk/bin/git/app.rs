use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::git::GitRepo;

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct App {
    /// Validated app identifier (slug). TOML keys `name` or `slug` map here.
    #[serde(alias = "slug")]
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    /// Platform tier label — must match a row in the backend `platforms` table.
    #[serde(default)]
    pub platform: Option<String>,
    /// GitHub URL for the platform's publish repo.
    #[serde(default)]
    pub git: Option<String>,
    /// User-chosen deployment branch in the platform repo.
    #[serde(default)]
    pub branch: Option<String>,
    /// Visibility intent — replaces the per-call `--visibility` flag.
    #[serde(default)]
    pub public: Option<bool>,
    /// Env-var reference for the GitHub token the backend will use to fetch
    /// this release. MUST be the literal form `"$ENV_VAR_NAME"` — bare
    /// secrets are rejected at parse time so committed configs cannot leak
    /// tokens. The CLI resolves `std::env::var(ENV_VAR_NAME)` at deploy/
    /// activate time and forwards the value in the request body. The token
    /// is never written to deployment.json. Omit for public-release
    /// platforms (community).
    #[serde(default)]
    pub access_token: Option<String>,
    /// Required server tags for activation/load targeting. The backend loads
    /// only when these tags are a subset of its configured AOMI_SERVER_TAGS.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server_tags: Vec<String>,
    #[serde(default)]
    pub config_path: PathBuf,
    #[serde(default)]
    pub source_path: PathBuf,
}

impl App {
    /// Resolve `[app].access_token` to a real token string by reading the
    /// referenced env var. Returns:
    /// - `Ok(None)` when no `access_token` is configured (public release).
    /// - `Ok(Some(token))` when the env var is set to a non-empty value.
    /// - `Err` when the env var name is set but the env var is unset or empty.
    pub fn resolved_access_token(&self) -> Result<Option<String>> {
        let Some(reference) = self
            .access_token
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            return Ok(None);
        };
        let var_name = reference
            .strip_prefix('$')
            .ok_or_else(|| {
                anyhow!(
                    "aomi.toml [app].access_token must be of the form `$ENV_VAR_NAME`, got `{reference}`"
                )
            })?;
        match std::env::var(var_name) {
            Ok(value) if !value.is_empty() => Ok(Some(value)),
            Ok(_) => bail!("env var `{var_name}` (from aomi.toml access_token) is empty"),
            Err(_) => bail!("env var `{var_name}` (from aomi.toml access_token) is not set"),
        }
    }
}

impl App {
    pub fn discover(repo: &GitRepo) -> Result<Self> {
        let mut dir = repo.start_dir().to_path_buf();

        loop {
            for candidate in [
                dir.join("aomi.toml"),
                dir.join(".aomi").join("app.toml"),
                dir.join("app.toml"),
            ] {
                if candidate.is_file() {
                    return Self::from_aomi_toml(&candidate, repo.root());
                }
            }

            let cargo = dir.join("Cargo.toml");
            if cargo.is_file()
                && let Some(app) = Self::from_cargo_toml(&cargo, repo.root())?
            {
                return Ok(app);
            }

            if dir == repo.root() || !dir.pop() {
                break;
            }
        }

        bail!(
            "could not discover app config from {}; expected aomi.toml, .aomi/app.toml, app.toml, or a Cargo.toml with [package].name",
            repo.start_dir().display()
        );
    }

    fn from_aomi_toml(path: &Path, git_root: &Path) -> Result<Self> {
        let raw: AomiConfigFile = toml::from_str(
            &std::fs::read_to_string(path)
                .with_context(|| format!("failed to read app config {}", path.display()))?,
        )
        .with_context(|| format!("failed to parse app config {}", path.display()))?;

        let mut app = raw.app;
        if app.name.trim().is_empty() {
            bail!("{} must define `name` under [app]", path.display());
        }
        app.name = normalize_slug(&app.name, path)?;
        if app.display_name.trim().is_empty() {
            app.display_name = humanize_slug(&app.name);
        } else {
            app.display_name = app.display_name.trim().to_string();
        }
        // Trim string optionals; empty → None.
        app.platform = trim_opt(app.platform);
        app.git = trim_opt(app.git);
        app.branch = trim_opt(app.branch);
        app.access_token = trim_opt(app.access_token);
        app.server_tags = normalize_tags(app.server_tags, "server_tags", path)?;

        // Reject access_token values that look like raw secrets (no `$`
        // prefix). This protects users from accidentally committing a
        // plaintext token to a file they're checking in.
        if let Some(reference) = app.access_token.as_deref()
            && !reference.starts_with('$')
        {
            bail!(
                "{}: [app].access_token must be `$ENV_VAR_NAME` (env-var reference). Literal tokens are rejected so committed configs cannot leak secrets.",
                path.display()
            );
        }

        app.fill_paths(path, git_root)?;
        Ok(app)
    }

    fn from_cargo_toml(path: &Path, git_root: &Path) -> Result<Option<Self>> {
        let manifest: CargoManifest = toml::from_str(
            &std::fs::read_to_string(path)
                .with_context(|| format!("failed to read Cargo manifest {}", path.display()))?,
        )
        .with_context(|| format!("failed to parse Cargo manifest {}", path.display()))?;
        let Some(package) = manifest.package else {
            return Ok(None);
        };

        let name = normalize_slug(&package.name, path)?;
        let display_name = package
            .description
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| humanize_slug(&name));

        let mut app = App {
            name,
            display_name,
            ..App::default()
        };
        app.fill_paths(path, git_root)?;
        Ok(Some(app))
    }

    fn fill_paths(&mut self, config_path: &Path, git_root: &Path) -> Result<()> {
        let source_dir = source_dir_for_config(config_path)?;
        self.source_path = relative_to(source_dir, git_root)?;
        self.config_path = relative_to(config_path, git_root)?;
        Ok(())
    }
}

#[derive(Debug, Default, Deserialize)]
struct AomiConfigFile {
    #[serde(default)]
    app: App,
}

#[derive(Debug, Deserialize)]
struct CargoManifest {
    package: Option<CargoPackage>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
    #[serde(default)]
    description: Option<String>,
}

fn relative_to(path: &Path, git_root: &Path) -> Result<PathBuf> {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let git_root = git_root
        .canonicalize()
        .unwrap_or_else(|_| git_root.to_path_buf());
    let relative = path
        .strip_prefix(&git_root)
        .with_context(|| format!("{} is not under {}", path.display(), git_root.display()))?;
    Ok(if relative.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        relative.to_path_buf()
    })
}

fn source_dir_for_config(config_path: &Path) -> Result<&Path> {
    let parent = config_path
        .parent()
        .ok_or_else(|| anyhow!("app config has no parent: {}", config_path.display()))?;
    if config_path.file_name().and_then(|name| name.to_str()) == Some("app.toml")
        && parent.file_name().and_then(|name| name.to_str()) == Some(".aomi")
    {
        return parent
            .parent()
            .ok_or_else(|| anyhow!("app config has no app root: {}", config_path.display()));
    }
    Ok(parent)
}

fn trim_opt(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn normalize_tags(values: Vec<String>, field: &str, source: &Path) -> Result<Vec<String>> {
    let mut tags = Vec::new();
    for raw in values {
        let tag = raw.trim().to_ascii_lowercase();
        if tag.is_empty() {
            continue;
        }
        if !tag
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        {
            bail!(
                "{} defines invalid {field} tag `{}`; use ASCII letters, numbers, '-' or '_'",
                source.display(),
                raw
            );
        }
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    Ok(tags)
}

fn normalize_slug(raw: &str, source: &Path) -> Result<String> {
    let slug = raw.trim().to_string();
    if slug.is_empty() {
        bail!("{} defines an empty app name", source.display());
    }
    if !slug
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        bail!(
            "{} defines invalid app name `{}`; use ASCII letters, numbers, '-' or '_'",
            source.display(),
            slug
        );
    }
    Ok(slug)
}

fn humanize_slug(slug: &str) -> String {
    slug.split(['-', '_'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
