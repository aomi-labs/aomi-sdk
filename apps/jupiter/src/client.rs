//! Client layer for the Jupiter swap aggregator (Solana best-execution).
//!
//! Jupiter's swap API is refreshingly flat — no envelope wrapper. Two calls:
//!   * `GET  /swap/v1/quote`  → the route (flat JSON; the whole object is the
//!     `quoteResponse` you hand back to /swap).
//!   * `POST /swap/v1/swap`   → `{ swapTransaction: <base64 v0 tx>, … }`, a
//!     fully-built VersionedTransaction the wallet signs as-is at commit.
//!
//! Errors come back as an HTTP 4xx with `{ "error": "...", "errorCode": "..." }`
//! (or a 5xx). [`decode`] surfaces both as `Err(String)` so a failed quote can
//! never masquerade as a route.
//!
//! Base URL defaults to the free `lite-api.jup.ag` tier; set `JUPITER_API_URL`
//! to point at the keyed `api.jup.ag` host (add the key via a reverse proxy —
//! this app carries no secret).

use serde_json::{Value, json};
use std::sync::OnceLock;
use std::time::Duration;

#[derive(Clone, Default)]
pub struct JupiterApp;

/// Free Jupiter tier. `api.jup.ag` (keyed) is a drop-in via `JUPITER_API_URL`.
pub(crate) const JUPITER_API_BASE: &str = "https://lite-api.jup.ag";

const PATH_QUOTE: &str = "/swap/v1/quote";
const PATH_SWAP: &str = "/swap/v1/swap";

pub(crate) struct JupiterClient {
    http: reqwest::blocking::Client,
    base_url: String,
}

static CLIENT: OnceLock<Result<JupiterClient, String>> = OnceLock::new();

pub(crate) fn jupiter_client() -> Result<&'static JupiterClient, String> {
    CLIENT
        .get_or_init(JupiterClient::new)
        .as_ref()
        .map_err(|e| e.clone())
}

impl JupiterClient {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            http: reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| format!("[jupiter] failed to build HTTP client: {e}"))?,
            base_url: std::env::var("JUPITER_API_URL")
                .unwrap_or_else(|_| JUPITER_API_BASE.to_string()),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    /// `GET /swap/v1/quote` — the best route for `amount` of `input_mint` into
    /// `output_mint`. Returns the flat quote object (feed it verbatim to
    /// [`swap`](Self::swap)).
    pub(crate) fn quote(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: &str,
        slippage_bps: u32,
        swap_mode: &str,
    ) -> Result<Value, String> {
        let url = self.url(PATH_QUOTE);
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("inputMint", input_mint),
                ("outputMint", output_mint),
                ("amount", amount),
                ("slippageBps", &slippage_bps.to_string()),
                ("swapMode", swap_mode),
            ])
            .send()
            .map_err(|e| format!("[jupiter] GET {url} failed: {e}"))?;
        decode(resp, &url)
    }

    /// `POST /swap/v1/swap` — exchange a quote for the unsigned swap tx. The
    /// `quote` is the whole object [`quote`](Self::quote) returned; `user` is
    /// the base58 payer. `wrap_and_unwrap_sol` lets Jupiter add WSOL wrap/unwrap
    /// ixs when SOL is one side.
    pub(crate) fn swap(
        &self,
        quote: &Value,
        user: &str,
        wrap_and_unwrap_sol: bool,
    ) -> Result<Value, String> {
        let url = self.url(PATH_SWAP);
        let body = json!({
            "quoteResponse": quote,
            "userPublicKey": user,
            "wrapAndUnwrapSol": wrap_and_unwrap_sol,
        });
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("[jupiter] POST {url} failed: {e}"))?;
        decode(resp, &url)
    }
}

/// Decode a Jupiter response body — flat JSON with no envelope. Surfaces a
/// non-2xx, or a 2xx carrying an `error` field, as `Err`.
fn decode(resp: reqwest::blocking::Response, url: &str) -> Result<Value, String> {
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("[jupiter] {url} returned HTTP {status}: {}", brief(&text)));
    }
    decode_body(&text).map_err(|e| format!("[jupiter] {url}: {e}"))
}

/// Parse + error-check a Jupiter body. Split out so it's unit-testable without a
/// live response.
fn decode_body(text: &str) -> Result<Value, String> {
    let value: Value =
        serde_json::from_str(text).map_err(|e| format!("decode failed: {e}; body: {}", brief(text)))?;
    if let Some(err) = value.get("error").and_then(Value::as_str) {
        let code = value
            .get("errorCode")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return Err(format!("Jupiter error [{code}]: {err}"));
    }
    Ok(value)
}

/// Trim a body for error messages — Jupiter payloads can be large; never dump
/// the whole thing into an LLM-facing error.
fn brief(text: &str) -> String {
    let t = text.trim();
    if t.len() > 300 {
        format!("{}…", &t[..300])
    } else {
        t.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_quote_body_passes_through() {
        let body = r#"{"inAmount":"10000000","outAmount":"126375201","priceImpactPct":"0.0001","routePlan":[]}"#;
        let v = decode_body(body).unwrap();
        assert_eq!(v["outAmount"], "126375201");
    }

    #[test]
    fn error_field_on_2xx_is_an_err() {
        let body = r#"{"error":"Could not find any route","errorCode":"COULD_NOT_FIND_ANY_ROUTE"}"#;
        let err = decode_body(body).unwrap_err();
        assert!(err.contains("COULD_NOT_FIND_ANY_ROUTE"), "{err}");
        assert!(err.contains("Could not find any route"), "{err}");
    }

    #[test]
    fn swap_body_carries_the_tx_blob() {
        let body = r#"{"swapTransaction":"AQABBASE64BLOB","lastValidBlockHeight":410021003}"#;
        let v = decode_body(body).unwrap();
        assert_eq!(v["swapTransaction"], "AQABBASE64BLOB");
    }

    #[test]
    fn brief_truncates_giant_bodies() {
        let big = "x".repeat(1000);
        assert!(brief(&big).len() < 320);
        assert_eq!(brief("small"), "small");
    }

    /// Live proof against Jupiter that the full quote → swap contract holds: a
    /// USDC→SOL quote returns a positive `outAmount`, and swapping it yields a
    /// `swapTransaction` blob (the Lane-2 input). Network-gated (`--ignored`).
    /// The SOL mint stands in as `userPublicKey` — the swap builder doesn't
    /// check balances, and no signature/broadcast happens.
    #[test]
    #[ignore = "network: hits lite-api.jup.ag"]
    fn live_quote_and_swap_contract() {
        const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        const SOL: &str = "So11111111111111111111111111111111111111112";
        let client = JupiterClient::new().expect("client builds");

        let quote = client
            .quote(USDC, SOL, "10000000", 50, "ExactIn")
            .expect("quote decodes");
        let out: f64 = quote["outAmount"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .expect("quote has a numeric outAmount");
        assert!(out > 0.0, "expected a positive SOL out amount: {quote}");

        let swap = client.swap(&quote, SOL, true).expect("swap decodes");
        assert!(
            swap["swapTransaction"].as_str().is_some_and(|s| !s.is_empty()),
            "swap must carry a swapTransaction blob: {swap}"
        );
    }
}
