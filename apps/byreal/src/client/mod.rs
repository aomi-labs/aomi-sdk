//! Client layer for the byreal app.
//!
//! Three concrete clients live here, one per byreal product line:
//!   * [`perps`]  — Hyperliquid perpetuals (`api.hyperliquid.xyz`)
//!   * [`spot`]   — byreal spot / CLMM / RFQ on Solana (`api2.byreal.io`)
//!   * [`lp`]     — byreal Copy Farming + incentives on Solana (`api2.byreal.io`)
//!
//! All three follow the same shape:
//! - A unit-struct `*Client` with a `reqwest::blocking::Client` + base URL.
//! - A `*_client()` accessor backed by `OnceLock` for lazy init.
//! - Plain HTTP wrappers (`get`, `post`) that surface non-2xx as `Err(String)`
//!   and decode the JSON envelope into typed responses.
//!
//! [`ByrealApp`] is the empty marker struct every `DynAomiTool::App` points to.

pub(crate) mod lp;
pub(crate) mod perps;
pub(crate) mod spot;

use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct ByrealApp;

/// Default base URL for the byreal Solana-side API. Both [`spot`] and [`lp`]
/// hit this host (perps is separate, talks straight to Hyperliquid).
pub(crate) const BYREAL_API_BASE: &str = "https://api2.byreal.io";

/// Build a blocking reqwest client with sane defaults for byreal calls.
pub(crate) fn http_client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("[byreal] failed to build HTTP client: {e}"))
}

/// byreal's response envelope. Every endpoint wraps the actual payload under
/// `result.data`; the outer `retCode`/`retMsg` carries transport-level status.
///
/// IMPORTANT: byreal has TWO success indicators — the outer `retCode` is
/// transport-level (200 from their API gateway), and a *separate* inner
/// `result.success` (with sibling `result.ret_code` / `result.ret_msg`) is
/// the actual operation status. For broadcast-sensitive endpoints like
/// `/byreal/api/dex/v2/send-swap-tx`, byreal has been observed returning
/// `retCode:0, result:{success:false, ret_code:500, ret_msg:"Internal Server
/// Error", data:null}` when their broadcast pipeline failed — and worse, in
/// some cases `result.data` is populated with a precomputed signature even
/// though `result.success:false`. Treating that as a success means handing
/// the LLM a phantom signature for a tx that never reached chain.
#[derive(serde::Deserialize)]
struct ByrealEnvelope {
    #[serde(rename = "retCode")]
    ret_code: i64,
    #[serde(rename = "retMsg", default)]
    ret_msg: String,
    #[serde(default)]
    result: Option<ByrealEnvelopeResult>,
}

#[derive(serde::Deserialize)]
struct ByrealEnvelopeResult {
    #[serde(default)]
    data: Option<Value>,
    /// Inner operation status. `None` = field absent (treat as success for
    /// endpoints that don't emit it, e.g. read-only GETs); `Some(true)` =
    /// success; `Some(false)` = failure, surface as Err.
    #[serde(default)]
    success: Option<bool>,
    /// Inner status code (sibling of `success`). Distinct from outer
    /// `retCode`. Present alongside `ret_msg` when `success:false`.
    #[serde(default)]
    ret_code: Option<i64>,
    /// Inner status message (sibling of `success`).
    #[serde(default)]
    ret_msg: Option<String>,
    // Some endpoints (e.g. swap quote) put the data fields directly at the
    // result level instead of under `result.data`. We capture the rest so
    // callers can fall back to the whole `result` object when `data` is absent.
    #[serde(flatten)]
    rest: serde_json::Map<String, Value>,
}

/// GET helper. Strips the byreal envelope and returns the inner `data`
/// (falling back to the full `result` object for endpoints that flatten).
pub(crate) fn byreal_get(
    http: &reqwest::blocking::Client,
    url: &str,
    query: &[(&str, String)],
) -> Result<Value, String> {
    let resp = http
        .get(url)
        .query(query)
        .send()
        .map_err(|e| format!("[byreal] GET {url} failed: {e}"))?;
    decode_envelope(resp, url)
}

/// POST helper with JSON body.
pub(crate) fn byreal_post<B: Serialize>(
    http: &reqwest::blocking::Client,
    url: &str,
    body: &B,
) -> Result<Value, String> {
    let resp = http
        .post(url)
        .json(body)
        .send()
        .map_err(|e| format!("[byreal] POST {url} failed: {e}"))?;
    decode_envelope(resp, url)
}

fn decode_envelope(resp: reqwest::blocking::Response, url: &str) -> Result<Value, String> {
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(format!("[byreal] {url} returned HTTP {status}: {text}"));
    }
    let env: ByrealEnvelope = serde_json::from_str(&text)
        .map_err(|e| format!("[byreal] {url} envelope decode failed: {e}; body: {text}"))?;
    if env.ret_code != 0 {
        return Err(format!(
            "[byreal] {url} returned retCode={} retMsg={}",
            env.ret_code, env.ret_msg
        ));
    }
    match env.result {
        Some(r) => {
            // EVM-analogous safety check: a wallet that fails to broadcast
            // refuses to return a tx hash. byreal's API has been observed
            // populating `result.data` with a precomputed signature even
            // when their broadcast pipeline failed (`result.success:false`).
            // Surface that as Err so the calling tool reports failure to
            // the LLM rather than handing back a phantom signature.
            if matches!(r.success, Some(false)) {
                let code = r
                    .ret_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "unknown".into());
                let msg = r.ret_msg.as_deref().unwrap_or("operation failed");
                return Err(format!(
                    "[byreal] {url} inner status failed: ret_code={code} ret_msg={msg}"
                ));
            }
            Ok(r.data.unwrap_or(Value::Object(r.rest)))
        }
        None => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(body: &str) -> Result<Value, String> {
        // Mirror decode_envelope's parse-then-validate path without needing a
        // live HTTP response. Keeps the test focused on envelope semantics.
        let env: ByrealEnvelope = serde_json::from_str(body)
            .map_err(|e| format!("envelope decode failed: {e}; body: {body}"))?;
        if env.ret_code != 0 {
            return Err(format!("retCode={} retMsg={}", env.ret_code, env.ret_msg));
        }
        match env.result {
            Some(r) => {
                if matches!(r.success, Some(false)) {
                    let code = r
                        .ret_code
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "unknown".into());
                    let msg = r.ret_msg.as_deref().unwrap_or("operation failed");
                    return Err(format!(
                        "inner status failed: ret_code={code} ret_msg={msg}"
                    ));
                }
                Ok(r.data.unwrap_or(Value::Object(r.rest)))
            }
            None => Ok(Value::Null),
        }
    }

    #[test]
    fn happy_path_with_inner_success_true() {
        // /send-swap-tx success: outer retCode=0, inner success=true, data=[sig]
        let body =
            r#"{"retCode":0,"retMsg":"","result":{"success":true,"data":["5gq9signaturebase58"]}}"#;
        let value = decode(body).expect("happy path should be Ok");
        assert_eq!(
            value,
            serde_json::json!(["5gq9signaturebase58"]),
            "data array must be extracted unchanged"
        );
    }

    #[test]
    fn inner_success_false_returns_err_even_when_data_populated() {
        // This is the bug scenario from production: byreal returned
        // retCode=0 (HTTP-level OK) but result.success=false with a phantom
        // signature in result.data. Pre-fix this leaked as success. Now Err.
        let body = r#"{"retCode":0,"retMsg":"","result":{"success":false,"ret_code":500,"ret_msg":"Internal Server Error","data":["5gq9phantomsig"]}}"#;
        let err = decode(body).expect_err("inner success:false must Err");
        assert!(
            err.contains("inner status failed"),
            "err message must flag inner status: {err}"
        );
        assert!(
            err.contains("500"),
            "err should include inner ret_code: {err}"
        );
        assert!(
            err.contains("Internal Server Error"),
            "err should include inner ret_msg: {err}"
        );
    }

    #[test]
    fn inner_success_false_with_null_data_still_errs() {
        // Same as the live byreal response we captured after blockhash expiry.
        let body = r#"{"retCode":0,"retMsg":"","result":{"success":false,"ret_code":500,"ret_msg":"Internal Server Error","data":null}}"#;
        assert!(decode(body).is_err());
    }

    #[test]
    fn absent_success_field_still_passes_through() {
        // GET endpoints (pools, tokens, ...) don't populate `success`. The
        // pre-fix behavior must be preserved: absent success treated as Ok.
        let body = r#"{"retCode":0,"result":{"data":{"records":[],"total":0}}}"#;
        let value = decode(body).expect("absent success must Ok");
        assert_eq!(value, serde_json::json!({"records":[],"total":0}));
    }

    #[test]
    fn absent_data_falls_back_to_flattened_rest() {
        // Some endpoints (e.g. swap quote) inline payload fields at the
        // `result` level instead of nesting under `result.data`. The
        // flattened `rest` map must still be returned.
        let body = r#"{"retCode":0,"result":{"transaction":"AQAAAA","quoteId":"q-1"}}"#;
        let value = decode(body).expect("flattened result must Ok");
        let obj = value.as_object().expect("must be object");
        assert_eq!(obj.get("transaction"), Some(&serde_json::json!("AQAAAA")));
        assert_eq!(obj.get("quoteId"), Some(&serde_json::json!("q-1")));
    }

    #[test]
    fn outer_retcode_nonzero_errs() {
        let body = r#"{"retCode":1001,"retMsg":"invalid signature"}"#;
        let err = decode(body).expect_err("non-zero retCode must Err");
        assert!(err.contains("retCode=1001"));
        assert!(err.contains("invalid signature"));
    }
}
