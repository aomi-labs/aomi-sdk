//! Client layer for the Cambrian DeFi data API (<https://docs.cambrian.org>).
//!
//! Cambrian is a read-only financial-intelligence API covering Base and
//! Ethereum (`/evm/*`) plus Solana (`/solana/*`), with social (`/deep42/*`)
//! and risk (`/risk/*`) surfaces behind the same key. Every request carries
//! `X-API-KEY`; keys are free at <https://console.cambrian.org>.
//!
//! Table endpoints answer in a ClickHouse-style columnar envelope:
//!
//! ```json
//! [{ "columns": [{ "name": "symbol", "type": "String" }, ...],
//!    "data":    [["USDC", 1.0], ...],
//!    "rows":    1 }]
//! ```
//!
//! [`columnar_to_rows`] flattens that into one JSON object per row so the
//! tool layer (and the model) only ever sees `{ "symbol": "USDC", ... }`.
//! Chain and DEX vocabularies live here too so every tool resolves user
//! wording (`base`, `eth`, `sol`, `uniswap`, `aero`, ...) the same way.

use aomi_sdk::*;
use reqwest::header::{ACCEPT, HeaderMap, HeaderName, HeaderValue};
use serde_json::{Map, Value};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct CambrianApp;

/// Production host. Override with `CAMBRIAN_API_URL` for a proxy or mock.
pub(crate) const API_BASE: &str = "https://api.cambrian.org";
const BASE_URL_ENV: &str = "CAMBRIAN_API_URL";
/// Secret slot name declared in `lib.rs` and read at call time.
pub(crate) const API_KEY_NAME: &str = "CAMBRIAN_API_KEY";
const API_KEY_HEADER: &str = "x-api-key";

/// Hard cap on rows any tool hands back to the model.
pub(crate) const MAX_ROWS: u32 = 200;

/// 429 handling: retry this many times, sleeping `backoff * attempt` between
/// tries (0.6s, 1.2s, 1.8s), which comfortably clears the 2 rps free-plan cap.
const RATE_LIMIT_RETRIES: u32 = 3;
const RATE_LIMIT_BACKOFF: Duration = Duration::from_millis(600);

// ============================================================================
// Chain + DEX vocabulary
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Chain {
    Base,
    Ethereum,
    Solana,
}

impl Chain {
    /// Resolve user wording to a chain. `None`/empty defaults to Base, which
    /// is also Cambrian's default `chain_id` (8453).
    pub(crate) fn parse(raw: Option<&str>) -> Result<Self, String> {
        let raw = raw
            .map(|s| s.trim().to_ascii_lowercase())
            .filter(|s| !s.is_empty());
        match raw.as_deref() {
            None | Some("base") | Some("8453") => Ok(Chain::Base),
            Some("ethereum") | Some("eth") | Some("mainnet") | Some("1") => Ok(Chain::Ethereum),
            Some("solana") | Some("sol") | Some("svm") | Some("900") => Ok(Chain::Solana),
            Some(other) => Err(format!(
                "[cambrian] unsupported chain `{other}`; use `base`, `ethereum`, or `solana`"
            )),
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Chain::Base => "base",
            Chain::Ethereum => "ethereum",
            Chain::Solana => "solana",
        }
    }

    pub(crate) fn evm_chain_id(self) -> Option<u64> {
        match self {
            Chain::Base => Some(8453),
            Chain::Ethereum => Some(1),
            Chain::Solana => None,
        }
    }

    /// The numeric `chain_id` query value, or a clear error for Solana.
    pub(crate) fn require_evm(self, what: &str) -> Result<String, String> {
        self.evm_chain_id().map(|id| id.to_string()).ok_or_else(|| {
            format!("[cambrian] {what} is only available on Base or Ethereum, not Solana")
        })
    }

    pub(crate) fn require_solana(self, what: &str) -> Result<(), String> {
        if self == Chain::Solana {
            Ok(())
        } else {
            Err(format!(
                "[cambrian] {what} is only available on Solana; pass chain = \"solana\""
            ))
        }
    }
}

/// EVM DEX families Cambrian indexes. Each maps to a `/evm/<dex>/...` path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvmDex {
    UniswapV3,
    PancakeV3,
    SushiV3,
    AlienBaseV3,
    /// Other Uniswap-V3-style forks Cambrian groups under `/evm/clones`.
    ClonesV3,
    /// Aerodrome classic (v2) pools — the only Aerodrome family with a list endpoint.
    AerodromeV2,
    /// Aerodrome Slipstream (concentrated) pools — single-pool stats only.
    AerodromeV3,
}

impl EvmDex {
    /// Resolve user wording to a DEX family. Defaults to Uniswap V3.
    pub(crate) fn parse(raw: Option<&str>) -> Result<Self, String> {
        let raw = raw
            .map(|s| s.trim().to_ascii_lowercase().replace([' ', '_'], "-"))
            .filter(|s| !s.is_empty());
        match raw.as_deref() {
            None | Some("uniswap") | Some("uniswap-v3") | Some("univ3") | Some("uni") => {
                Ok(EvmDex::UniswapV3)
            }
            Some("pancake") | Some("pancakeswap") | Some("pancake-v3") => Ok(EvmDex::PancakeV3),
            Some("sushi") | Some("sushiswap") | Some("sushi-v3") => Ok(EvmDex::SushiV3),
            Some("alien") | Some("alienbase") | Some("alien-base") => Ok(EvmDex::AlienBaseV3),
            Some("clones") | Some("uniswap-clones") | Some("other") => Ok(EvmDex::ClonesV3),
            Some("aerodrome") | Some("aero") | Some("aerodrome-v2") | Some("aero-v2") => {
                Ok(EvmDex::AerodromeV2)
            }
            Some("aerodrome-v3")
            | Some("aero-v3")
            | Some("slipstream")
            | Some("aerodrome-slipstream") => Ok(EvmDex::AerodromeV3),
            Some(other) => Err(format!(
                "[cambrian] unknown EVM dex `{other}`; use uniswap, pancake, sushi, alienbase, clones, aerodrome (v2), or aerodrome-v3"
            )),
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            EvmDex::UniswapV3 => "uniswap-v3",
            EvmDex::PancakeV3 => "pancake-v3",
            EvmDex::SushiV3 => "sushi-v3",
            EvmDex::AlienBaseV3 => "alienbase-v3",
            EvmDex::ClonesV3 => "uniswap-clones-v3",
            EvmDex::AerodromeV2 => "aerodrome-v2",
            EvmDex::AerodromeV3 => "aerodrome-v3",
        }
    }

    /// Pool-list endpoint, or `None` when the family has no list surface.
    pub(crate) fn pools_path(self) -> Option<&'static str> {
        match self {
            EvmDex::UniswapV3 => Some("/evm/uniswap/v3/pools"),
            EvmDex::PancakeV3 => Some("/evm/pancake/v3/pools"),
            EvmDex::SushiV3 => Some("/evm/sushi/v3/pools"),
            EvmDex::AlienBaseV3 => Some("/evm/alien/v3/pools"),
            EvmDex::ClonesV3 => Some("/evm/clones/v3/pools"),
            EvmDex::AerodromeV2 => Some("/evm/aero/v2/pools"),
            EvmDex::AerodromeV3 => None,
        }
    }

    /// Whether the list endpoint accepts a `token_address` filter server-side.
    pub(crate) fn pools_filter_by_token(self) -> bool {
        !matches!(self, EvmDex::AerodromeV2)
    }

    pub(crate) fn pool_path(self) -> &'static str {
        match self {
            EvmDex::UniswapV3 => "/evm/uniswap/v3/pool",
            EvmDex::PancakeV3 => "/evm/pancake/v3/pool",
            EvmDex::SushiV3 => "/evm/sushi/v3/pool",
            EvmDex::AlienBaseV3 => "/evm/alien/v3/pool",
            EvmDex::ClonesV3 => "/evm/clones/v3/pool",
            EvmDex::AerodromeV2 => "/evm/aero/v2/pool",
            EvmDex::AerodromeV3 => "/evm/aero/v3/pool",
        }
    }
}

/// Solana DEX families with a per-pool stats endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SolanaDex {
    Orca,
    MeteoraDlmm,
    RaydiumClmm,
}

impl SolanaDex {
    pub(crate) fn parse(raw: Option<&str>) -> Result<Self, String> {
        let raw = raw
            .map(|s| s.trim().to_ascii_lowercase().replace([' ', '_'], "-"))
            .filter(|s| !s.is_empty());
        match raw.as_deref() {
            Some("orca") | Some("whirlpool") | Some("orca-whirlpool") => Ok(SolanaDex::Orca),
            Some("meteora") | Some("meteora-dlmm") | Some("dlmm") => Ok(SolanaDex::MeteoraDlmm),
            Some("raydium") | Some("raydium-clmm") | Some("clmm") => Ok(SolanaDex::RaydiumClmm),
            None => Err(
                "[cambrian] Solana pool stats need a dex: orca, meteora-dlmm, or raydium-clmm (see pool_dex from cambrian_find_pools)"
                    .to_string(),
            ),
            Some(other) => Err(format!(
                "[cambrian] unknown Solana dex `{other}`; use orca, meteora-dlmm, or raydium-clmm"
            )),
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            SolanaDex::Orca => "orca",
            SolanaDex::MeteoraDlmm => "meteora-dlmm",
            SolanaDex::RaydiumClmm => "raydium-clmm",
        }
    }

    pub(crate) fn pool_path(self) -> &'static str {
        match self {
            SolanaDex::Orca => "/solana/orca/pool",
            SolanaDex::MeteoraDlmm => "/solana/meteora-dlmm/pool",
            SolanaDex::RaydiumClmm => "/solana/raydium-clmm/pool",
        }
    }
}

// ============================================================================
// HTTP client
// ============================================================================

pub(crate) struct CambrianClient {
    http: reqwest::blocking::Client,
    base_url: String,
}

impl CambrianClient {
    /// Build a client for this call. The key comes from the host secret
    /// vault (`ctx.secrets`) or, for CLI/tests, the `CAMBRIAN_API_KEY` env var.
    pub(crate) fn from_ctx(ctx: &DynToolCallCtx) -> Result<Self, String> {
        let api_key = resolve_secret_value(
            ctx,
            None,
            API_KEY_NAME,
            "[cambrian] missing CAMBRIAN_API_KEY. Get a free key at https://console.cambrian.org and add it to the app secrets.",
        )?;
        let mut key = HeaderValue::from_str(&api_key)
            .map_err(|e| format!("[cambrian] invalid CAMBRIAN_API_KEY value: {e}"))?;
        key.set_sensitive(true);
        let mut headers = HeaderMap::new();
        headers.insert(HeaderName::from_static(API_KEY_HEADER), key);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .map_err(|e| format!("[cambrian] failed to build HTTP client: {e}"))?;

        let base_url = std::env::var(BASE_URL_ENV)
            .ok()
            .map(|s| s.trim().trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| API_BASE.to_string());

        Ok(Self { http, base_url })
    }

    /// `GET {base}{path}?{query}` and decode the JSON body. Non-2xx statuses
    /// become short actionable errors via [`api_error`].
    ///
    /// The free plan enforces 2 requests/second per path, and one agent turn
    /// routinely issues several calls back to back, so a 429 is retried a few
    /// times with a short backoff before it surfaces as an error.
    pub(crate) fn get_raw(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let mut attempt = 0;
        loop {
            let response = self
                .http
                .get(&url)
                .query(query)
                .send()
                .map_err(|e| format!("[cambrian] request to {path} failed: {e}"))?;

            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            if status == 429 && attempt < RATE_LIMIT_RETRIES {
                attempt += 1;
                std::thread::sleep(RATE_LIMIT_BACKOFF * attempt);
                continue;
            }
            if !(200..300).contains(&status) {
                return Err(api_error(path, status, &body));
            }
            return serde_json::from_str(&body)
                .map_err(|e| format!("[cambrian] {path} returned a non-JSON body: {e}"));
        }
    }

    /// `GET` a table endpoint and flatten the columnar envelope into rows.
    pub(crate) fn get_rows(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<Vec<Map<String, Value>>, String> {
        let value = self.get_raw(path, query)?;
        columnar_to_rows(&value).ok_or_else(|| {
            format!(
                "[cambrian] {path} returned an unexpected shape: {}",
                brief(&value.to_string())
            )
        })
    }
}

// ============================================================================
// Response normalization
// ============================================================================

/// Flatten Cambrian's columnar envelope into row objects.
///
/// Accepts `[{columns, data, rows}, ...]`, a bare `{columns, data}` table, or
/// an already-flat array of objects (some Deep42 endpoints). Returns `None`
/// when the shape is unrecognized so callers can surface it instead of
/// silently returning nothing.
pub(crate) fn columnar_to_rows(value: &Value) -> Option<Vec<Map<String, Value>>> {
    fn is_table(v: &Value) -> bool {
        v.get("columns").is_some() && v.get("data").is_some()
    }

    fn table_rows(table: &Value) -> Option<Vec<Map<String, Value>>> {
        let columns = table.get("columns")?.as_array()?;
        let data = table.get("data")?.as_array()?;
        let names: Vec<String> = columns
            .iter()
            .map(|c| {
                c.get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            })
            .collect();
        Some(
            data.iter()
                .filter_map(Value::as_array)
                .map(|row| {
                    names
                        .iter()
                        .zip(row.iter())
                        .map(|(name, cell)| (name.clone(), cell.clone()))
                        .collect()
                })
                .collect(),
        )
    }

    match value {
        Value::Object(_) if is_table(value) => table_rows(value),
        Value::Array(items) if !items.is_empty() && items.iter().all(is_table) => {
            let mut out = Vec::new();
            for table in items {
                out.extend(table_rows(table)?);
            }
            Some(out)
        }
        Value::Array(items) if items.iter().all(Value::is_object) => Some(
            items
                .iter()
                .filter_map(|i| i.as_object().cloned())
                .collect(),
        ),
        _ => None,
    }
}

/// Turn an upstream error body into one short, actionable line.
pub(crate) fn api_error(path: &str, status: u16, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            let err = v.get("error").and_then(Value::as_str).map(str::to_string);
            let msg = v.get("message").and_then(Value::as_str).map(str::to_string);
            match (err, msg) {
                (Some(e), Some(m)) if e != m => Some(format!("{e}: {m}")),
                (_, Some(m)) => Some(m),
                (Some(e), None) => Some(e),
                (None, None) => None,
            }
        })
        .unwrap_or_else(|| brief(body));
    let detail = if detail.is_empty() {
        "no detail".to_string()
    } else {
        detail
    };

    match status {
        401 | 403 => format!(
            "[cambrian] {status} unauthorized on {path}: {detail}. Check CAMBRIAN_API_KEY (free keys at https://console.cambrian.org)."
        ),
        429 => format!(
            "[cambrian] rate limited on {path}: {detail}. The free plan allows 2 requests/second and 1,000 calls/month; wait and retry or upgrade the plan."
        ),
        404 => format!("[cambrian] {path} not found (404): {detail}"),
        400 => format!("[cambrian] bad request on {path}: {detail}"),
        _ => format!("[cambrian] HTTP {status} on {path}: {detail}"),
    }
}

/// Trim a body for error messages so we never echo giant payloads.
pub(crate) fn brief(s: &str) -> String {
    const MAX: usize = 240;
    let s = s.trim();
    if s.chars().count() > MAX {
        let head: String = s.chars().take(MAX).collect();
        format!("{head}…")
    } else {
        s.to_string()
    }
}

// ============================================================================
// Row accessors (Cambrian is inconsistent about `priceUsd` vs `priceUSD`,
// numbers-as-strings for 256-bit ints, and 0/1 vs true/false flags)
// ============================================================================

pub(crate) fn row_str(row: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| match row.get(*k)? {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    })
}

pub(crate) fn row_f64(row: &Map<String, Value>, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|k| match row.get(*k)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

pub(crate) fn row_u64(row: &Map<String, Value>, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|k| match row.get(*k)? {
        Value::Number(n) => n.as_u64().or_else(|| n.as_f64().map(|f| f as u64)),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    })
}

pub(crate) fn row_bool(row: &Map<String, Value>, keys: &[&str]) -> Option<bool> {
    keys.iter().find_map(|k| match row.get(*k)? {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_i64().map(|n| n != 0),
        _ => None,
    })
}

pub(crate) fn row_value(row: &Map<String, Value>, keys: &[&str]) -> Value {
    keys.iter()
        .find_map(|k| row.get(*k).cloned())
        .unwrap_or(Value::Null)
}

/// Split comma-separated and/or repeated entries into trimmed, non-empty
/// addresses. Model inputs arrive both as `["a","b"]` and `["a,b"]`.
pub(crate) fn split_addresses(items: &[String]) -> Vec<String> {
    items
        .iter()
        .flat_map(|s| s.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Clamp a caller-supplied page size into `[1, max]`, defaulting when absent.
pub(crate) fn clamp_limit(limit: Option<u32>, default: u32, max: u32) -> u32 {
    limit.unwrap_or(default).clamp(1, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn columnar_envelope_flattens_to_rows() {
        let body = json!([{
            "columns": [{"name": "symbol", "type": "String"}, {"name": "priceUsd", "type": "Float64"}],
            "data": [["USDC", 1.0], ["WETH", 1873.01]],
            "rows": 2
        }]);
        let rows = columnar_to_rows(&body).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["symbol"], "WETH");
        assert_eq!(row_f64(&rows[1], &["priceUSD", "priceUsd"]), Some(1873.01));
    }

    #[test]
    fn multiple_tables_concatenate() {
        let body = json!([
            {"columns": [{"name": "a"}], "data": [[1]], "rows": 1},
            {"columns": [{"name": "a"}], "data": [[2]], "rows": 1}
        ]);
        let rows = columnar_to_rows(&body).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["a"], 2);
    }

    #[test]
    fn flat_object_arrays_pass_through_and_garbage_is_none() {
        let flat = json!([{"handle": "x", "score": 3}]);
        assert_eq!(columnar_to_rows(&flat).unwrap()[0]["handle"], "x");
        assert!(columnar_to_rows(&json!([])).unwrap().is_empty());
        assert!(columnar_to_rows(&json!("nope")).is_none());
        assert!(columnar_to_rows(&json!([1, 2])).is_none());
    }

    #[test]
    fn chain_aliases_resolve() {
        assert_eq!(Chain::parse(None).unwrap(), Chain::Base);
        assert_eq!(Chain::parse(Some(" ETH ")).unwrap(), Chain::Ethereum);
        assert_eq!(Chain::parse(Some("1")).unwrap(), Chain::Ethereum);
        assert_eq!(Chain::parse(Some("sol")).unwrap(), Chain::Solana);
        assert!(Chain::parse(Some("arbitrum")).is_err());
        assert_eq!(Chain::Base.require_evm("x").unwrap(), "8453");
        assert!(Chain::Solana.require_evm("token search").is_err());
    }

    #[test]
    fn dex_aliases_resolve() {
        assert_eq!(EvmDex::parse(None).unwrap(), EvmDex::UniswapV3);
        assert_eq!(
            EvmDex::parse(Some("Aerodrome")).unwrap(),
            EvmDex::AerodromeV2
        );
        assert_eq!(EvmDex::parse(Some("aero v3")).unwrap(), EvmDex::AerodromeV3);
        assert_eq!(
            EvmDex::parse(Some("PancakeSwap")).unwrap(),
            EvmDex::PancakeV3
        );
        assert!(EvmDex::parse(Some("curve")).is_err());
        assert_eq!(
            SolanaDex::parse(Some("Raydium")).unwrap(),
            SolanaDex::RaydiumClmm
        );
        assert!(SolanaDex::parse(None).is_err());
    }

    #[test]
    fn api_errors_are_short_and_actionable() {
        let e = api_error(
            "/evm/chains",
            401,
            r#"{"error":"Unauthorized","message":"API Key is invalid"}"#,
        );
        assert!(e.contains("Unauthorized: API Key is invalid"), "{e}");
        assert!(e.contains("CAMBRIAN_API_KEY"), "{e}");
        let e = api_error("/evm/chains", 429, "");
        assert!(e.contains("rate limited"), "{e}");
        let e = api_error("/evm/x", 500, &"<html>".repeat(200));
        assert!(e.len() < 400, "{e}");
    }

    #[test]
    fn address_lists_split_and_limits_clamp() {
        let got = split_addresses(&["a, b".to_string(), "".to_string(), "c".to_string()]);
        assert_eq!(got, vec!["a", "b", "c"]);
        assert_eq!(clamp_limit(None, 20, 200), 20);
        assert_eq!(clamp_limit(Some(0), 20, 200), 1);
        assert_eq!(clamp_limit(Some(9999), 20, 200), 200);
    }

    #[test]
    fn row_accessors_tolerate_upstream_variants() {
        let row: Map<String, Value> = serde_json::from_value(json!({
            "priceUSD": "12.5", "isStable": 1, "borrowable": false, "liq": "991726704338392184"
        }))
        .unwrap();
        assert_eq!(row_f64(&row, &["priceUsd", "priceUSD"]), Some(12.5));
        assert_eq!(row_bool(&row, &["isStable"]), Some(true));
        assert_eq!(row_bool(&row, &["borrowable"]), Some(false));
        assert_eq!(row_u64(&row, &["liq"]), Some(991726704338392184));
        assert!(row_str(&row, &["missing"]).is_none());
    }
}
