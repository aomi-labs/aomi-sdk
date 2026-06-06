//! `aomi-git status` - local state plus backend registry/runtime state.
//!
//! The GitHub App deploy model keeps GitHub credentials and source/release
//! verification on the backend. The CLI does not call the GitHub API here.

use std::time::Duration;

use anyhow::{Result, anyhow};
use serde::Serialize;

const UA: &str = concat!("aomi-git/", env!("CARGO_PKG_VERSION"));
const PROBE_TIMEOUT: Duration = Duration::from_secs(12);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(6);

pub struct StatusRequest {
    pub app_name: String,
    pub app_release_tag: String,
    pub backend_url: Option<String>,
    pub local: LocalState,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalState {
    pub pushed: bool,
    pub deployed: bool,
    pub activated: bool,
    pub updated_at: i64,
}

#[derive(Debug, Serialize)]
pub struct StatusReport {
    pub app_name: String,
    pub app_release_tag: String,
    pub local: LocalState,
    pub backend: BackendStatus,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum BackendStatus {
    NotChecked,
    NotRegistered {
        backend: String,
    },
    Found {
        backend: String,
        registered: bool,
        is_active: Option<bool>,
        visibility: Option<String>,
        loaded: bool,
    },
    Unknown {
        detail: String,
    },
}

impl StatusReport {
    pub async fn collect(req: StatusRequest) -> Self {
        StatusProbe::new(req).collect().await
    }

    pub fn ready_to_activate(&self) -> bool {
        self.local.deployed && !self.local.activated
    }

    pub fn render(&self) -> String {
        use std::fmt::Write as _;

        let mut out = String::new();
        let _ = writeln!(out, "Deployment status");
        let _ = writeln!(out, "  app              : {}", self.app_name);
        let _ = writeln!(out, "  app_release_tag  : {}", self.app_release_tag);
        let _ = writeln!(
            out,
            "  local state      : pushed={} deployed={} activated={}",
            self.local.pushed, self.local.deployed, self.local.activated
        );

        match &self.backend {
            BackendStatus::NotChecked => {
                let _ = writeln!(out, "  backend          : not checked");
            }
            BackendStatus::NotRegistered { backend } => {
                let _ = writeln!(out, "  backend          : not activated yet on {backend}");
            }
            BackendStatus::Unknown { detail } => {
                let _ = writeln!(out, "  backend          : unknown ({detail})");
            }
            BackendStatus::Found {
                backend,
                registered,
                is_active,
                visibility,
                loaded,
            } => {
                let _ = writeln!(out, "  backend          : {backend}");
                let _ = writeln!(
                    out,
                    "      db row       : registered={registered} active={} visibility={}",
                    Self::bool_label(is_active),
                    visibility.as_deref().unwrap_or("?"),
                );
                let health = if *loaded {
                    "[ok] loaded - serving on this backend"
                } else {
                    "[fail] not loaded"
                };
                let _ = writeln!(out, "      server       : {health}");
            }
        }

        if self.ready_to_activate() {
            let _ = writeln!(out);
            let _ = writeln!(out, "  Release tag is recorded locally. Activate it with:");
            let _ = writeln!(out, "    aomi-git activate");
        }

        out
    }

    fn bool_label(value: &Option<bool>) -> String {
        value
            .map(|b| b.to_string())
            .unwrap_or_else(|| "?".to_string())
    }
}

struct StatusProbe {
    http: reqwest::Client,
    req: StatusRequest,
}

impl StatusProbe {
    fn new(req: StatusRequest) -> Self {
        let http = reqwest::Client::builder()
            .timeout(PROBE_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .user_agent(UA)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { http, req }
    }

    async fn collect(self) -> StatusReport {
        let backend = match &self.req.backend_url {
            Some(url) => self.backend(url).await,
            None => BackendStatus::NotChecked,
        };

        StatusReport {
            app_name: self.req.app_name,
            app_release_tag: self.req.app_release_tag,
            local: self.req.local,
            backend,
        }
    }

    async fn backend(&self, backend_url: &str) -> BackendStatus {
        let base = backend_url.trim().trim_end_matches('/');
        let value: serde_json::Value = match self
            .json(self.http.get(format!("{base}/api/control/apps/status")))
            .await
        {
            Ok(v) => v,
            Err(e) => {
                return BackendStatus::Unknown {
                    detail: e.to_string(),
                };
            }
        };

        let needle = self.req.app_name.trim().to_ascii_lowercase();
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
            registered: app
                .get("registered")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            is_active: app.get("is_active").and_then(|v| v.as_bool()),
            visibility: app
                .get("visibility")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            loaded: app.get("loaded").and_then(|v| v.as_bool()).unwrap_or(false),
        }
    }

    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T> {
        let response = request
            .header(reqwest::header::USER_AGENT, UA)
            .send()
            .await
            .map_err(|e| anyhow!("{e}"))?;
        if !response.status().is_success() {
            return Err(anyhow!("backend returned {}", response.status()));
        }
        response.json::<T>().await.map_err(|e| anyhow!("{e}"))
    }
}
