//! Marinade Finance HTTP client (mainnet-beta).
//!
//! Marinade's public-facing HTTP API at `api.marinade.finance` is **stats-only**:
//! it exposes APY, TVL, exchange rate, validator scores, and similar
//! observability data. There is no tx-build endpoint analogous to Jupiter's
//! `/swap` or byreal's `/router-service/swap`; on-chain action (stake, unstake,
//! claim) is composed client-side against the Marinade Anchor program.
//!
//! That asymmetry shapes this app:
//!
//! - The [`stats`] client is real — every read tool hits the live API.
//! - The write tools ([`crate::tool::writes`]) build instructions in Rust
//!   against pinned Marinade program constants (program id, mSOL mint,
//!   discriminators). The ix construction today is a structural scaffold
//!   pending integration with a typed Marinade SDK / IDL decoder; see
//!   `HANDOFF.md` for the production-readiness gap.

pub(crate) mod stats;

use std::sync::OnceLock;
use std::time::Duration;

/// Public Marinade stats API. Override via `MARINADE_API_URL` for
/// self-hosted mirrors or tests.
pub(crate) const MARINADE_API_BASE: &str = "https://api.marinade.finance";

const HTTP_TIMEOUT: Duration = Duration::from_secs(15);

static HTTP: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();

pub(crate) fn http_client() -> Result<reqwest::blocking::Client, String> {
    HTTP.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent("aomi-marinade/0.1")
            .build()
            .map_err(|e| format!("[marinade] http client init: {e}"))
    })
    .clone()
}

/// Marker app type — `dyn_aomi_app!` requires `Clone + Default + Send + Sync`.
#[derive(Debug, Clone, Default)]
pub(crate) struct MarinadeApp;

/// Strip Marinade's envelope (if any) and surface the inner data with a
/// `"source": "marinade"` tag for downstream attribution.
pub(crate) fn marinade_get<T: serde::de::DeserializeOwned + serde::Serialize>(
    path: &str,
) -> Result<serde_json::Value, String> {
    let base = std::env::var("MARINADE_API_URL")
        .unwrap_or_else(|_| MARINADE_API_BASE.to_string());
    let url = format!("{base}{path}");
    let client = http_client()?;
    let resp = client
        .get(&url)
        .send()
        .map_err(|e| format!("[marinade] GET {url}: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .map_err(|e| format!("[marinade] read body {url}: {e}"))?;
    if !status.is_success() {
        return Err(format!("[marinade] {url} returned {status}: {text}"));
    }
    let value: T = serde_json::from_str(&text)
        .map_err(|e| format!("[marinade] parse {url}: {e}; body: {text}"))?;
    let mut tagged = serde_json::to_value(value)
        .map_err(|e| format!("[marinade] re-serialize {url}: {e}"))?;
    if let serde_json::Value::Object(ref mut map) = tagged {
        map.insert(
            "source".to_string(),
            serde_json::Value::String("marinade".to_string()),
        );
    }
    Ok(tagged)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marinade_api_base_is_https() {
        assert!(MARINADE_API_BASE.starts_with("https://"));
    }
}
