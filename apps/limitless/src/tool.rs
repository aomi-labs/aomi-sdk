//! Curated tool layer for Limitless prediction markets on Base. Public reads
//! go through the progenitor-generated client at `crate::client` (see
//! `apps/limitless/openapi.yaml`). Authenticated reads (portfolio, key info)
//! bypass the generated client and call reqwest directly so we can attach the
//! HMAC-signed `lmts-*` headers that the Limitless spec doesn't model as
//! per-operation parameters — see `crate::auth` for the prehash format.
//!
//! Designed for the user story: research active prediction markets on Base
//! via Limitless, look at orderbooks, view your positions/trades, and place
//! limit/market orders end-to-end through the host wallet flow.
//!
//! 9 curated tools:
//!   * limitless_search_markets       — semantic text search across markets (public)
//!   * limitless_browse_active        — list active markets by category    (public)
//!   * limitless_get_market           — full detail for one market         (public)
//!   * limitless_get_orderbook        — bids/asks for a market             (public)
//!   * limitless_check_key            — verify the API key + secret work   (signed)
//!   * limitless_get_my_positions     — your open positions                (signed)
//!   * limitless_get_my_trades        — your trade history                 (signed)
//!   * limitless_build_order          — build + route order to wallet sign  (routed)
//!   * limitless_submit_order         — POST signed order to /orders        (signed)

use crate::auth::{iso_timestamp, sign};
// The progenitor-generated client is currently unused at runtime: every
// endpoint we exercise has spec drift (renamed query params, extra response
// fields, or 404 paths), so the curated layer hits the API via raw reqwest
// in `public_get` / `signed_get`. We keep the generated module compiled so
// regenerating the spec is friction-free; switch back to typed calls once
// the spec is realigned with the live server.
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub(crate) struct LimitlessApp;

const BASE_URL: &str = "https://api.limitless.exchange";

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value).map_err(|e| format!("[limitless] serialize: {e}"))?;
    Ok(match value {
        Value::Object(mut m) => {
            m.insert("source".into(), Value::String("limitless".into()));
            Value::Object(m)
        }
        other => json!({ "source": "limitless", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[limitless] runtime: {e}"))
}

/// Minimal percent-encoder for URL path/query segments. Keeps unreserved
/// characters per RFC 3986 verbatim, percent-encodes everything else.
fn urlencode(s: &str) -> String {
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

/// Resolve `(api_key, api_secret)` from the tool args first, then the per-app
/// secret vault (via `ctx.secrets`), then env vars.
fn resolve_creds(
    ctx: &DynToolCallCtx,
    api_key: Option<&str>,
    api_secret: Option<&str>,
) -> Result<(String, String), String> {
    let key = resolve_secret_value(
        ctx,
        api_key,
        "LIMITLESS_API_KEY",
        "[limitless] missing api_key argument and LIMITLESS_API_KEY env var",
    )?;
    let sec = resolve_secret_value(
        ctx,
        api_secret,
        "LIMITLESS_API_SECRET",
        "[limitless] missing api_secret argument and LIMITLESS_API_SECRET env var",
    )?;
    Ok((key, sec))
}

/// Run an HMAC-signed GET against `path_with_query` (no body) and decode the
/// JSON response into `serde_json::Value`. Used by every signed tool below.
async fn signed_get(
    api_key: &str,
    api_secret: &str,
    path_with_query: &str,
) -> Result<Value, String> {
    let timestamp = iso_timestamp();
    let signature = sign(api_secret, &timestamp, "GET", path_with_query, "")?;
    let url = format!("{BASE_URL}{path_with_query}");
    let resp = reqwest::Client::new()
        .get(&url)
        .header("lmts-api-key", api_key)
        .header("lmts-timestamp", &timestamp)
        .header("lmts-signature", &signature)
        .send()
        .await
        .map_err(|e| format!("[limitless] HTTP error on {path_with_query}: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("[limitless] failed to read response body: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "[limitless] {path_with_query} returned {status}: {body}"
        ));
    }
    serde_json::from_str(&body).map_err(|e| {
        format!(
            "[limitless] response was not JSON ({e}); first 200 chars: {}",
            body.chars().take(200).collect::<String>()
        )
    })
}

/// Public GET (no auth) returning raw JSON. Used for endpoints where the
/// generated client's typed response disagrees with the live API (spec drift)
/// — we forward the body unchanged and let the LLM read what's actually there.
async fn public_get(path_with_query: &str) -> Result<Value, String> {
    let url = format!("{BASE_URL}{path_with_query}");
    let resp = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("[limitless] HTTP error on {path_with_query}: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("[limitless] failed to read response body: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "[limitless] {path_with_query} returned {status}: {body}"
        ));
    }
    serde_json::from_str(&body).map_err(|e| {
        format!(
            "[limitless] response was not JSON ({e}); first 200 chars: {}",
            body.chars().take(200).collect::<String>()
        )
    })
}

// ============================================================================
// Tool 1: search_markets — semantic search (public)
// ============================================================================

pub(crate) struct SearchMarkets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchMarketsArgs {
    /// Free-text query (e.g., "election", "ETH price by year-end", "fed rate cut").
    pub query: String,
    /// Max markets to return (default 20).
    #[serde(default)]
    pub limit: Option<i64>,
}

impl DynAomiTool for SearchMarkets {
    type App = LimitlessApp;
    type Args = SearchMarketsArgs;
    const NAME: &'static str = "limitless_search_markets";
    const DESCRIPTION: &'static str = "Semantic search across active Limitless prediction markets. Use when the user names a topic ('election', 'ETH price', 'fed cuts') and wants matching markets. Returns market summaries (slug, title, current YES/NO prices). Public — no API key needed.";

    fn run(_app: &LimitlessApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            // Live API expects ?query= (the spec called it `q`, which the
            // server rejects). Use raw public_get so we control the wire.
            let mut path = format!("/markets/search?query={}", urlencode(&args.query));
            if let Some(limit) = args.limit {
                path.push_str(&format!("&limit={limit}"));
            }
            let resp = public_get(&path).await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 2: browse_active — list active markets, optionally by category (public)
// ============================================================================

pub(crate) struct BrowseActive;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BrowseActiveArgs {
    /// Optional category ID to filter by. Omit to browse all categories.
    #[serde(default)]
    pub category_id: Option<f64>,
    /// Max results per page (default 50).
    #[serde(default)]
    pub limit: Option<i64>,
    /// Page index (1-based, default 1).
    #[serde(default)]
    pub page: Option<i64>,
}

impl DynAomiTool for BrowseActive {
    type App = LimitlessApp;
    type Args = BrowseActiveArgs;
    const NAME: &'static str = "limitless_browse_active";
    const DESCRIPTION: &'static str = "Browse active Limitless markets. Use when the user wants to discover what's currently tradeable. Optional `category_id` narrows the list. Public — no API key needed.";

    fn run(_app: &LimitlessApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            // Live API rejects unknown query params, so build the URL by hand
            // (the generated client sends a few extras that aren't accepted).
            let limit = args.limit.unwrap_or(50);
            let page = args.page.unwrap_or(1);
            let mut path = format!("/markets/active?limit={limit}&page={page}");
            if let Some(cat) = args.category_id {
                path.push_str(&format!("&category_id={cat}"));
            }
            let resp = public_get(&path).await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 3: get_market — full detail for a single market (public)
// ============================================================================

pub(crate) struct GetMarket;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMarketArgs {
    /// Market slug or contract address.
    pub address_or_slug: String,
}

impl DynAomiTool for GetMarket {
    type App = LimitlessApp;
    type Args = GetMarketArgs;
    const NAME: &'static str = "limitless_get_market";
    const DESCRIPTION: &'static str = "Fetch full detail for one market by slug or contract address — title, description, outcomes, current prices, volume, expiration. Use after `limitless_search_markets` or `limitless_browse_active` returns a slug. Public — no API key needed.";

    fn run(_app: &LimitlessApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            // Path is `/markets/{slug}`. Forward raw JSON — the typed schema
            // misses several fields the live API returns.
            let path = format!("/markets/{}", urlencode(&args.address_or_slug));
            let resp = public_get(&path).await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 4: get_orderbook — bids/asks for a market (public)
// ============================================================================

pub(crate) struct GetOrderbook;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOrderbookArgs {
    /// Market slug.
    pub slug: String,
}

impl DynAomiTool for GetOrderbook {
    type App = LimitlessApp;
    type Args = GetOrderbookArgs;
    const NAME: &'static str = "limitless_get_orderbook";
    const DESCRIPTION: &'static str = "Get the L2 orderbook (bids and asks) for a Limitless market. Use to gauge liquidity before placing an order. Public — no API key needed.";

    fn run(_app: &LimitlessApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            // Live response carries an extra `side` field per level (and may
            // omit some optional fields), so the generated typed response
            // fails to deserialize. Forward the raw JSON instead.
            let path = format!("/markets/{}/orderbook", urlencode(&args.slug));
            let resp = public_get(&path).await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 5: check_key — verify the active key works (signed)
// ============================================================================

pub(crate) struct CheckKey;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CheckKeyArgs {
    /// Limitless API key id (falls back to LIMITLESS_API_KEY env var).
    #[serde(default)]
    #[schemars(skip)]
    pub api_key: Option<String>,
    /// Limitless API secret, base64-encoded as shown in the dashboard
    /// (falls back to LIMITLESS_API_SECRET env var).
    #[serde(default)]
    #[schemars(skip)]
    pub api_secret: Option<String>,
}

impl DynAomiTool for CheckKey {
    type App = LimitlessApp;
    type Args = CheckKeyArgs;
    const NAME: &'static str = "limitless_check_key";
    const DESCRIPTION: &'static str = "Verify that the configured Limitless API key + secret work. Calls GET /auth/api-keys/active and returns metadata about the active key (id, scopes, created_at). Run this first if any signed tool starts failing.";

    fn run(_app: &LimitlessApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let (key, sec) = resolve_creds(&ctx, args.api_key.as_deref(), args.api_secret.as_deref())?;
        let runtime = rt()?;
        runtime.block_on(async move {
            // Live path is `/auth/api-keys` (no `/active` suffix). Returns
            // the metadata for the currently-authenticated key.
            let resp = signed_get(&key, &sec, "/auth/api-keys").await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 6: get_my_positions — open positions for the authenticated user (signed)
// ============================================================================

pub(crate) struct GetMyPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMyPositionsArgs {
    /// Limitless API key id (falls back to LIMITLESS_API_KEY env var).
    #[serde(default)]
    #[schemars(skip)]
    pub api_key: Option<String>,
    /// Limitless API secret (falls back to LIMITLESS_API_SECRET env var).
    #[serde(default)]
    #[schemars(skip)]
    pub api_secret: Option<String>,
}

impl DynAomiTool for GetMyPositions {
    type App = LimitlessApp;
    type Args = GetMyPositionsArgs;
    const NAME: &'static str = "limitless_get_my_positions";
    const DESCRIPTION: &'static str = "Get the authenticated user's open positions across all Limitless markets. Requires a Limitless API key + secret (HMAC-signed). Returns size, entry price, current PnL per position.";

    fn run(_app: &LimitlessApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let (key, sec) = resolve_creds(&ctx, args.api_key.as_deref(), args.api_secret.as_deref())?;
        let runtime = rt()?;
        runtime.block_on(async move {
            let resp = signed_get(&key, &sec, "/portfolio/positions").await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 7: get_my_trades — trade history for the authenticated user (signed)
// ============================================================================

pub(crate) struct GetMyTrades;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMyTradesArgs {
    /// Limitless API key id (falls back to LIMITLESS_API_KEY env var).
    #[serde(default)]
    #[schemars(skip)]
    pub api_key: Option<String>,
    /// Limitless API secret (falls back to LIMITLESS_API_SECRET env var).
    #[serde(default)]
    #[schemars(skip)]
    pub api_secret: Option<String>,
}

impl DynAomiTool for GetMyTrades {
    type App = LimitlessApp;
    type Args = GetMyTradesArgs;
    const NAME: &'static str = "limitless_get_my_trades";
    const DESCRIPTION: &'static str = "Get the authenticated user's recent trade fills on Limitless. Requires a Limitless API key + secret (HMAC-signed). Useful for PnL review and position reconciliation.";

    fn run(_app: &LimitlessApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let (key, sec) = resolve_creds(&ctx, args.api_key.as_deref(), args.api_secret.as_deref())?;
        let runtime = rt()?;
        runtime.block_on(async move {
            let resp = signed_get(&key, &sec, "/portfolio/trades").await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Helpers — order construction, EIP-712 typed data, signed POST
// ============================================================================

/// One Limitless CTF Exchange order — the EIP-712 message + the bits required
/// to POST it to `/orders`. Passed verbatim from `limitless_build_order` to
/// `limitless_submit_order` via the routed continuation; the LLM never
/// constructs this struct by hand.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LimitlessOrderPlan {
    pub salt: u64,
    pub maker: String,
    pub signer: String,
    pub taker: String,
    pub token_id: String,
    pub maker_amount: u64,
    pub taker_amount: u64,
    pub expiration: u64,
    pub nonce: u64,
    pub fee_rate_bps: u64,
    pub side: u8,
    pub signature_type: u8,
    pub price: f64,
    pub size: f64,
    pub outcome: String,
    pub side_label: String,
    pub market_slug: String,
    pub order_type: String,
    pub verifying_contract: String,
    pub owner_id: i64,
    pub chain_id: u64,
}

/// Compute (makerAmount, takerAmount) per the spec at
/// `openapi.yaml#/components/schemas/Order.makerAmount.description`:
/// GTC BUY → makerAmount = price*size*1e6, takerAmount = size*1e6
/// GTC SELL → makerAmount = size*1e6, takerAmount = price*size*1e6
/// FOK BUY → makerAmount = USDC_to_spend*1e6, takerAmount = 1
/// FOK SELL → makerAmount = shares*1e6, takerAmount = 1
fn compute_amounts(
    side: u8,
    price: f64,
    size: f64,
    order_type: &str,
) -> Result<(u64, u64), String> {
    if !(0.01..=0.99).contains(&price) {
        return Err(format!(
            "[limitless] price must be in [0.01, 0.99] for GTC orders, got {price}"
        ));
    }
    if size <= 0.0 {
        return Err(format!("[limitless] size must be > 0, got {size}"));
    }
    let scale = 1_000_000.0;
    Ok(match (side, order_type) {
        (0, "GTC") => (
            (price * size * scale).round() as u64,
            (size * scale).round() as u64,
        ),
        (1, "GTC") => (
            (size * scale).round() as u64,
            (price * size * scale).round() as u64,
        ),
        (0, "FOK") => ((size * scale).round() as u64, 1),
        (1, "FOK") => ((size * scale).round() as u64, 1),
        _ => {
            return Err(format!(
                "[limitless] unsupported (side={side}, order_type={order_type})"
            ));
        }
    })
}

/// EIP-712 typed-data payload that the host wallet will sign. Mirrors the
/// shape `host::EvmCommitMessage` expects (typed_data + human description).
fn build_order_typed_data(plan: &LimitlessOrderPlan) -> Value {
    json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"}
            ],
            "Order": [
                {"name": "salt", "type": "uint256"},
                {"name": "maker", "type": "address"},
                {"name": "signer", "type": "address"},
                {"name": "taker", "type": "address"},
                {"name": "tokenId", "type": "uint256"},
                {"name": "makerAmount", "type": "uint256"},
                {"name": "takerAmount", "type": "uint256"},
                {"name": "expiration", "type": "uint256"},
                {"name": "nonce", "type": "uint256"},
                {"name": "feeRateBps", "type": "uint256"},
                {"name": "side", "type": "uint8"},
                {"name": "signatureType", "type": "uint8"}
            ]
        },
        "primaryType": "Order",
        "domain": {
            "name": "Limitless CTF Exchange",
            "version": "1",
            "chainId": plan.chain_id,
            "verifyingContract": plan.verifying_contract,
        },
        "message": {
            "salt": plan.salt.to_string(),
            "maker": plan.maker,
            "signer": plan.signer,
            "taker": plan.taker,
            "tokenId": plan.token_id,
            "makerAmount": plan.maker_amount.to_string(),
            "takerAmount": plan.taker_amount.to_string(),
            "expiration": plan.expiration.to_string(),
            "nonce": plan.nonce.to_string(),
            "feeRateBps": plan.fee_rate_bps.to_string(),
            "side": plan.side,
            "signatureType": plan.signature_type,
        }
    })
}

/// HMAC-signed POST against `path_with_query`. Mirrors `signed_get` but
/// includes a JSON-stringified `body` in the prehash and as the request body.
async fn signed_post(
    api_key: &str,
    api_secret: &str,
    path_with_query: &str,
    body_json: &Value,
) -> Result<Value, String> {
    let body =
        serde_json::to_string(body_json).map_err(|e| format!("[limitless] serialize body: {e}"))?;
    let timestamp = iso_timestamp();
    let signature = sign(api_secret, &timestamp, "POST", path_with_query, &body)?;
    let url = format!("{BASE_URL}{path_with_query}");
    let resp = reqwest::Client::new()
        .post(&url)
        .header("lmts-api-key", api_key)
        .header("lmts-timestamp", &timestamp)
        .header("lmts-signature", &signature)
        .header("content-type", "application/json")
        .body(body)
        .send()
        .await
        .map_err(|e| format!("[limitless] HTTP error on POST {path_with_query}: {e}"))?;
    let status = resp.status();
    let resp_body = resp
        .text()
        .await
        .map_err(|e| format!("[limitless] failed to read response body: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "[limitless] POST {path_with_query} returned {status}: {resp_body}"
        ));
    }
    serde_json::from_str(&resp_body).map_err(|e| {
        format!(
            "[limitless] POST response was not JSON ({e}); first 200 chars: {}",
            resp_body.chars().take(200).collect::<String>()
        )
    })
}

/// Extract the per-market exchange contract address from the `get_market`
/// response. The live API surfaces this at `venue.exchange`; older payloads
/// occasionally inline it at the top level. Returns `Err` with a helpful
/// hint when the slug is a *group* market (no direct exchange — caller must
/// pick a child slug from `markets[]`).
fn extract_verifying_contract(market: &Value) -> Result<String, String> {
    // Canonical location: top-level `venue.exchange`.
    if let Some(s) = market
        .get("venue")
        .and_then(|v| v.get("exchange"))
        .and_then(|v| v.as_str())
        && s.starts_with("0x")
        && s.len() == 42
    {
        return Ok(s.to_string());
    }
    // Legacy top-level keys.
    for key in ["exchange", "exchangeAddress"] {
        if let Some(s) = market.get(key).and_then(|v| v.as_str())
            && s.starts_with("0x")
            && s.len() == 42
        {
            return Ok(s.to_string());
        }
    }
    // Group-market case — surface the child slugs so the user knows what to pick.
    if let Some(children) = market.get("markets").and_then(|v| v.as_array()) {
        let child_slugs: Vec<String> = children
            .iter()
            .filter_map(|m| m.get("slug").and_then(|v| v.as_str()).map(String::from))
            .collect();
        if !child_slugs.is_empty() {
            return Err(format!(
                "[limitless] this slug is a GROUP market — pass a child slug instead. Children: {}",
                child_slugs.join(", ")
            ));
        }
    }
    Err("[limitless] could not resolve venue.exchange from market response".to_string())
}

/// Extract YES/NO tokenIds from the market response. Live API uses either
/// `tokens: {yes, no}` (current) or `tokens: [{outcome, tokenId}]` (legacy);
/// some flat payloads use `yes_token_id` / `no_token_id`.
fn extract_token_id(market: &Value, outcome: &str) -> Option<String> {
    let want = outcome.to_ascii_uppercase();
    let tokens = market.get("tokens")?;

    // Current shape: { "yes": "<numeric>", "no": "<numeric>" }
    if tokens.is_object() {
        let key = if want == "YES" { "yes" } else { "no" };
        if let Some(s) = tokens.get(key).and_then(|v| v.as_str()) {
            return Some(s.to_string());
        }
    }
    // Legacy shape: [{outcome, tokenId}]
    if let Some(arr) = tokens.as_array() {
        for tok in arr {
            let o = tok
                .get("outcome")
                .or_else(|| tok.get("label"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_ascii_uppercase())
                .unwrap_or_default();
            if o == want
                && let Some(id) = tok.get("tokenId").and_then(|v| v.as_str())
            {
                return Some(id.to_string());
            }
        }
    }
    // Flat fallback.
    let key = if want == "YES" {
        "yes_token_id"
    } else {
        "no_token_id"
    };
    market.get(key).and_then(|v| v.as_str()).map(String::from)
}

// ============================================================================
// Tool 8: build_order — compose order + route to host wallet for EIP-712 sign
// ============================================================================

pub(crate) struct BuildOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BuildOrderArgs {
    /// Market slug (e.g. `epl-brentford-vs-crystal-palace-may-17-2026-1777798810481`).
    pub slug: String,
    /// `"YES"` or `"NO"`.
    pub outcome: String,
    /// `"BUY"` or `"SELL"`.
    pub side: String,
    /// Limit price as decimal in [0.01, 0.99] (probability, not USD).
    pub price: f64,
    /// Share count (NOT USDC) the user wants to BUY or SELL.
    pub size: f64,
    /// `"GTC"` (default) or `"FOK"`.
    #[serde(default)]
    pub order_type: Option<String>,
    /// User's wallet address on Base (`maker` / `signer`).
    pub wallet_address: String,
    /// Profile ID of the order owner (from `/auth/api-keys` response or dashboard).
    pub owner_id: i64,
    /// Optional explicit nonce. Defaults to current ms epoch.
    #[serde(default)]
    pub nonce: Option<u64>,
    /// Optional expiration timestamp (seconds). Defaults to 0 (no expiration).
    #[serde(default)]
    pub expiration: Option<u64>,
    /// Optional fee rate override (bps). Defaults to 0; the matcher will
    /// charge the live rate regardless.
    #[serde(default)]
    pub fee_rate_bps: Option<u64>,
}

impl DynAomiTool for BuildOrder {
    type App = LimitlessApp;
    type Args = BuildOrderArgs;
    const NAME: &'static str = "limitless_build_order";
    const DESCRIPTION: &'static str = "Build a Limitless CTF Exchange order and route the EIP-712 signing step to the user's wallet. After the wallet signs, the runtime automatically continues to `limitless_submit_order` with the bound signature, which POSTs to /orders. Args: slug, outcome (YES/NO), side (BUY/SELL), price (0.01..0.99), size (shares), wallet_address, owner_id. Optional: order_type (GTC|FOK), nonce, expiration, fee_rate_bps.";

    fn run_with_routes(
        _app: &LimitlessApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let order_type = args.order_type.as_deref().unwrap_or("GTC").to_uppercase();
        if !matches!(order_type.as_str(), "GTC" | "FOK") {
            return Err(format!("[limitless] unsupported order_type={order_type}"));
        }
        let side_label = args.side.to_uppercase();
        let side: u8 = match side_label.as_str() {
            "BUY" => 0,
            "SELL" => 1,
            _ => {
                return Err(format!(
                    "[limitless] side must be BUY or SELL, got {side_label}"
                ));
            }
        };
        let outcome_label = args.outcome.to_uppercase();
        if !matches!(outcome_label.as_str(), "YES" | "NO") {
            return Err(format!(
                "[limitless] outcome must be YES or NO, got {outcome_label}"
            ));
        }

        // Resolve verifying contract + tokenId from the market.
        let market_path = format!("/markets/{}", urlencode(&args.slug));
        let market = rt()?.block_on(public_get(&market_path))?;
        let verifying_contract = extract_verifying_contract(&market)?;
        let token_id = extract_token_id(&market, &outcome_label).ok_or_else(|| {
            format!(
                "[limitless] could not resolve {outcome_label} tokenId from market {}",
                args.slug
            )
        })?;

        let (maker_amount, taker_amount) =
            compute_amounts(side, args.price, args.size, &order_type)?;
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let plan = LimitlessOrderPlan {
            salt: now_ms,
            maker: args.wallet_address.clone(),
            signer: args.wallet_address.clone(),
            taker: "0x0000000000000000000000000000000000000000".to_string(),
            token_id,
            maker_amount,
            taker_amount,
            expiration: args.expiration.unwrap_or(0),
            nonce: args.nonce.unwrap_or(now_ms),
            fee_rate_bps: args.fee_rate_bps.unwrap_or(0),
            side,
            signature_type: 0, // EOA
            price: args.price,
            size: args.size,
            outcome: outcome_label.clone(),
            side_label: side_label.clone(),
            market_slug: args.slug.clone(),
            order_type: order_type.clone(),
            verifying_contract,
            owner_id: args.owner_id,
            chain_id: 8453,
        };

        let typed_data = build_order_typed_data(&plan);
        let description = format!(
            "Limitless: {} {} {} @ {} on {}",
            side_label, args.size, outcome_label, args.price, args.slug
        );
        let wallet_request = json!({
            "typed_data": typed_data,
            "description": description,
        });

        let mut result = json!({
            "source": "limitless",
            "stage": "awaiting_order_signature",
            "order_plan": plan,
            "preview": {
                "market_slug": plan.market_slug,
                "outcome": plan.outcome,
                "side": plan.side_label,
                "price": plan.price,
                "size": plan.size,
                "maker_amount_usdc_micro": plan.maker_amount,
                "taker_amount_micro": plan.taker_amount,
                "verifying_contract": plan.verifying_contract,
                "order_type": plan.order_type,
            },
            "wallet_request": wallet_request,
        });
        if let Some(obj) = result.as_object_mut() {
            obj.insert(
                "wallet_signature_step".to_string(),
                json!({
                    "wallet_tool": host::EvmCommitMessage::tool_name(),
                    "signing_primitive": "EIP712_TYPED_DATA_V4",
                    "callback_field": "order_signature",
                    "requires_user_confirmation_before_call": true,
                }),
            );
        }

        let submit_template = json!({
            "order_plan": plan,
            "order_signature": null,
        });

        Ok(ToolReturn::route(result)
            .next(|next| {
                next.add::<host::EvmCommitMessage>(wallet_request)
                    .bind_as("order_signature")
                    .note("Sign this Limitless order. User has already confirmed price/size upstream.");
            })
            .after::<SubmitOrder>(submit_template)
            .awaits("order_signature")
            .note("Wallet signed — POST the signed order to Limitless /orders.")
            .build())
    }
}

// ============================================================================
// Tool 9: submit_order — POST signed order to /orders
// ============================================================================

pub(crate) struct SubmitOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SubmitOrderArgs {
    /// Order plan produced by `limitless_build_order` — forward verbatim.
    pub order_plan: LimitlessOrderPlan,
    /// Wallet signature for the order typed data. Bound automatically by the
    /// runtime when `evm_commit_message` returns its `order_signature` callback;
    /// the LLM should never have to fill this manually.
    pub order_signature: Option<String>,
    /// Optional client-side idempotency key (max 128 chars).
    #[serde(default)]
    pub client_order_id: Option<String>,
    #[serde(default)]
    #[schemars(skip)]
    pub api_key: Option<String>,
    #[serde(default)]
    #[schemars(skip)]
    pub api_secret: Option<String>,
}

impl DynAomiTool for SubmitOrder {
    type App = LimitlessApp;
    type Args = SubmitOrderArgs;
    const NAME: &'static str = "limitless_submit_order";
    const DESCRIPTION: &'static str = "POST a wallet-signed Limitless order to /orders. Continuation of `limitless_build_order` — usually invoked automatically by the runtime after the wallet sig callback binds `order_signature`. Treat `order_plan` as opaque continuation state.";

    fn run(_app: &LimitlessApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let signature = args
            .order_signature
            .as_deref()
            .ok_or_else(|| {
                "[limitless] submit_order needs order_signature from the wallet callback"
                    .to_string()
            })?
            .to_string();
        let (key, sec) = resolve_creds(&ctx, args.api_key.as_deref(), args.api_secret.as_deref())?;

        let plan = args.order_plan;
        let order_body = json!({
            "salt": plan.salt,
            "maker": plan.maker,
            "signer": plan.signer,
            "taker": plan.taker,
            "tokenId": plan.token_id,
            "makerAmount": plan.maker_amount,
            "takerAmount": plan.taker_amount,
            "expiration": plan.expiration.to_string(),
            "nonce": plan.nonce,
            "price": plan.price,
            "feeRateBps": plan.fee_rate_bps,
            "side": plan.side,
            "signature": signature,
            "signatureType": plan.signature_type,
        });
        let mut envelope = json!({
            "order": order_body,
            "ownerId": plan.owner_id,
            "orderType": plan.order_type,
            "marketSlug": plan.market_slug,
        });
        if let (Some(obj), Some(coi)) = (envelope.as_object_mut(), args.client_order_id.as_deref())
        {
            obj.insert("clientOrderId".to_string(), Value::String(coi.to_string()));
        }

        let runtime = rt()?;
        runtime.block_on(async move {
            let resp = signed_post(&key, &sec, "/orders", &envelope).await?;
            ok(resp)
        })
    }
}
