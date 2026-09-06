//! Curated tool layer for GMX v2 public API. Hand-written from the
//! progenitor-generated client at `aomi_ext::gmx::Client` — see
//! ext/specs/gmx.yaml.
//!
//! GMX exposes the same surface on two hosts (Arbitrum + Avalanche). Each tool
//! constructs a `Client` against the host that matches the requested chain.

use aomi_ext::gmx::Client as GmxClient;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub(crate) struct GmxApp;

const ARBITRUM_API: &str = "https://arbitrum-api.gmxinfra.io";
const AVALANCHE_API: &str = "https://avalanche-api.gmxinfra.io";

// Account data (positions, orders) moved off the oracle hosts to the GMX API
// (`https://{chain}.gmxapi.io/v1`, `address` query param). The oracle hosts
// now answer `/positions` and `/orders` with 500 / timeouts for every account.
const ARBITRUM_ACCOUNT_API: &str = "https://arbitrum.gmxapi.io/v1";
const AVALANCHE_ACCOUNT_API: &str = "https://avalanche.gmxapi.io/v1";

fn account_api_for(chain: Option<&str>) -> String {
    match chain.map(|s| s.to_lowercase()).as_deref() {
        Some("avalanche") | Some("avax") => std::env::var("GMX_AVALANCHE_ACCOUNT_API_ENDPOINT")
            .unwrap_or_else(|_| AVALANCHE_ACCOUNT_API.to_string()),
        _ => std::env::var("GMX_ARBITRUM_ACCOUNT_API_ENDPOINT")
            .unwrap_or_else(|_| ARBITRUM_ACCOUNT_API.to_string()),
    }
}

/// `GET {account_api}/{resource}?address=<account>` on the GMX API. Returns the
/// JSON array as-is (one object per position / order).
async fn fetch_account_resource(
    base: &str,
    resource: &str,
    account: &str,
) -> Result<Value, String> {
    let url = format!("{base}/{resource}");
    let response = reqwest::Client::new()
        .get(&url)
        .query(&[("address", account)])
        .send()
        .await
        .map_err(|e| format!("[gmx] {resource} {account}: request failed: {e}"))?;
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        let snippet: String = body.chars().take(200).collect();
        return Err(format!(
            "[gmx] {resource} {account}: HTTP {status}: {snippet}"
        ));
    }
    serde_json::from_str(&body)
        .map_err(|e| format!("[gmx] {resource} {account}: invalid JSON: {e}"))
}

// ============================================================================
// Helpers
// ============================================================================

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[gmx] runtime: {e}"))
}

fn resolve_chain_label(chain: Option<&str>) -> &'static str {
    match chain.map(|s| s.to_lowercase()).as_deref() {
        Some("avalanche") | Some("avax") => "avalanche",
        _ => "arbitrum",
    }
}

fn base_url_for(chain: Option<&str>) -> String {
    match chain.map(|s| s.to_lowercase()).as_deref() {
        Some("avalanche") | Some("avax") => std::env::var("GMX_AVALANCHE_API_ENDPOINT")
            .unwrap_or_else(|_| AVALANCHE_API.to_string()),
        _ => {
            std::env::var("GMX_ARBITRUM_API_ENDPOINT").unwrap_or_else(|_| ARBITRUM_API.to_string())
        }
    }
}

fn ok<T: Serialize>(value: T, chain: &str) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[gmx] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("gmx".to_string()));
            map.insert("chain".to_string(), Value::String(chain.to_string()));
            Value::Object(map)
        }
        other => json!({ "source": "gmx", "chain": chain, "data": other }),
    })
}

// ============================================================================
// Tool: GetGmxPrices
// ============================================================================

pub(crate) struct GetGmxPrices;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxPricesArgs {
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

impl DynAomiTool for GetGmxPrices {
    type App = GmxApp;
    type Args = GetGmxPricesArgs;
    const NAME: &'static str = "get_gmx_prices";
    const DESCRIPTION: &'static str = "Use when the user asks the current price of any token tradable on GMX (BTC, ETH, ARB, AVAX, etc.) or wants to compare GMX's oracle to other venues. Returns min/max oracle prices and the address+symbol for every listed token on the chosen chain. Defaults to Arbitrum.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let base = base_url_for(args.chain.as_deref());
        rt()?.block_on(async move {
            let client = GmxClient::new(&base);
            let tickers = client
                .get_prices()
                .await
                .map_err(|e| format!("[gmx] prices: {e}"))?
                .into_inner();
            ok(json!({ "tickers": tickers }), chain)
        })
    }
}

// ============================================================================
// Tool: GetGmxSignedPrices
// ============================================================================

pub(crate) struct GetGmxSignedPrices;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxSignedPricesArgs {
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

impl DynAomiTool for GetGmxSignedPrices {
    type App = GmxApp;
    type Args = GetGmxSignedPricesArgs;
    const NAME: &'static str = "get_gmx_signed_prices";
    const DESCRIPTION: &'static str = "Use only when the user specifically asks about keeper-signed oracle prices, EIP-712 price payloads, or what prices GMX will use for on-chain settlement. Returns the latest signed price packets from GMX keepers (with signatures and validity windows). For ordinary 'what is the price' questions use get_gmx_prices instead.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let base = base_url_for(args.chain.as_deref());
        rt()?.block_on(async move {
            let client = GmxClient::new(&base);
            let resp = client
                .get_signed_prices()
                .await
                .map_err(|e| format!("[gmx] signed prices: {e}"))?
                .into_inner();
            ok(resp, chain)
        })
    }
}

// ============================================================================
// Tool: GetGmxMarkets
// ============================================================================

pub(crate) struct GetGmxMarkets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxMarketsArgs {
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

impl DynAomiTool for GetGmxMarkets {
    type App = GmxApp;
    type Args = GetGmxMarketsArgs;
    const NAME: &'static str = "get_gmx_markets";
    const DESCRIPTION: &'static str = "Use when the user asks what markets exist on GMX v2, GM pool composition, current funding rates, open interest skew, borrow rates, or pool TVL. Returns every GM market on the chosen chain with market address, index/long/short tokens, funding and borrow rates, and long/short OI. Defaults to Arbitrum.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let base = base_url_for(args.chain.as_deref());
        rt()?.block_on(async move {
            let client = GmxClient::new(&base);
            let resp = client
                .get_markets()
                .await
                .map_err(|e| format!("[gmx] markets: {e}"))?
                .into_inner();
            ok(resp, chain)
        })
    }
}

// ============================================================================
// Tool: GetGmxPositions
// ============================================================================

pub(crate) struct GetGmxPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxPositionsArgs {
    /// Ethereum address of the account to query positions for (e.g. "0x1234...")
    pub(crate) account: String,
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

impl DynAomiTool for GetGmxPositions {
    type App = GmxApp;
    type Args = GetGmxPositionsArgs;
    const NAME: &'static str = "get_gmx_positions";
    const DESCRIPTION: &'static str = "Use when the user asks about their (or another address's) open GMX v2 perp positions, size, leverage, collateral, entry price, or unrealized PnL. Returns one entry per open position with market, side, size, collateral, entry price, and PnL. Pass the EVM address (0x...). Defaults to Arbitrum.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let base = account_api_for(args.chain.as_deref());
        let account = args.account.clone();
        rt()?.block_on(async move {
            let positions = fetch_account_resource(&base, "positions", &account).await?;
            let count = positions.as_array().map(Vec::len).unwrap_or(0);
            ok(
                json!({ "account": account, "count": count, "positions": positions }),
                chain,
            )
        })
    }
}

// ============================================================================
// Tool: GetGmxOrders
// ============================================================================

pub(crate) struct GetGmxOrders;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxOrdersArgs {
    /// Ethereum address of the account to query orders for (e.g. "0x1234...")
    pub(crate) account: String,
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

impl DynAomiTool for GetGmxOrders {
    type App = GmxApp;
    type Args = GetGmxOrdersArgs;
    const NAME: &'static str = "get_gmx_orders";
    const DESCRIPTION: &'static str = "Use when the user asks about their pending GMX v2 orders -- limit entries, take-profit, stop-loss, or any trigger order awaiting keeper execution. Returns one entry per pending order with order type, market, size delta, trigger price, and acceptable price. Pass the EVM address (0x...). Defaults to Arbitrum.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let base = account_api_for(args.chain.as_deref());
        let account = args.account.clone();
        rt()?.block_on(async move {
            let orders = fetch_account_resource(&base, "orders", &account).await?;
            let count = orders.as_array().map(Vec::len).unwrap_or(0);
            ok(
                json!({ "account": account, "count": count, "orders": orders }),
                chain,
            )
        })
    }
}
