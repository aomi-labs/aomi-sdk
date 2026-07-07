use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct AomiAppFiles {
    /// Validated app identifier (`aomi.toml [app].name`).
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    /// Platform tier label — must match a row in the backend `platforms` table.
    #[serde(default)]
    pub platform: Option<String>,
    /// Legacy deployment-state field. New aomi.toml files should not set it.
    #[serde(default)]
    pub git: Option<String>,
    /// Visibility intent — replaces the per-call `--visibility` flag.
    #[serde(default)]
    pub public: Option<bool>,
    /// Required server tags for activation/load targeting. The backend loads
    /// only when these tags are a subset of its configured AOMI_SERVER_TAGS.
    /// When `aomi.toml` omits this field (or sets it to `[]`), `App::discover`
    /// fills in `[DEFAULT_SERVER_TAG]` so unconfigured deploys land on staging
    /// rather than every server class. The defaulting is recorded on
    /// `server_tags_defaulted` and surfaced as a check in deployment.json.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server_tags: Vec<String>,
    /// `true` when `server_tags` was filled in by the implicit default rather
    /// than the user. Not serialized — the marker is only useful at plan time.
    #[serde(skip)]
    pub server_tags_defaulted: bool,
    #[serde(default)]
    pub config_path: PathBuf,
    #[serde(default)]
    pub source_path: PathBuf,
}

/// Default `server_tags` injected when `aomi.toml` does not declare any.
/// Chosen to fail safe: unconfigured deploys reach the staging class only.
pub const DEFAULT_SERVER_TAG: &str = "staging";

impl AomiAppFiles {
    pub fn discover(start_dir: &Path, git_root: &Path) -> Result<Self> {
        let mut dir = start_dir.to_path_buf();

        loop {
            for candidate in [
                dir.join("aomi.toml"),
                dir.join(".aomi").join("app.toml"),
                dir.join("app.toml"),
            ] {
                if candidate.is_file() {
                    return Self::from_aomi_toml(&candidate, git_root);
                }
            }

            let cargo = dir.join("Cargo.toml");
            if cargo.is_file()
                && let Some(app) = Self::from_cargo_toml(&cargo, git_root)?
            {
                return Ok(app);
            }

            if dir == git_root || !dir.pop() {
                break;
            }
        }

        bail!(
            "could not discover app config from {}; expected aomi.toml, .aomi/app.toml, app.toml, or a Cargo.toml with [package].name",
            start_dir.display()
        );
    }

    pub(crate) fn from_aomi_toml(path: &Path, git_root: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read app config {}", path.display()))?;
        let file: toml::Table = toml::from_str(&content)
            .with_context(|| format!("failed to parse app config {}", path.display()))?;
        let raw: AomiAppConfig = file
            .get("app")
            .ok_or_else(|| anyhow!("{} must define an [app] table", path.display()))?
            .clone()
            .try_into()
            .with_context(|| format!("failed to parse [app] in {}", path.display()))?;

        let source_subdir = raw.source_subdir.clone();
        let mut app = raw.into_app();
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
        app.server_tags = normalize_tags(app.server_tags, "server_tags", path)?;
        if app.server_tags.is_empty() {
            app.server_tags = vec![DEFAULT_SERVER_TAG.to_string()];
            app.server_tags_defaulted = true;
        }

        app.fill_paths(path, git_root, source_subdir.as_deref())?;
        Ok(app)
    }

    fn from_cargo_toml(path: &Path, git_root: &Path) -> Result<Option<Self>> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read Cargo manifest {}", path.display()))?;
        let file: toml::Table = toml::from_str(&content)
            .with_context(|| format!("failed to parse Cargo manifest {}", path.display()))?;
        let Some(package_value) = file.get("package") else {
            return Ok(None);
        };
        let package: CargoPackage = package_value
            .clone()
            .try_into()
            .with_context(|| format!("failed to parse [package] in {}", path.display()))?;

        let name = normalize_slug(&package.name, path)?;
        let display_name = package
            .description
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| humanize_slug(&name));

        let mut app = AomiAppFiles {
            name,
            display_name,
            server_tags: vec![DEFAULT_SERVER_TAG.to_string()],
            server_tags_defaulted: true,
            ..AomiAppFiles::default()
        };
        app.fill_paths(path, git_root, None)?;
        Ok(Some(app))
    }

    fn fill_paths(
        &mut self,
        config_path: &Path,
        git_root: &Path,
        source_subdir: Option<&str>,
    ) -> Result<()> {
        let source_dir = match source_subdir
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(".") | None => source_dir_for_config(config_path)?.to_path_buf(),
            Some(subdir) => git_root.join(subdir),
        };
        self.source_path = relative_to(&source_dir, git_root)?;
        self.config_path = relative_to(config_path, git_root)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct AomiAppConfig {
    /// Validated app identifier (`aomi.toml [app].name`).
    pub name: String,
    #[serde(default)]
    pub display_name: String,
    /// Platform tier label — must match a row in the backend `platforms` table.
    #[serde(default)]
    pub platform: Option<String>,
    /// Visibility intent — replaces the per-call `--visibility` flag.
    #[serde(default)]
    pub public: Option<bool>,
    /// Required server tags for activation/load targeting.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub server_tags: Vec<String>,
    /// Optional path inside the source repository. Defaults to the directory
    /// containing `aomi.toml`.
    #[serde(default)]
    pub source_subdir: Option<String>,
}

impl AomiAppConfig {
    fn into_app(self) -> AomiAppFiles {
        AomiAppFiles {
            name: self.name,
            display_name: self.display_name,
            platform: self.platform,
            public: self.public,
            server_tags: self.server_tags,
            ..AomiAppFiles::default()
        }
    }
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
