//! The authenticated context a Builder command runs in.
//!
//! Resolving the backend URL, deriving the Build URL from it, and proving the
//! saved credential still works are three steps that always happen together and
//! always produce the same answer within one invocation. They used to be three
//! loose locals re-derived at every call site, which is why a single wizard
//! deploy re-authenticated four times and printed "Logged in as" four times.
//!
//! Here they are one value with one constructor. Opening a session is
//! memoized for the process, so later steps reuse the first one.

use std::sync::Mutex;

use anyhow::{Result, anyhow};

use super::build_client::BuildClient;
use super::cli::login;
use super::cli::shared::{BUILD_URL_ENV, resolve_backend, resolve_build_url};
use super::types::CliStatusResult;

/// A resolved Build environment plus the identity we authenticated as.
#[derive(Clone, Debug)]
pub struct Session {
    backend_url: Option<String>,
    build_url: String,
    pub client: BuildClient,
    pub identity: CliStatusResult,
}

/// Memoized per process. Keyed on the Build URL so the wizard switching
/// environments mid-loop re-authenticates instead of reusing the wrong one.
static OPEN: Mutex<Option<Session>> = Mutex::new(None);

impl Session {
    /// Resolve the Build environment from `--backend` / `--build-url` (falling
    /// back to env and saved config) and authenticate.
    pub async fn open(backend: &Option<String>, build_url: &Option<String>) -> Result<Self> {
        let backend_url = resolve_backend(backend);
        let build_url = resolve_build_url(build_url, backend_url.as_deref())
            .ok_or_else(|| anyhow!("this command needs --build-url or {BUILD_URL_ENV}"))?;
        Self::at(&build_url, backend_url).await
    }

    /// Authenticate against an already-resolved Build URL — the wizard prompts
    /// for the backend itself, so it has one before a session exists.
    pub async fn at(build_url: &str, backend_url: Option<String>) -> Result<Self> {
        Self::at_with_options(build_url, backend_url, false).await
    }

    pub async fn at_with_options(
        build_url: &str,
        backend_url: Option<String>,
        no_browser: bool,
    ) -> Result<Self> {
        if let Some(open) = Self::cached(build_url) {
            return Ok(open);
        }
        let authenticated = login::authenticate_with_options(build_url, no_browser).await?;
        let session = Self {
            backend_url,
            build_url: build_url.to_string(),
            client: authenticated.client,
            identity: authenticated.identity,
        };
        // Announce the operating context once per process, at the point we
        // actually establish it: which binary, which environment, who. This is
        // the only line that tells a user mid-deploy whether they're pointed at
        // staging or production.
        println!(
            "✓ aomi-build {} · {} · @{}\n",
            aomi_sdk::AOMI_SDK_VERSION,
            session.env_label(),
            session.identity.github_login
        );
        *OPEN.lock().expect("session cache poisoned") = Some(session.clone());
        Ok(session)
    }

    fn cached(build_url: &str) -> Option<Self> {
        OPEN.lock()
            .expect("session cache poisoned")
            .as_ref()
            .filter(|open| open.build_url == build_url)
            .cloned()
    }

    /// The backend this session's Build environment fronts, when one was
    /// resolved. `None` is normal: most Builder commands never call it.
    pub fn backend_url(&self) -> Option<&str> {
        self.backend_url.as_deref()
    }

    pub fn build_url(&self) -> &str {
        &self.build_url
    }

    /// Human name for the environment this session points at: `staging` /
    /// `production` for the known Build URLs (with the backend host when one is
    /// resolved), the raw URL otherwise.
    pub fn env_label(&self) -> String {
        let label = match self.build_url.as_str() {
            "https://build-staging.aomi.dev" => "staging",
            "https://build.aomi.dev" => "production",
            other => other,
        };
        match self.backend_url.as_deref() {
            Some(backend) => format!(
                "{label} ({})",
                backend
                    .trim_start_matches("https://")
                    .trim_start_matches("http://")
            ),
            None => label.to_string(),
        }
    }
}
