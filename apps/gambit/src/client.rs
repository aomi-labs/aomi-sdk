use crate::auth;
use serde_json::Value;

const LIMITLESS_URL: &str = "https://api.limitless.exchange";
const ODDS_API_URL: &str = "https://api.the-odds-api.com/v4";

// ============================================================================
// Limitless API — public endpoints
// ============================================================================

pub(crate) async fn limitless_public(path: &str) -> Result<Value, String> {
    let url = format!("{LIMITLESS_URL}{path}");
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("[gambit] Limitless HTTP: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("[gambit] body read: {e}"))?;
    if !status.is_success() {
        return Err(format!("[gambit] {path} → {status}: {body}"));
    }
    serde_json::from_str(&body).map_err(|e| {
        format!(
            "[gambit] JSON parse ({e}): {}",
            body.chars().take(200).collect::<String>()
        )
    })
}

// ============================================================================
// Limitless API — HMAC-signed endpoints
// ============================================================================

pub(crate) async fn limitless_signed(
    api_key: &str,
    api_secret: &str,
    method: &str,
    path: &str,
    body: &str,
) -> Result<Value, String> {
    let timestamp = auth::iso_timestamp();
    let signature = auth::sign(api_secret, &timestamp, method, path, body)
        .map_err(|e| format!("[gambit] HMAC sign: {e}"))?;
    let url = format!("{LIMITLESS_URL}{path}");

    let client = reqwest::Client::new();
    let req = match method {
        "POST" => client
            .post(&url)
            .header("content-type", "application/json")
            .body(body.to_string()),
        _ => client.get(&url),
    };

    let resp = req
        .header("lmts-api-key", api_key)
        .header("lmts-timestamp", &timestamp)
        .header("lmts-signature", &signature)
        .send()
        .await
        .map_err(|e| format!("[gambit] signed HTTP: {e}"))?;

    let status = resp.status();
    let resp_body = resp
        .text()
        .await
        .map_err(|e| format!("[gambit] body read: {e}"))?;
    if !status.is_success() {
        return Err(format!("[gambit] signed {path} → {status}: {resp_body}"));
    }
    serde_json::from_str(&resp_body).map_err(|e| {
        format!(
            "[gambit] JSON ({e}): {}",
            resp_body.chars().take(200).collect::<String>()
        )
    })
}

// ============================================================================
// The Odds API — real-world bookmaker odds
// ============================================================================

pub(crate) async fn odds_api(path: &str, api_key: &str) -> Result<Value, String> {
    let sep = if path.contains('?') { "&" } else { "?" };
    let url = format!("{ODDS_API_URL}{path}{sep}apiKey={api_key}");
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("[gambit] Odds API HTTP: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("[gambit] body read: {e}"))?;
    if !status.is_success() {
        return Err(format!("[gambit] Odds API {path} → {status}: {body}"));
    }
    serde_json::from_str(&body).map_err(|e| {
        format!(
            "[gambit] JSON parse ({e}): {}",
            body.chars().take(200).collect::<String>()
        )
    })
}

// ============================================================================
// Utility functions
// ============================================================================

pub(crate) fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        let c = *b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{:02X}", b);
        }
    }
    out
}

/// Decimal odds → implied probability (0.0–1.0)
pub(crate) fn decimal_to_prob(decimal: f64) -> f64 {
    if decimal > 0.0 { 1.0 / decimal } else { 0.0 }
}

/// Edge = bookmaker probability − Limitless price.
/// Positive = Limitless underpriced (value bet on YES).
pub(crate) fn edge(bookmaker_prob: f64, limitless_price: f64) -> f64 {
    bookmaker_prob - limitless_price
}

/// Format probability as percentage string.
pub(crate) fn pct(p: f64) -> String {
    format!("{}%", (p * 100.0).round())
}

/// Extract YES price from a Limitless market JSON.
pub(crate) fn yes_price(m: &Value) -> f64 {
    m.get("yesPrice")
        .or_else(|| m.get("outcomePrices").and_then(|o| o.get(0)))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

/// Extract NO price from a Limitless market JSON.
pub(crate) fn no_price(m: &Value) -> f64 {
    m.get("noPrice")
        .or_else(|| m.get("outcomePrices").and_then(|o| o.get(1)))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}

/// Extract title from a Limitless market JSON.
pub(crate) fn market_title(m: &Value) -> &str {
    m.get("title")
        .or_else(|| m.get("question"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
}

/// Extract volume from a Limitless market JSON.
pub(crate) fn market_volume(m: &Value) -> f64 {
    m.get("volume")
        .or_else(|| m.get("volume24hr"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0)
}
