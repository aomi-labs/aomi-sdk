//! Curated tool layer for Zora creator/content coins on Base.
//!
//! The progenitor-generated client at `crate::client` is compiled but **not
//! used at runtime** — same pattern as `apps/limitless/src/tool.rs`. The
//! local `openapi.yaml` predates Zora's Uniswap V3 → V4 migration: it still
//! marks `uniswapV3PoolAddress` (and on `/coin` also `zoraComments` +
//! `creatorEarnings`) as required, but the live API dropped those fields
//! when it moved to V4 hooks. The `/featuredCreators` response wrapper also
//! got renamed (`featuredCreators` → `traderLeaderboardFeaturedCreators`).
//!
//! Until the spec is realigned + regenerated, we hit the live API via raw
//! reqwest and forward the JSON body unchanged. The LLM reads what the
//! server actually returns instead of getting a deserialize error or an
//! empty struct.
//!
//! 6 curated read tools focused on discovery + analytics. Coin deployment +
//! quoted swaps (POST /quote, Uniswap V4 hook setup) are intentionally NOT
//! exposed — those need typed bodies + signing.

use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub(crate) struct ZoraApp;

const BASE_URL: &str = "https://api-sdk.zora.engineering";

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value).map_err(|e| format!("[zora] serialize: {e}"))?;
    Ok(match value {
        Value::Object(mut m) => {
            m.insert("source".into(), Value::String("zora".into()));
            Value::Object(m)
        }
        other => json!({ "source": "zora", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[zora] runtime: {e}"))
}

/// Minimal percent-encoder for URL query values. Keeps RFC 3986 unreserved
/// chars verbatim, percent-encodes everything else.
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

/// Resolve the optional Zora API key: explicit arg, then the host-provisioned
/// `ZORA_API_KEY` slot. Returning `None` is fine — every Zora endpoint also
/// works unauthenticated at a lower rate limit.
fn resolve_key(ctx: &DynToolCallCtx, api_key: Option<&str>) -> Option<String> {
    resolve_secret_value(ctx, api_key, "ZORA_API_KEY", "").ok()
}

/// GET `path_with_query` and decode as JSON. Injects `api-key` header when
/// one is configured.
async fn public_get(path_with_query: &str, api_key: Option<&str>) -> Result<Value, String> {
    let url = format!("{BASE_URL}{path_with_query}");
    let mut req = reqwest::Client::new().get(&url);
    if let Some(k) = api_key {
        req = req.header("api-key", k);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("[zora] HTTP error on {path_with_query}: {e}"))?;
    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| format!("[zora] failed to read response body: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "[zora] {path_with_query} returned {status}: {body}"
        ));
    }
    serde_json::from_str(&body).map_err(|e| {
        format!(
            "[zora] response was not JSON ({e}); first 200 chars: {}",
            body.chars().take(200).collect::<String>()
        )
    })
}

// ============================================================================
// Tool 1: get_coin
// ============================================================================
pub(crate) struct GetCoin;
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCoinArgs {
    /// Coin contract address on Base (`0x...`).
    pub address: String,
    /// Optional chain ID (defaults to 8453, Base mainnet).
    #[serde(default)]
    pub chain: Option<i64>,
    #[serde(default)]
    pub api_key: Option<String>,
}
impl DynAomiTool for GetCoin {
    type App = ZoraApp;
    type Args = GetCoinArgs;
    const NAME: &'static str = "zora_get_coin";
    const DESCRIPTION: &'static str = "Get full detail for a Zora coin by address — name, symbol, creator, market data. Use after the user names a coin or one is discovered via `zora_get_trends_by_name`.";

    fn run(_app: &ZoraApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let key = resolve_key(&ctx, args.api_key.as_deref());
        rt()?.block_on(async move {
            let chain = args.chain.unwrap_or(8453);
            let path = format!("/coin?address={}&chain={}", urlencode(&args.address), chain);
            let resp = public_get(&path, key.as_deref()).await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 2: get_coin_holders
// ============================================================================
pub(crate) struct GetCoinHolders;
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCoinHoldersArgs {
    pub address: String,
    #[serde(default)]
    pub chain_id: Option<i64>,
    #[serde(default)]
    pub count: Option<i64>,
    #[serde(default)]
    pub api_key: Option<String>,
}
impl DynAomiTool for GetCoinHolders {
    type App = ZoraApp;
    type Args = GetCoinHoldersArgs;
    const NAME: &'static str = "zora_get_coin_holders";
    const DESCRIPTION: &'static str =
        "Get top holders of a Zora coin. Useful to gauge concentration before trading.";

    fn run(_app: &ZoraApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let key = resolve_key(&ctx, args.api_key.as_deref());
        rt()?.block_on(async move {
            let chain = args.chain_id.unwrap_or(8453);
            let mut path = format!(
                "/coinHolders?address={}&chainId={}",
                urlencode(&args.address),
                chain
            );
            if let Some(count) = args.count {
                path.push_str(&format!("&count={count}"));
            }
            let resp = public_get(&path, key.as_deref()).await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 3: get_coin_price_history
// ============================================================================
pub(crate) struct GetCoinPriceHistory;
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCoinPriceHistoryArgs {
    pub address: String,
    #[serde(default)]
    pub chain: Option<i64>,
    #[serde(default)]
    pub api_key: Option<String>,
}
impl DynAomiTool for GetCoinPriceHistory {
    type App = ZoraApp;
    type Args = GetCoinPriceHistoryArgs;
    const NAME: &'static str = "zora_get_coin_price_history";
    const DESCRIPTION: &'static str = "Price + volume history for a Zora coin. Use when the user asks for a chart or wants momentum.";

    fn run(_app: &ZoraApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let key = resolve_key(&ctx, args.api_key.as_deref());
        rt()?.block_on(async move {
            let chain = args.chain.unwrap_or(8453);
            let path = format!(
                "/coinPriceHistory?address={}&chain={}",
                urlencode(&args.address),
                chain
            );
            let resp = public_get(&path, key.as_deref()).await?;
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 4: get_trends_by_name
// ============================================================================
pub(crate) struct GetTrendsByName;
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTrendsByNameArgs {
    pub name: String,
    #[serde(default)]
    pub first: Option<i64>,
    #[serde(default)]
    pub api_key: Option<String>,
}
impl DynAomiTool for GetTrendsByName {
    type App = ZoraApp;
    type Args = GetTrendsByNameArgs;
    const NAME: &'static str = "zora_get_trends_by_name";
    const DESCRIPTION: &'static str = "Search trending Zora creator/content coins by name. Use to discover what's hot or find a coin matching a term.";

    fn run(_app: &ZoraApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let key = resolve_key(&ctx, args.api_key.as_deref());
        rt()?.block_on(async move {
            // Cap at 10 by default — the raw response per coin is ~3KB and
            // history/sidebar truncation chops off contract addresses past
            // turn boundaries if we forward all 100+ hits.
            let first = args.first.unwrap_or(10);
            let path = format!("/trendsByName?name={}&first={first}", urlencode(&args.name));
            let resp = public_get(&path, key.as_deref()).await?;
            // Project each node down to the ~12 fields a downstream tool
            // actually needs (address, market data, creator handle). Keeps
            // the contract addresses visible after storage truncation.
            ok(compact_trends(resp))
        })
    }
}

/// Whittle the trendsByName response down to the bare essentials. Leaves
/// `pageInfo` + `count` alone for pagination, and per-edge keeps only the
/// fields a follow-up call ever needs to read.
fn compact_trends(mut value: Value) -> Value {
    let Some(trends) = value.get_mut("trendsByName").and_then(Value::as_object_mut) else {
        return value;
    };
    let Some(edges) = trends.get_mut("edges").and_then(Value::as_array_mut) else {
        return value;
    };
    for edge in edges.iter_mut() {
        let Some(node) = edge.get_mut("node").and_then(Value::as_object_mut) else {
            continue;
        };
        let creator_handle = node
            .get("creatorProfile")
            .and_then(|p| p.get("handle"))
            .cloned();
        let price_usd = node
            .get("tokenPrice")
            .and_then(|p| p.get("priceInUsdc"))
            .cloned();
        let keep = [
            "address",
            "chainId",
            "coinType",
            "name",
            "symbol",
            "marketCap",
            "marketCapDelta24h",
            "volume24h",
            "totalVolume",
            "uniqueHolders",
            "createdAt",
            "creatorAddress",
        ];
        node.retain(|k, _| keep.contains(&k.as_str()));
        if let Some(h) = creator_handle {
            node.insert("creatorHandle".into(), h);
        }
        if let Some(p) = price_usd {
            node.insert("priceUsd".into(), p);
        }
    }
    value
}

// ============================================================================
// Tool 5: get_featured_creators
// ============================================================================
pub(crate) struct GetFeaturedCreators;
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetFeaturedCreatorsArgs {
    #[serde(default)]
    pub first: Option<i64>,
    #[serde(default)]
    pub api_key: Option<String>,
}
impl DynAomiTool for GetFeaturedCreators {
    type App = ZoraApp;
    type Args = GetFeaturedCreatorsArgs;
    const NAME: &'static str = "zora_get_featured_creators";
    const DESCRIPTION: &'static str = "Current featured/top creators on Zora. Use when the user wants to discover noteworthy creator-coin issuers.";

    fn run(_app: &ZoraApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let key = resolve_key(&ctx, args.api_key.as_deref());
        rt()?.block_on(async move {
            let mut path = String::from("/featuredCreators");
            if let Some(first) = args.first {
                path.push_str(&format!("?first={first}"));
            }
            let mut resp = public_get(&path, key.as_deref()).await?;
            // Live API renamed the wrapper key:
            //   spec:  { "featuredCreators": { edges: [...] } }
            //   live:  { "traderLeaderboardFeaturedCreators": { edges: [...] } }
            // Surface both names so the LLM reads a sensible structure
            // regardless of which one it remembers from the preamble.
            if let Value::Object(map) = &mut resp
                && !map.contains_key("featuredCreators")
                && let Some(v) = map.remove("traderLeaderboardFeaturedCreators")
            {
                map.insert("featuredCreators".into(), v);
            }
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 6: get_profile
// ============================================================================
pub(crate) struct GetProfile;
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetProfileArgs {
    /// Handle, address, or profile ID.
    pub identifier: String,
    #[serde(default)]
    pub api_key: Option<String>,
}
impl DynAomiTool for GetProfile {
    type App = ZoraApp;
    type Args = GetProfileArgs;
    const NAME: &'static str = "zora_get_profile";
    const DESCRIPTION: &'static str =
        "Get a full Zora profile by handle, address, or profile ID — bio, stats, deployed coins.";

    fn run(_app: &ZoraApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let key = resolve_key(&ctx, args.api_key.as_deref());
        rt()?.block_on(async move {
            let path = format!("/profile?identifier={}", urlencode(&args.identifier));
            let resp = public_get(&path, key.as_deref()).await?;
            ok(resp)
        })
    }
}
