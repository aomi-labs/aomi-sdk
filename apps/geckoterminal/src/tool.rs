//! Curated tool layer for GeckoTerminal — DEX pool analytics across 200+
//! networks. Hand-written from the progenitor-generated client in
//! `src/client/` (spec: `apps/geckoterminal/openapi.yaml`).
//!
//! Designed for the user story: track and analyze DeFi liquidity pools —
//! find trending pools, inspect a pool's price, volume, and recent trades
//! across networks.
//!
//! 11 curated tools (fully public API, no auth):
//!   * geckoterminal_list_networks       — network / DEX ID discovery
//!   * geckoterminal_get_trending_pools  — trending pools, global or per network
//!   * geckoterminal_get_top_pools       — top pools by network (optionally one DEX)
//!   * geckoterminal_get_new_pools       — freshly created pools
//!   * geckoterminal_search_pools        — find a pool by token symbol/name/address
//!   * geckoterminal_get_pool            — full stats for one or more pools
//!   * geckoterminal_get_pool_ohlcv      — OHLCV price/volume history for a pool
//!   * geckoterminal_get_pool_trades     — last 24h trades in a pool
//!   * geckoterminal_get_token           — token profile + its top pools (composite)
//!   * geckoterminal_get_token_price     — batch spot prices by token address
//!   * geckoterminal_check_token_safety  — honeypot / rug-risk signals for a token

use crate::client::Client as GenClient;
use crate::client::types::{
    GetnetworksNetworkDexesDexPoolsSort, GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency,
    GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe, GetnetworksNetworkPoolsSort,
    GetnetworksNetworkTokensTokenaddressPoolsSort, GetnetworksNetworkTrendingpoolsDuration,
    GetnetworksTrendingpoolsDuration,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub(crate) struct GeckoterminalApp;

/// The spec's `servers` entry is the relative `/api/v2`; the live host is
/// api.geckoterminal.com.
const BASE_URL: &str = "https://api.geckoterminal.com/api/v2";

/// Attributes to hydrate on pool list responses so the LLM sees token
/// symbols and DEX names instead of bare relationship IDs.
const POOL_INCLUDE: &str = "base_token,quote_token,dex";

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value =
        serde_json::to_value(value).map_err(|e| format!("[geckoterminal] serialize: {e}"))?;
    Ok(match value {
        Value::Object(mut m) => {
            m.insert("source".into(), Value::String("geckoterminal".into()));
            Value::Object(m)
        }
        other => json!({ "source": "geckoterminal", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[geckoterminal] runtime: {e}"))
}

fn client() -> GenClient {
    GenClient::new(BASE_URL)
}

fn invalid_duration(s: &str) -> String {
    format!("[geckoterminal] invalid duration '{s}'; use one of: 5m, 1h, 6h, 24h")
}

fn invalid_sort(s: &str) -> String {
    format!("[geckoterminal] invalid sort '{s}'; use 'volume' or 'transactions'")
}

// ============================================================================
// Tool 1: ListNetworks — GET /networks, GET /networks/{network}/dexes
// ============================================================================

pub(crate) struct ListNetworks;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ListNetworksArgs {
    /// Omit to list all supported networks (their IDs are used by every other
    /// tool, e.g. "eth", "base", "solana", "bsc"). Provide a network ID to
    /// instead list the DEXes on that network (DEX IDs like "uniswap_v3").
    pub network: Option<String>,
    /// Results page, starting at 1 (default 1).
    pub page: Option<i64>,
}

impl DynAomiTool for ListNetworks {
    type App = GeckoterminalApp;
    type Args = ListNetworksArgs;
    const NAME: &'static str = "geckoterminal_list_networks";
    const DESCRIPTION: &'static str = "Use to discover valid IDs before other calls: with no arguments lists all supported networks and their IDs (eth, base, solana, …); with a network ID lists that network's DEXes (uniswap_v3, aerodrome-base, …). Only needed when unsure of an ID.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            match args.network.as_deref() {
                Some(network) => {
                    let resp = client()
                        .getnetworks_network_dexes(network, args.page)
                        .await
                        .map_err(|e| format!("[geckoterminal] list dexes: {e}"))?
                        .into_inner();
                    ok(resp)
                }
                None => {
                    let resp = client()
                        .getnetworks(args.page)
                        .await
                        .map_err(|e| format!("[geckoterminal] list networks: {e}"))?
                        .into_inner();
                    ok(resp)
                }
            }
        })
    }
}

// ============================================================================
// Tool 2: GetTrendingPools — GET /networks/trending_pools,
//                            GET /networks/{network}/trending_pools
// ============================================================================

pub(crate) struct GetTrendingPools;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTrendingPoolsArgs {
    /// Network ID (e.g. "eth", "base", "solana"). Omit for trending pools
    /// across all networks.
    pub network: Option<String>,
    /// Trending window: "5m", "1h", "6h", or "24h" (default "24h").
    pub duration: Option<String>,
    /// Results page, starting at 1 (default 1, max 10; 20 pools per page).
    pub page: Option<i64>,
}

impl DynAomiTool for GetTrendingPools {
    type App = GeckoterminalApp;
    type Args = GetTrendingPoolsArgs;
    const NAME: &'static str = "geckoterminal_get_trending_pools";
    const DESCRIPTION: &'static str = "Use when the user asks what's trending or hot right now. Returns the currently trending DEX pools — across all networks, or on one network — with price, 24h volume, liquidity, and price-change stats. Duration picks the trending window (5m/1h/6h/24h).";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            match args.network.as_deref() {
                Some(network) => {
                    let duration: GetnetworksNetworkTrendingpoolsDuration = args
                        .duration
                        .as_deref()
                        .unwrap_or("24h")
                        .parse()
                        .map_err(|_| invalid_duration(args.duration.as_deref().unwrap_or("")))?;
                    let resp = client()
                        .getnetworks_network_trendingpools(
                            network,
                            Some(duration),
                            Some(POOL_INCLUDE),
                            None,
                            args.page,
                        )
                        .await
                        .map_err(|e| format!("[geckoterminal] trending pools: {e}"))?
                        .into_inner();
                    ok(resp)
                }
                None => {
                    let duration: GetnetworksTrendingpoolsDuration = args
                        .duration
                        .as_deref()
                        .unwrap_or("24h")
                        .parse()
                        .map_err(|_| invalid_duration(args.duration.as_deref().unwrap_or("")))?;
                    let resp = client()
                        .getnetworks_trendingpools(
                            Some(duration),
                            Some("base_token,quote_token,dex,network"),
                            None,
                            args.page,
                        )
                        .await
                        .map_err(|e| format!("[geckoterminal] trending pools: {e}"))?
                        .into_inner();
                    ok(resp)
                }
            }
        })
    }
}

// ============================================================================
// Tool 3: GetTopPools — GET /networks/{network}/pools,
//                       GET /networks/{network}/dexes/{dex}/pools
// ============================================================================

pub(crate) struct GetTopPools;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTopPoolsArgs {
    /// Network ID (e.g. "eth", "base", "solana").
    pub network: String,
    /// DEX ID (e.g. "uniswap_v3") to rank pools on a single exchange.
    /// Omit to rank across the whole network.
    pub dex: Option<String>,
    /// Ranking: "volume" (24h USD volume, default) or "transactions" (24h tx count).
    pub sort: Option<String>,
    /// Results page, starting at 1 (default 1, max 10; 20 pools per page).
    pub page: Option<i64>,
}

impl DynAomiTool for GetTopPools {
    type App = GeckoterminalApp;
    type Args = GetTopPoolsArgs;
    const NAME: &'static str = "geckoterminal_get_top_pools";
    const DESCRIPTION: &'static str = "Use when the user wants the biggest/most active pools on a network (or one DEX on that network), ranked by 24h volume or transaction count. Returns pool price, volume, liquidity, and price-change stats.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let sort_key = args.sort.as_deref().unwrap_or("volume");
            match args.dex.as_deref() {
                Some(dex) => {
                    let sort = match sort_key {
                        "volume" => GetnetworksNetworkDexesDexPoolsSort::H24VolumeUsdDesc,
                        "transactions" => GetnetworksNetworkDexesDexPoolsSort::H24TxCountDesc,
                        other => return Err(invalid_sort(other)),
                    };
                    let resp = client()
                        .getnetworks_network_dexes_dex_pools(
                            args.network.as_str(),
                            dex,
                            Some(POOL_INCLUDE),
                            None,
                            args.page,
                            Some(sort),
                        )
                        .await
                        .map_err(|e| format!("[geckoterminal] top pools by dex: {e}"))?
                        .into_inner();
                    ok(resp)
                }
                None => {
                    let sort = match sort_key {
                        "volume" => GetnetworksNetworkPoolsSort::H24VolumeUsdDesc,
                        "transactions" => GetnetworksNetworkPoolsSort::H24TxCountDesc,
                        other => return Err(invalid_sort(other)),
                    };
                    let resp = client()
                        .getnetworks_network_pools(
                            args.network.as_str(),
                            Some(POOL_INCLUDE),
                            None,
                            args.page,
                            Some(sort),
                        )
                        .await
                        .map_err(|e| format!("[geckoterminal] top pools: {e}"))?
                        .into_inner();
                    ok(resp)
                }
            }
        })
    }
}

// ============================================================================
// Tool 4: GetNewPools — GET /networks/new_pools,
//                       GET /networks/{network}/new_pools
// ============================================================================

pub(crate) struct GetNewPools;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetNewPoolsArgs {
    /// Network ID (e.g. "eth", "base", "solana"). Omit for new pools across
    /// all networks.
    pub network: Option<String>,
    /// Results page, starting at 1 (default 1, max 10; 20 pools per page).
    pub page: Option<i64>,
}

impl DynAomiTool for GetNewPools {
    type App = GeckoterminalApp;
    type Args = GetNewPoolsArgs;
    const NAME: &'static str = "geckoterminal_get_new_pools";
    const DESCRIPTION: &'static str = "Use when the user asks about newly created / recently launched pools — across all networks or on one network. Returns the latest pools with creation time, price, volume, and liquidity. Useful for spotting brand-new listings.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            match args.network.as_deref() {
                Some(network) => {
                    let resp = client()
                        .getnetworks_network_newpools(network, Some(POOL_INCLUDE), None, args.page)
                        .await
                        .map_err(|e| format!("[geckoterminal] new pools: {e}"))?
                        .into_inner();
                    ok(resp)
                }
                None => {
                    let resp = client()
                        .getnetworks_newpools(
                            Some("base_token,quote_token,dex,network"),
                            None,
                            args.page,
                        )
                        .await
                        .map_err(|e| format!("[geckoterminal] new pools: {e}"))?
                        .into_inner();
                    ok(resp)
                }
            }
        })
    }
}

// ============================================================================
// Tool 5: SearchPools — GET /search/pools
// ============================================================================

pub(crate) struct SearchPools;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchPoolsArgs {
    /// Search text: a token symbol ("PEPE"), pair ("ETH/USDC"), pool address,
    /// or token address.
    pub query: String,
    /// Network ID to restrict results (e.g. "eth", "base"). Omit to search
    /// all networks.
    pub network: Option<String>,
    /// Results page, starting at 1 (default 1).
    pub page: Option<i64>,
}

impl DynAomiTool for SearchPools {
    type App = GeckoterminalApp;
    type Args = SearchPoolsArgs;
    const NAME: &'static str = "geckoterminal_search_pools";
    const DESCRIPTION: &'static str = "Use when the user names a token or pair but you don't have a pool address yet — e.g. 'the PEPE/WETH pool on Ethereum'. Searches pools by token symbol, pair name, or address and returns matching pools with their addresses for follow-up calls.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let resp = client()
                .getsearch_pools(
                    Some(POOL_INCLUDE),
                    args.network.as_deref(),
                    args.page,
                    Some(args.query.as_str()),
                )
                .await
                .map_err(|e| format!("[geckoterminal] search pools: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 6: GetPool — GET /networks/{network}/pools/{address},
//                   GET /networks/{network}/pools/multi/{addresses}
// ============================================================================

pub(crate) struct GetPool;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPoolArgs {
    /// Network ID the pool lives on (e.g. "eth", "base", "solana").
    pub network: String,
    /// Pool contract address. Comma-separate up to 30 addresses to fetch
    /// several pools in one call.
    pub pool_addresses: String,
}

impl DynAomiTool for GetPool {
    type App = GeckoterminalApp;
    type Args = GetPoolArgs;
    const NAME: &'static str = "geckoterminal_get_pool";
    const DESCRIPTION: &'static str = "Use when the user wants full stats for a specific pool (or a known set of pools): current token prices, 24h/6h/1h volume with buy/sell breakdown, liquidity (reserve USD), FDV/market cap, price-change percentages, and transaction counts. Requires the pool address — use geckoterminal_search_pools first if you only know the token.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            if args.pool_addresses.contains(',') {
                let resp = client()
                    .getnetworks_network_pools_multi_addresses(
                        args.network.as_str(),
                        args.pool_addresses.as_str(),
                        Some(POOL_INCLUDE),
                        None,
                        Some(true),
                    )
                    .await
                    .map_err(|e| format!("[geckoterminal] get pools: {e}"))?
                    .into_inner();
                ok(resp)
            } else {
                let resp = client()
                    .getnetworks_network_pools_address(
                        args.network.as_str(),
                        args.pool_addresses.as_str(),
                        Some(POOL_INCLUDE),
                        None,
                        Some(true),
                    )
                    .await
                    .map_err(|e| format!("[geckoterminal] get pool: {e}"))?
                    .into_inner();
                ok(resp)
            }
        })
    }
}

// ============================================================================
// Tool 7: GetPoolOhlcv — GET /networks/{network}/pools/{pool_address}/ohlcv/{timeframe}
// ============================================================================

pub(crate) struct GetPoolOhlcv;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPoolOhlcvArgs {
    /// Network ID the pool lives on (e.g. "eth", "base", "solana").
    pub network: String,
    /// Pool contract address.
    pub pool_address: String,
    /// Candle timeframe: "day" (default), "hour", or "minute".
    pub timeframe: Option<String>,
    /// Periods aggregated per candle. Valid: day → 1; hour → 1, 4, 12;
    /// minute → 1, 5, 15. Default 1.
    pub aggregate: Option<String>,
    /// Number of candles to return (default 100, max 1000).
    pub limit: Option<i64>,
}

impl DynAomiTool for GetPoolOhlcv {
    type App = GeckoterminalApp;
    type Args = GetPoolOhlcvArgs;
    const NAME: &'static str = "geckoterminal_get_pool_ohlcv";
    const DESCRIPTION: &'static str = "Use when the user asks about a pool's price or volume history / chart / trend — e.g. 'how has it moved this week'. Returns OHLCV candles (open, high, low, close, volume in USD) for the pool's base token, newest first.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let timeframe_key = args.timeframe.as_deref().unwrap_or("day");
            let timeframe: GetnetworksNetworkPoolsPooladdressOhlcvTimeframeTimeframe =
                timeframe_key.parse().map_err(|_| {
                    format!(
                        "[geckoterminal] invalid timeframe '{timeframe_key}'; use day, hour, or minute"
                    )
                })?;
            let resp = client()
                .getnetworks_network_pools_pooladdress_ohlcv_timeframe(
                    args.network.as_str(),
                    args.pool_address.as_str(),
                    timeframe,
                    args.aggregate.as_deref(),
                    None,
                    Some(GetnetworksNetworkPoolsPooladdressOhlcvTimeframeCurrency::Usd),
                    None,
                    args.limit,
                    None,
                )
                .await
                .map_err(|e| format!("[geckoterminal] pool ohlcv: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 8: GetPoolTrades — GET /networks/{network}/pools/{pool_address}/trades
// ============================================================================

pub(crate) struct GetPoolTrades;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPoolTradesArgs {
    /// Network ID the pool lives on (e.g. "eth", "base", "solana").
    pub network: String,
    /// Pool contract address.
    pub pool_address: String,
    /// Only return trades larger than this USD volume (e.g. 10000 for whale
    /// trades). Omit for all trades.
    pub min_volume_usd: Option<f64>,
}

impl DynAomiTool for GetPoolTrades {
    type App = GeckoterminalApp;
    type Args = GetPoolTradesArgs;
    const NAME: &'static str = "geckoterminal_get_pool_trades";
    const DESCRIPTION: &'static str = "Use when the user wants recent trading activity in a pool — buys/sells, sizes, prices, or whale trades. Returns the last 300 trades from the past 24 hours (tx hash, side, amounts, USD volume, price). Filter with min_volume_usd to surface only large trades.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let resp = client()
                .getnetworks_network_pools_pooladdress_trades(
                    args.network.as_str(),
                    args.pool_address.as_str(),
                    None,
                    args.min_volume_usd,
                )
                .await
                .map_err(|e| format!("[geckoterminal] pool trades: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 9: GetToken — composite:
//   GET /networks/{network}/tokens/{address}
//   GET /networks/{network}/tokens/{token_address}/pools
// ============================================================================

pub(crate) struct GetToken;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTokenArgs {
    /// Network ID the token lives on (e.g. "eth", "base", "solana").
    pub network: String,
    /// Token contract address.
    pub token_address: String,
}

impl DynAomiTool for GetToken {
    type App = GeckoterminalApp;
    type Args = GetTokenArgs;
    const NAME: &'static str = "geckoterminal_get_token";
    const DESCRIPTION: &'static str = "Use when the user asks about a token (not a specific pool): 'tell me about PEPE on Ethereum', 'where does it trade'. Returns the token's profile (price, FDV, market cap, total 24h volume) plus its top pools ranked by 24h volume, in one call.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let c = client();
            let token = c
                .getnetworks_network_tokens_address(
                    args.network.as_str(),
                    args.token_address.as_str(),
                    None,
                    None,
                    None,
                )
                .await
                .map_err(|e| format!("[geckoterminal] token data: {e}"))?
                .into_inner();
            let top_pools = c
                .getnetworks_network_tokens_tokenaddress_pools(
                    args.network.as_str(),
                    args.token_address.as_str(),
                    Some(POOL_INCLUDE),
                    None,
                    None,
                    Some(GetnetworksNetworkTokensTokenaddressPoolsSort::H24VolumeUsdDesc),
                )
                .await
                .map_err(|e| format!("[geckoterminal] token top pools: {e}"))?
                .into_inner();
            ok(json!({ "token": token, "top_pools": top_pools }))
        })
    }
}

// ============================================================================
// Tool 10: GetTokenPrice — GET /simple/networks/{network}/token_price/{addresses}
// ============================================================================

pub(crate) struct GetTokenPrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTokenPriceArgs {
    /// Network ID the tokens live on (e.g. "eth", "base", "solana").
    pub network: String,
    /// Token contract address. Comma-separate up to 30 addresses to price
    /// several tokens in one call.
    pub token_addresses: String,
}

impl DynAomiTool for GetTokenPrice {
    type App = GeckoterminalApp;
    type Args = GetTokenPriceArgs;
    const NAME: &'static str = "geckoterminal_get_token_price";
    const DESCRIPTION: &'static str = "Use when the user just wants current USD prices for one or more tokens by contract address (up to 30 at once). Returns spot price, 24h price change, 24h volume, and market cap (FDV fallback) per token. Lighter than geckoterminal_get_token when only prices matter.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let resp = client()
                .getsimple_networks_network_tokenprice_addresses(
                    args.network.as_str(),
                    args.token_addresses.as_str(),
                    Some(true),
                    Some(true),
                    None,
                    Some(true),
                    Some(true),
                    Some(true),
                )
                .await
                .map_err(|e| format!("[geckoterminal] token price: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// Tool 11: CheckTokenSafety — GET /networks/{network}/tokens/{address}/info
// ============================================================================

pub(crate) struct CheckTokenSafety;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CheckTokenSafetyArgs {
    /// Network ID the token lives on (e.g. "eth", "base", "solana").
    pub network: String,
    /// Token contract address.
    pub token_address: String,
}

impl DynAomiTool for CheckTokenSafety {
    type App = GeckoterminalApp;
    type Args = CheckTokenSafetyArgs;
    const NAME: &'static str = "geckoterminal_check_token_safety";
    const DESCRIPTION: &'static str = "Use when the user asks whether a token is safe, legit, or a possible rug/honeypot — especially before acting on a new or trending pool. Returns GeckoTerminal's trust signals: GT score (with breakdown), honeypot flag, holder count and top-holder concentration, mint/freeze authority (Solana), developer holding %, plus project description and socials/websites. Data is sourced on-chain and not vetted; present it as risk signals, not a verdict.";

    fn run(
        _app: &GeckoterminalApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let resp = client()
                .getnetworks_network_tokens_address_info(
                    args.network.as_str(),
                    args.token_address.as_str(),
                )
                .await
                .map_err(|e| format!("[geckoterminal] token safety info: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}
