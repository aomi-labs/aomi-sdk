//! Curated tool layer for the Cambrian DeFi data API.
//!
//! Eleven read-only tools over ~80 upstream endpoints. Each tool resolves the
//! chain (`base` / `ethereum` / `solana`) once, picks the matching EVM or
//! Solana path, and hands back stable snake_case JSON so the model never has
//! to learn Cambrian's per-endpoint column names or its columnar envelope.
//!
//!   * `cambrian_search_tokens`        — symbol/name → EVM token addresses
//!   * `cambrian_get_token_price`      — current USD price, any chain
//!   * `cambrian_get_price_history`    — hourly / interval price series
//!   * `cambrian_get_token_stats`      — Solana price + volume + trades + holders
//!   * `cambrian_trending_tokens`      — Solana movers by change / volume / price
//!   * `cambrian_find_pools`           — pools for a token (per EVM DEX, all Solana DEXes)
//!   * `cambrian_get_pool_stats`       — TVL / volume / fee APR for pool addresses
//!   * `cambrian_find_lending_yields`  — ranked lending pools & vaults (EVM)
//!   * `cambrian_get_wallet_holdings`  — token balances with USD value
//!   * `cambrian_get_top_holders`      — largest holders of a token
//!   * `cambrian_raw_get`              — any documented GET path (escape hatch)

use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

// ============================================================================
// Shared helpers
// ============================================================================

fn q(key: &'static str, value: impl ToString) -> (&'static str, String) {
    (key, value.to_string())
}

fn normalize_address(chain: Chain, address: &str) -> String {
    let trimmed = address.trim();
    match chain {
        Chain::Solana => trimmed.to_string(),
        _ => trimmed.to_ascii_lowercase(),
    }
}

fn require_address(chain: Chain, address: &str, what: &str) -> Result<String, String> {
    let address = normalize_address(chain, address);
    if address.is_empty() {
        return Err(format!("[cambrian] {what} is required"));
    }
    if chain != Chain::Solana && !(address.starts_with("0x") && address.len() == 42) {
        return Err(format!(
            "[cambrian] {what} `{address}` is not a 0x-prefixed 42-character EVM address; use cambrian_search_tokens to resolve symbols"
        ));
    }
    Ok(address)
}

fn pct_change(from: Option<f64>, to: Option<f64>) -> Option<f64> {
    match (from, to) {
        (Some(from), Some(to)) if from != 0.0 => Some((to - from) / from * 100.0),
        _ => None,
    }
}

fn nonempty(s: Option<&str>) -> Option<&str> {
    s.map(str::trim).filter(|s| !s.is_empty())
}

// ============================================================================
// cambrian_search_tokens
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchTokensArgs {
    /// Symbol or name fragment to search, e.g. `USDC`, `cbBTC`, `Aerodrome`
    pub(crate) query: String,
    /// `base` (default) or `ethereum`. Solana has no symbol search; use the mint address directly.
    pub(crate) chain: Option<String>,
    /// Max matches to return (default 20, max 100)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct SearchTokens;

impl DynAomiTool for SearchTokens {
    type App = CambrianApp;
    type Args = SearchTokensArgs;
    const NAME: &'static str = "cambrian_search_tokens";
    const DESCRIPTION: &'static str = "Resolve a token symbol or name to ERC-20 contract addresses on Base or Ethereum. Call this before price, pool, or lending tools whenever the user gives a symbol instead of a 0x address. Not available for Solana.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = Chain::parse(args.chain.as_deref())?;
        let chain_id = chain.require_evm("token search")?;
        let query = nonempty(Some(&args.query))
            .ok_or("[cambrian] query is required")?
            .to_string();
        let limit = clamp_limit(args.limit, 20, 100);

        let client = CambrianClient::from_ctx(&ctx)?;
        let rows = client.get_rows(
            "/evm/tokens",
            &[
                q("chain_id", &chain_id),
                q("filter", &query),
                q("limit", limit),
            ],
        )?;

        let needle = query.to_ascii_lowercase();
        let mut tokens: Vec<Value> = rows
            .iter()
            .map(|row| {
                let symbol = row_str(row, &["tokenSymbol", "symbol"]).unwrap_or_default();
                let name = row_str(row, &["tokenName", "name"]).unwrap_or_default();
                let exact =
                    symbol.eq_ignore_ascii_case(&needle) || name.eq_ignore_ascii_case(&needle);
                json!({
                    "address": row_str(row, &["tokenAddress"]),
                    "symbol": symbol,
                    "name": name,
                    "decimals": row_u64(row, &["tokenDecimals", "decimals"]),
                    "is_stable": row_bool(row, &["isStable"]).unwrap_or(false),
                    "exact_match": exact,
                })
            })
            .collect();
        // Exact symbol hits first so the model picks the canonical token.
        tokens.sort_by_key(|t| !t["exact_match"].as_bool().unwrap_or(false));

        Ok(json!({
            "chain": chain.label(),
            "query": query,
            "count": tokens.len(),
            "tokens": tokens,
            "note": "Many symbols collide (copycat tokens). Prefer exact_match rows and, for major assets, the well-known addresses in the preamble.",
        }))
    }
}

// ============================================================================
// cambrian_get_token_price
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTokenPriceArgs {
    /// Token addresses: 0x contract addresses on EVM, base58 mints on Solana. Up to 10 on EVM, 50 on Solana.
    pub(crate) token_addresses: Vec<String>,
    /// `base` (default), `ethereum`, or `solana`
    pub(crate) chain: Option<String>,
}

pub(crate) struct GetTokenPrice;

impl DynAomiTool for GetTokenPrice {
    type App = CambrianApp;
    type Args = GetTokenPriceArgs;
    const NAME: &'static str = "cambrian_get_token_price";
    const DESCRIPTION: &'static str = "Get the current USD price of one or more tokens on Base, Ethereum, or Solana, derived from DEX liquidity. Pass several addresses in one call to save API quota.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = Chain::parse(args.chain.as_deref())?;
        let addresses = split_addresses(&args.token_addresses);
        if addresses.is_empty() {
            return Err("[cambrian] token_addresses must contain at least one address".to_string());
        }
        let client = CambrianClient::from_ctx(&ctx)?;

        let mut prices = Vec::new();
        let mut missing = Vec::new();

        if chain == Chain::Solana {
            if addresses.len() > 50 {
                return Err("[cambrian] at most 50 Solana mints per call".to_string());
            }
            let rows = client.get_rows(
                "/solana/price-current",
                &[q("token_addresses", addresses.join(","))],
            )?;
            for row in &rows {
                prices.push(json!({
                    "token_address": row_str(row, &["tokenAddress"]),
                    "symbol": row_str(row, &["symbol"]),
                    "price_usd": row_f64(row, &["priceUSD", "priceUsd"]),
                }));
            }
            let seen: Vec<String> = rows
                .iter()
                .filter_map(|r| row_str(r, &["tokenAddress"]))
                .collect();
            missing.extend(addresses.into_iter().filter(|a| !seen.contains(a)));
        } else {
            if addresses.len() > 10 {
                return Err("[cambrian] at most 10 EVM token addresses per call (each is one upstream request)".to_string());
            }
            let chain_id = chain.require_evm("token price")?;
            for address in addresses {
                let address = require_address(chain, &address, "token address")?;
                let rows = client.get_rows(
                    "/evm/price-current",
                    &[q("chain_id", &chain_id), q("token_address", &address)],
                )?;
                match rows.first() {
                    Some(row) => prices.push(json!({
                        "token_address": row_str(row, &["tokenAddress"]).unwrap_or(address),
                        "symbol": row_str(row, &["symbol", "tokenSymbol"]),
                        "price_usd": row_f64(row, &["priceUsd", "priceUSD"]),
                    })),
                    None => missing.push(address),
                }
            }
        }

        Ok(json!({
            "chain": chain.label(),
            "prices": prices,
            "missing": missing,
            "source": "cambrian",
        }))
    }
}

// ============================================================================
// cambrian_get_price_history
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPriceHistoryArgs {
    /// Token address (0x on EVM, base58 mint on Solana)
    pub(crate) token_address: String,
    /// `base` (default), `ethereum`, or `solana`
    pub(crate) chain: Option<String>,
    /// Bucket size. EVM supports only `1H`. Solana: `1H`, `2H`, `4H`, `6H`, `8H`, `12H`, `1D`, `3D`, `1W`, `1M`. Default `1H`.
    pub(crate) interval: Option<String>,
    /// Number of most-recent buckets to return (default 24, max 200; EVM history goes back at most 1000 hours)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct GetPriceHistory;

impl DynAomiTool for GetPriceHistory {
    type App = CambrianApp;
    type Args = GetPriceHistoryArgs;
    const NAME: &'static str = "cambrian_get_price_history";
    const DESCRIPTION: &'static str = "Get recent historical USD prices for a token as time buckets (newest first) plus a change summary. Use limit=24 with 1H for the last day, or a larger Solana interval for longer windows.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = Chain::parse(args.chain.as_deref())?;
        let address = require_address(chain, &args.token_address, "token_address")?;
        let interval = nonempty(args.interval.as_deref())
            .map(|s| s.to_ascii_uppercase())
            .unwrap_or_else(|| "1H".to_string());
        let limit = clamp_limit(args.limit, 24, MAX_ROWS);
        let client = CambrianClient::from_ctx(&ctx)?;

        let rows = if chain == Chain::Solana {
            const ALLOWED: [&str; 10] =
                ["1H", "2H", "4H", "6H", "8H", "12H", "1D", "3D", "1W", "1M"];
            if !ALLOWED.contains(&interval.as_str()) {
                return Err(format!(
                    "[cambrian] interval `{interval}` is not supported; use one of {}",
                    ALLOWED.join(", ")
                ));
            }
            client.get_rows(
                "/solana/price-hour",
                &[
                    q("token_address", &address),
                    q("interval", &interval),
                    q("limit", limit),
                ],
            )?
        } else {
            if interval != "1H" {
                return Err(
                    "[cambrian] EVM price history is hourly only; use interval `1H` and a larger limit"
                        .to_string(),
                );
            }
            let chain_id = chain.require_evm("price history")?;
            client.get_rows(
                "/evm/price-hour",
                &[
                    q("chain_id", &chain_id),
                    q("token_address", &address),
                    q("limit", limit),
                ],
            )?
        };

        let points: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "time": row_str(row, &["blockHour", "intervalStart"]),
                    "price_usd": row_f64(row, &["priceUsd", "priceUSD"]),
                })
            })
            .collect();
        let symbol = rows
            .first()
            .and_then(|r| row_str(r, &["tokenSymbol", "symbol"]));
        let latest = rows
            .first()
            .and_then(|r| row_f64(r, &["priceUsd", "priceUSD"]));
        let earliest = rows
            .last()
            .and_then(|r| row_f64(r, &["priceUsd", "priceUSD"]));

        Ok(json!({
            "chain": chain.label(),
            "token_address": address,
            "symbol": symbol,
            "interval": interval,
            "order": "newest_first",
            "count": points.len(),
            "summary": {
                "latest_price_usd": latest,
                "earliest_price_usd": earliest,
                "change_pct": pct_change(earliest, latest),
                "latest_time": rows.first().and_then(|r| row_str(r, &["blockHour", "intervalStart"])),
                "earliest_time": rows.last().and_then(|r| row_str(r, &["blockHour", "intervalStart"])),
            },
            "points": points,
        }))
    }
}

// ============================================================================
// cambrian_get_token_stats (Solana)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTokenStatsArgs {
    /// Solana mint addresses (up to 50)
    pub(crate) token_addresses: Vec<String>,
    /// Must be `solana` (default when omitted)
    pub(crate) chain: Option<String>,
}

pub(crate) struct GetTokenStats;

impl DynAomiTool for GetTokenStats {
    type App = CambrianApp;
    type Args = GetTokenStatsArgs;
    const NAME: &'static str = "cambrian_get_token_stats";
    const DESCRIPTION: &'static str = "Solana only: one-call market snapshot per mint — price, 1h/24h/7d trade counts and USD volume, 24h buy vs sell split, last trade time, and holder count. Use for 'how is token X doing' questions on Solana.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = Chain::parse(args.chain.as_deref().or(Some("solana")))?;
        chain.require_solana("token stats")?;
        let addresses = split_addresses(&args.token_addresses);
        if addresses.is_empty() || addresses.len() > 50 {
            return Err("[cambrian] pass between 1 and 50 Solana mint addresses".to_string());
        }
        let client = CambrianClient::from_ctx(&ctx)?;
        let rows = client.get_rows(
            "/solana/token-details",
            &[q("token_addresses", addresses.join(","))],
        )?;

        let tokens: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "token_address": row_str(row, &["tokenAddress"]),
                    "symbol": row_str(row, &["symbol"]),
                    "name": row_str(row, &["name"]),
                    "decimals": row_u64(row, &["decimals"]),
                    "price_usd": row_f64(row, &["priceUSD", "priceUsd"]),
                    "last_trade_time": row_str(row, &["lastTradeHumanTime"]),
                    "trades": {
                        "1h": row_u64(row, &["trade1hCount"]),
                        "24h": row_u64(row, &["trade24hCount"]),
                        "7d": row_u64(row, &["trade7dCount"]),
                    },
                    "volume_usd": {
                        "1h": row_f64(row, &["volume1hUSD"]),
                        "24h": row_f64(row, &["volume24hUSD"]),
                        "7d": row_f64(row, &["volume7dUSD"]),
                    },
                    "buy_sell_24h": {
                        "buy_count": row_u64(row, &["buy24hCount"]),
                        "sell_count": row_u64(row, &["sell24hCount"]),
                        "buy_volume_usd": row_f64(row, &["buyVolume24hUSD"]),
                        "sell_volume_usd": row_f64(row, &["sellVolume24hUSD"]),
                    },
                    "holder_count": row_u64(row, &["holderCount"]),
                })
            })
            .collect();

        Ok(json!({
            "chain": "solana",
            "count": tokens.len(),
            "tokens": tokens,
        }))
    }
}

// ============================================================================
// cambrian_trending_tokens (Solana)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct TrendingTokensArgs {
    /// Ranking: `price_change` (24h % change, default), `volume` (24h USD volume), or `price` (current USD price)
    pub(crate) order_by: Option<String>,
    /// Number of tokens (default 10, max 100)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct TrendingTokens;

impl DynAomiTool for TrendingTokens {
    type App = CambrianApp;
    type Args = TrendingTokensArgs;
    const NAME: &'static str = "cambrian_trending_tokens";
    const DESCRIPTION: &'static str = "Solana only: list trending tokens ranked by 24h price change, 24h volume, or price. Returns mint, symbol, current price, price 24h ago, % change, and 24h volume. Small-cap movers dominate the price_change ranking; warn users accordingly.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let order_by = match nonempty(args.order_by.as_deref())
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            None | Some("price_change") | Some("change") | Some("price_change_percentage") => {
                "price_change_percentage"
            }
            Some("volume") | Some("volume_24h") | Some("volume_usd_24h") => "volume_usd_24h",
            Some("price") | Some("current_price") | Some("current_price_usd") => {
                "current_price_usd"
            }
            Some(other) => {
                return Err(format!(
                    "[cambrian] unknown order_by `{other}`; use price_change, volume, or price"
                ));
            }
        };
        let limit = clamp_limit(args.limit, 10, 100);
        let client = CambrianClient::from_ctx(&ctx)?;
        let rows = client.get_rows(
            "/solana/trending-tokens",
            &[q("order_by", order_by), q("limit", limit)],
        )?;

        let tokens: Vec<Value> = rows
            .iter()
            .map(|row| {
                json!({
                    "token_address": row_str(row, &["tokenAddress"]),
                    "symbol": row_str(row, &["symbol"]),
                    "price_usd": row_f64(row, &["currentPriceUSD"]),
                    "price_24h_ago_usd": row_f64(row, &["price24hAgo"]),
                    "change_24h_pct": row_f64(row, &["priceChangePercentage"]),
                    "volume_24h_usd": row_f64(row, &["volume24hUSD"]),
                })
            })
            .collect();

        Ok(json!({
            "chain": "solana",
            "order_by": order_by,
            "count": tokens.len(),
            "tokens": tokens,
        }))
    }
}

// ============================================================================
// cambrian_find_pools
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FindPoolsArgs {
    /// Token address to find pools for (0x on EVM, base58 mint on Solana)
    pub(crate) token_address: String,
    /// `base` (default), `ethereum`, or `solana`
    pub(crate) chain: Option<String>,
    /// EVM DEX family: `uniswap` (default), `pancake`, `sushi`, `alienbase`, `clones`, or `aerodrome` (v2). On Solana this is an optional substring filter on the pool's DEX name (e.g. `orca`, `raydium`, `meteora`, `pump`).
    pub(crate) dex: Option<String>,
    /// Max pools to return (default 20, max 100)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct FindPools;

impl DynAomiTool for FindPools {
    type App = CambrianApp;
    type Args = FindPoolsArgs;
    const NAME: &'static str = "cambrian_find_pools";
    const DESCRIPTION: &'static str = "Find liquidity pools containing a token. EVM: lists pools on one DEX family (default Uniswap V3; Aerodrome v2 rows include TVL, 7d volume and APR). Solana: searches all DEXes and returns 24h volume and trade counts per pool. Follow up with cambrian_get_pool_stats for TVL/APR detail.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = Chain::parse(args.chain.as_deref())?;
        let address = require_address(chain, &args.token_address, "token_address")?;
        let limit = clamp_limit(args.limit, 20, 100);
        let client = CambrianClient::from_ctx(&ctx)?;

        if chain == Chain::Solana {
            let rows = client.get_rows(
                "/solana/token-pool-search",
                &[q("token_address", &address), q("limit", limit)],
            )?;
            let filter = nonempty(args.dex.as_deref()).map(|s| s.to_ascii_lowercase());
            let pools: Vec<Value> = rows
                .iter()
                .filter(|row| match &filter {
                    Some(f) => row_str(row, &["poolDex"])
                        .map(|d| d.to_ascii_lowercase().contains(f.as_str()))
                        .unwrap_or(false),
                    None => true,
                })
                .map(|row| {
                    json!({
                        "pool_address": row_str(row, &["poolAddress"]),
                        "dex": row_str(row, &["poolDex"]),
                        "pair": format!(
                            "{}/{}",
                            row_str(row, &["tokenSymbol"]).unwrap_or_default(),
                            row_str(row, &["poolPairToken"]).unwrap_or_default()
                        ),
                        "token_price_usd": row_f64(row, &["tokenPrice"]),
                        "volume_24h_usd": row_f64(row, &["volume24hUSD"]),
                        "trades_24h": row_u64(row, &["trades24hCount"]),
                        "buys_24h": row_u64(row, &["buys24hCount"]),
                        "sells_24h": row_u64(row, &["sells24hCount"]),
                    })
                })
                .collect();
            return Ok(json!({
                "chain": "solana",
                "token_address": address,
                "token_symbol": rows.first().and_then(|r| row_str(r, &["tokenSymbol"])),
                "count": pools.len(),
                "pools": pools,
                "note": "For TVL/APR on a pool call cambrian_get_pool_stats with dex = orca, meteora-dlmm, or raydium-clmm (other DEXes have no stats endpoint).",
            }));
        }

        let chain_id = chain.require_evm("pool search")?;
        let dex = EvmDex::parse(args.dex.as_deref())?;
        let path = dex.pools_path().ok_or_else(|| {
            format!(
                "[cambrian] {} has no pool-list endpoint; use dex = aerodrome (v2) to list, or pass a known pool address to cambrian_get_pool_stats",
                dex.label()
            )
        })?;

        let mut query = vec![q("chain_id", &chain_id)];
        if dex.pools_filter_by_token() {
            query.push(q("token_address", &address));
            query.push(q("limit", limit));
            query.push(q("order_desc", "createdAt"));
        } else {
            // Aerodrome v2 has no server-side token filter; pull a wide page and filter here.
            query.push(q("limit", 500));
        }
        let rows = client.get_rows(path, &query)?;

        let pools: Vec<Value> = rows
            .iter()
            .filter(|row| {
                dex.pools_filter_by_token() || {
                    let t0 = row_str(row, &["token0", "token0Address"]).unwrap_or_default();
                    let t1 = row_str(row, &["token1", "token1Address"]).unwrap_or_default();
                    t0.eq_ignore_ascii_case(&address) || t1.eq_ignore_ascii_case(&address)
                }
            })
            .take(limit as usize)
            .map(|row| {
                let t0s = row_str(row, &["token0Symbol"]).unwrap_or_default();
                let t1s = row_str(row, &["token1Symbol"]).unwrap_or_default();
                json!({
                    "pool_address": row_str(row, &["poolAddress", "poolId"]),
                    "dex": dex.label(),
                    "pair": format!("{t0s}/{t1s}"),
                    "token0": { "address": row_str(row, &["token0Address", "token0"]), "symbol": t0s },
                    "token1": { "address": row_str(row, &["token1Address", "token1"]), "symbol": t1s },
                    "fee_tier": row_u64(row, &["fee", "feeTier"]),
                    "created_at": row_str(row, &["createdAt"]),
                    "tvl_usd": row_f64(row, &["poolTvlUsd"]),
                    "volume_24h_usd": row_f64(row, &["volume24hUsd"]),
                    "volume_7d_usd": row_f64(row, &["volume7dUsd"]),
                    "fee_apr_7d": row_f64(row, &["swapFeeApr7d"]),
                    "total_apr_7d": row_f64(row, &["totalApr7d"]),
                })
            })
            .collect();

        Ok(json!({
            "chain": chain.label(),
            "dex": dex.label(),
            "token_address": address,
            "count": pools.len(),
            "pools": pools,
            "note": if dex == EvmDex::AerodromeV2 {
                "Aerodrome v2 rows carry TVL/volume/APR directly. Null USD fields mean a pool token has no price."
            } else {
                "Uniswap-style listings carry no TVL; call cambrian_get_pool_stats on the pools you care about."
            },
        }))
    }
}

// ============================================================================
// cambrian_get_pool_stats
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPoolStatsArgs {
    /// Pool addresses. EVM: one 0x pool per call. Solana: up to 20 pools from the same DEX.
    pub(crate) pool_addresses: Vec<String>,
    /// `base` (default), `ethereum`, or `solana`
    pub(crate) chain: Option<String>,
    /// EVM: `uniswap` (default), `pancake`, `sushi`, `alienbase`, `clones`, `aerodrome` (v2), `aerodrome-v3`. Solana (required): `orca`, `meteora-dlmm`, or `raydium-clmm`.
    pub(crate) dex: Option<String>,
}

pub(crate) struct GetPoolStats;

impl DynAomiTool for GetPoolStats {
    type App = CambrianApp;
    type Args = GetPoolStatsArgs;
    const NAME: &'static str = "cambrian_get_pool_stats";
    const DESCRIPTION: &'static str = "Get live stats for specific pools: TVL, current price, swap volume, fee APR, volatility, swap and unique-user counts keyed by window (EVM: 5 minute/1 hour/1 day/1 week/1 month/1 year; Solana: 24h). Requires the pool's DEX family.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = Chain::parse(args.chain.as_deref())?;
        let addresses = split_addresses(&args.pool_addresses);
        if addresses.is_empty() {
            return Err("[cambrian] pool_addresses must contain at least one pool".to_string());
        }
        let client = CambrianClient::from_ctx(&ctx)?;

        if chain == Chain::Solana {
            let dex = SolanaDex::parse(args.dex.as_deref())?;
            if addresses.len() > 20 {
                return Err("[cambrian] at most 20 Solana pools per call".to_string());
            }
            let rows =
                client.get_rows(dex.pool_path(), &[q("pool_addresses", addresses.join(","))])?;
            let pools: Vec<Value> = rows
                .iter()
                .map(|row| {
                    let mut out = pool_core(row);
                    out.insert("tvl_usd".into(), row_value(row, &["tvl", "poolTvlUsd"]));
                    out.insert("volume_24h_usd".into(), row_value(row, &["volume24h"]));
                    out.insert("fees_24h_usd".into(), row_value(row, &["fees24h"]));
                    out.insert("fee_apr_24h".into(), row_value(row, &["apr24h"]));
                    out.insert(
                        "price_volatility".into(),
                        row_value(row, &["priceVolatility"]),
                    );
                    out.insert("tick_spacing".into(), row_value(row, &["tickSpacing"]));
                    out.insert("fee_tier".into(), row_value(row, &["feeTier"]));
                    Value::Object(out)
                })
                .collect();
            return Ok(json!({
                "chain": "solana",
                "dex": dex.label(),
                "count": pools.len(),
                "pools": pools,
            }));
        }

        let chain_id = chain.require_evm("pool stats")?;
        let dex = EvmDex::parse(args.dex.as_deref())?;
        if addresses.len() != 1 {
            return Err(
                "[cambrian] EVM pool stats take exactly one pool_address per call".to_string(),
            );
        }
        let pool = require_address(chain, &addresses[0], "pool_address")?;
        let mut query = vec![q("chain_id", &chain_id), q("pool_address", &pool)];
        if dex == EvmDex::AerodromeV2 {
            query.push(q("apr_days_annualized", 7));
        }
        let rows = client.get_rows(dex.pool_path(), &query)?;
        let row = rows.first().ok_or_else(|| {
            format!(
                "[cambrian] no {} pool found at {pool} on {}; check the address and dex family",
                dex.label(),
                chain.label()
            )
        })?;

        let mut out = pool_core(row);
        out.insert("pool_address".into(), Value::String(pool.clone()));
        if dex == EvmDex::AerodromeV2 {
            out.insert("tvl_usd".into(), row_value(row, &["poolTvlUsd", "tvlUsd"]));
            out.insert("volume_24h_usd".into(), row_value(row, &["volume24hUsd"]));
            out.insert("volume_7d_usd".into(), row_value(row, &["volume7dUsd"]));
            out.insert("fees_7d_usd".into(), row_value(row, &["fees7dUsd"]));
            out.insert("fee_apr_7d".into(), row_value(row, &["swapFeeApr7d"]));
            out.insert(
                "aero_reward_apr_7d".into(),
                row_value(row, &["aeroRewardApr7d"]),
            );
            out.insert("total_apr_7d".into(), row_value(row, &["totalApr7d"]));
            // Keep any extra columns the docs did not enumerate.
            for (k, v) in row {
                if !out.contains_key(k) && !CORE_KEYS.contains(&k.as_str()) {
                    out.insert(k.clone(), v.clone());
                }
            }
        } else {
            out.insert("fee_tier".into(), row_value(row, &["feeTier", "fee"]));
            out.insert("tick_spacing".into(), row_value(row, &["tickSpacing"]));
            out.insert("current_tick".into(), row_value(row, &["currentTick"]));
            out.insert(
                "current_price_token1_per_token0".into(),
                row_value(row, &["currentPoolPrice"]),
            );
            out.insert("tvl_usd".into(), row_value(row, &["poolTvlUsd"]));
            out.insert("swap_volume_usd".into(), row_value(row, &["swapVolumeUsd"]));
            out.insert("fee_apr".into(), row_value(row, &["feeApr"]));
            out.insert(
                "price_volatility_pct".into(),
                row_value(row, &["priceVolatilityPct"]),
            );
            out.insert("swap_count".into(), row_value(row, &["swapCount"]));
            out.insert(
                "unique_user_count".into(),
                row_value(row, &["uniqueUserCount"]),
            );
            out.insert(
                "windows".into(),
                json!(["5 minute", "1 hour", "1 day", "1 week", "1 month", "1 year"]),
            );
        }

        Ok(json!({
            "chain": chain.label(),
            "dex": dex.label(),
            "count": 1,
            "pools": [Value::Object(out)],
            "note": "APR values are fractions (0.15 = 15%). Window-keyed maps use the window name as key.",
        }))
    }
}

const CORE_KEYS: [&str; 9] = [
    "poolAddress",
    "poolId",
    "createdAt",
    "token0Address",
    "token0Symbol",
    "token1Address",
    "token1Symbol",
    "token0",
    "token1",
];

/// Fields every pool row shares regardless of DEX family.
fn pool_core(row: &Map<String, Value>) -> Map<String, Value> {
    let mut out = Map::new();
    let t0s = row_str(row, &["token0Symbol"]).unwrap_or_default();
    let t1s = row_str(row, &["token1Symbol"]).unwrap_or_default();
    out.insert(
        "pool_address".into(),
        row_value(row, &["poolAddress", "poolId"]),
    );
    out.insert("pair".into(), Value::String(format!("{t0s}/{t1s}")));
    out.insert(
        "token0".into(),
        json!({ "address": row_str(row, &["token0Address", "token0"]), "symbol": t0s, "decimals": row_u64(row, &["token0Decimals"]) }),
    );
    out.insert(
        "token1".into(),
        json!({ "address": row_str(row, &["token1Address", "token1"]), "symbol": t1s, "decimals": row_u64(row, &["token1Decimals"]) }),
    );
    out.insert("created_at".into(), row_value(row, &["createdAt"]));
    out
}

// ============================================================================
// cambrian_find_lending_yields (EVM)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FindLendingYieldsArgs {
    /// `base` (default) or `ethereum`
    pub(crate) chain: Option<String>,
    /// Underlying token 0x address to filter by (e.g. USDC on Base `0x833589fcd6edb6e08f4c7c32d4f71b54bda02913`)
    pub(crate) underlying_address: Option<String>,
    /// Protocol filter: `aave-v3`, `morpho-v1`, `morpho-v2`, `euler-lend`, or `sparklend`
    pub(crate) protocol_id: Option<String>,
    /// Minimum pool/vault TVL in USD (default 100000)
    pub(crate) min_tvl_usd: Option<u64>,
    /// `true` to return only pools where borrowing is enabled (Aave/Euler/Spark markets rather than Morpho supply vaults)
    pub(crate) borrowable: Option<bool>,
    /// Ranking: `supply_apy` (default), `net_supply_apy`, `borrow_apy`, `tvl`, `available_liquidity`, or `utilization`. Always descending.
    pub(crate) sort_by: Option<String>,
    /// Max rows (default 20, max 100)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct FindLendingYields;

impl DynAomiTool for FindLendingYields {
    type App = CambrianApp;
    type Args = FindLendingYieldsArgs;
    const NAME: &'static str = "cambrian_find_lending_yields";
    const DESCRIPTION: &'static str = "Rank lending pools and vaults (Aave V3, Morpho V1/V2, Euler, Sparklend) on Base or Ethereum by supply APY, TVL, borrow APY, liquidity, or utilization. Filter by underlying token address, protocol, minimum TVL, and borrowability. Use for 'best yield for my USDC' and 'where can I borrow X' questions.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = Chain::parse(args.chain.as_deref())?;
        let chain_id = chain.require_evm("lending data")?;
        let limit = clamp_limit(args.limit, 20, 100);
        let min_tvl = args.min_tvl_usd.unwrap_or(100_000);
        let sort_col = match nonempty(args.sort_by.as_deref())
            .map(|s| s.to_ascii_lowercase())
            .as_deref()
        {
            None | Some("supply_apy") | Some("apy") => "supplyApy",
            Some("net_supply_apy") | Some("net_apy") => "netSupplyApy",
            Some("borrow_apy") => "borrowApy",
            Some("tvl") | Some("tvl_usd") => "tvlUsd",
            Some("available_liquidity") | Some("liquidity") => "availableLiquidityUsd",
            Some("utilization") | Some("utilization_rate") => "utilizationRate",
            Some(other) => {
                return Err(format!(
                    "[cambrian] unknown sort_by `{other}`; use supply_apy, net_supply_apy, borrow_apy, tvl, available_liquidity, or utilization"
                ));
            }
        };

        let mut query = vec![
            q("chain_id", &chain_id),
            q("min_tvl_usd", min_tvl),
            q("limit", limit),
            q("order_desc", sort_col),
        ];
        if let Some(underlying) = nonempty(args.underlying_address.as_deref()) {
            query.push(q(
                "underlying_address",
                require_address(chain, underlying, "underlying_address")?,
            ));
        }
        if let Some(protocol) = nonempty(args.protocol_id.as_deref()) {
            query.push(q("protocol_id", protocol.to_ascii_lowercase()));
        }
        if let Some(borrowable) = args.borrowable {
            query.push(q("borrowable", borrowable));
        }

        let client = CambrianClient::from_ctx(&ctx)?;
        let rows = client.get_rows("/evm/lending/overview", &query)?;
        let pools: Vec<Value> = rows.iter().map(lending_row).collect();

        Ok(json!({
            "chain": chain.label(),
            "filters": {
                "underlying_address": args.underlying_address,
                "protocol_id": args.protocol_id,
                "min_tvl_usd": min_tvl,
                "borrowable": args.borrowable,
                "sort_by": sort_col,
            },
            "count": pools.len(),
            "pools": pools,
            "note": "APY fields are fractions (0.05 = 5%). Morpho/Euler vaults are supply-only (borrowable=false); Aave/Spark/Euler markets expose borrow_apy and ltv.",
        }))
    }
}

pub(crate) fn lending_row(row: &Map<String, Value>) -> Value {
    let name = row_str(row, &["vaultName"])
        .filter(|s| !s.is_empty())
        .or_else(|| row_str(row, &["symbol"]));
    json!({
        "protocol_id": row_str(row, &["protocolId"]),
        "address": row_str(row, &["address", "vaultAddress", "poolAddress"]),
        "name": name,
        "symbol": row_str(row, &["symbol"]),
        "underlying": {
            "address": row_str(row, &["underlyingAddress"]),
            "symbol": row_str(row, &["underlyingSymbol"]),
        },
        "tvl_usd": row_f64(row, &["tvlUsd"]),
        "supply_usd": row_f64(row, &["supplyUsd"]),
        "borrow_usd": row_f64(row, &["borrowUsd"]),
        "available_liquidity_usd": row_f64(row, &["availableLiquidityUsd"]),
        "remaining_capacity_usd": row_f64(row, &["remainingCapacityUsd"]),
        "supply_apy": row_f64(row, &["supplyApy"]),
        "net_supply_apy": row_f64(row, &["netSupplyApy"]),
        "borrow_apy": row_f64(row, &["borrowApy"]),
        "utilization_rate": row_f64(row, &["utilizationRate"]),
        "ltv": row_f64(row, &["ltv"]),
        "performance_fee": row_f64(row, &["performanceFee"]),
        "borrowable": row_bool(row, &["borrowable"]),
        "deposit_enabled": row_bool(row, &["depositEnabled"]),
        "updated_at": row_str(row, &["updatedAt"]),
    })
}

// ============================================================================
// cambrian_get_wallet_holdings
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetWalletHoldingsArgs {
    /// Wallet address (0x on EVM, base58 on Solana)
    pub(crate) wallet_address: String,
    /// `base` (default), `ethereum`, or `solana`
    pub(crate) chain: Option<String>,
    /// EVM only: skip tokens with no USD price (default true)
    pub(crate) priced_only: Option<bool>,
    /// Max holdings, largest USD value first (default 50, max 200)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct GetWalletHoldings;

impl DynAomiTool for GetWalletHoldings {
    type App = CambrianApp;
    type Args = GetWalletHoldingsArgs;
    const NAME: &'static str = "cambrian_get_wallet_holdings";
    const DESCRIPTION: &'static str = "List a wallet's token balances with USD values on Base, Ethereum, or Solana, plus the total. Requires the wallet address; use the connected wallet when the user says 'my wallet'.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = Chain::parse(args.chain.as_deref())?;
        let wallet = require_address(chain, &args.wallet_address, "wallet_address")?;
        let limit = clamp_limit(args.limit, 50, MAX_ROWS);
        let client = CambrianClient::from_ctx(&ctx)?;

        let holdings: Vec<Value> = if chain == Chain::Solana {
            let rows = client.get_rows(
                "/solana/holder-token-balances",
                &[q("wallet_address", &wallet), q("limit", limit)],
            )?;
            // Balances come back as raw mint + lamports; enrich symbols/decimals
            // with one token-details call (best effort — quota or unknown mints
            // must not fail the holdings answer).
            let mints: Vec<String> = rows
                .iter()
                .filter_map(|r| row_str(r, &["tokenAddress"]))
                .take(50)
                .collect();
            let details: Map<String, Value> = if mints.is_empty() {
                Map::new()
            } else {
                client
                    .get_rows(
                        "/solana/token-details",
                        &[q("token_addresses", mints.join(","))],
                    )
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|r| row_str(&r, &["tokenAddress"]).map(|a| (a, Value::Object(r))))
                    .collect()
            };
            rows.iter()
                .map(|row| {
                    let mint = row_str(row, &["tokenAddress"]).unwrap_or_default();
                    let detail = details.get(&mint).and_then(Value::as_object);
                    let decimals = detail.and_then(|d| row_u64(d, &["decimals"]));
                    let raw = row_f64(row, &["balanceRaw"]);
                    let amount = match (raw, decimals) {
                        (Some(raw), Some(dec)) => Some(raw / 10f64.powi(dec as i32)),
                        _ => None,
                    };
                    json!({
                        "token_address": mint,
                        "symbol": detail.and_then(|d| row_str(d, &["symbol"])),
                        "name": detail.and_then(|d| row_str(d, &["name"])),
                        "amount": amount,
                        "amount_raw": row_value(row, &["balanceRaw"]),
                        "decimals": decimals,
                        "value_usd": row_f64(row, &["balanceUSD", "balanceUsd"]),
                    })
                })
                .collect()
        } else {
            let chain_id = chain.require_evm("wallet holdings")?;
            let rows = client.get_rows(
                "/evm/tvl/status",
                &[
                    q("chain_id", &chain_id),
                    q("wallet_address", &wallet),
                    q("hasprice", args.priced_only.unwrap_or(true)),
                    q("limit", limit),
                ],
            )?;
            rows.iter()
                .map(|row| {
                    json!({
                        "token_address": row_str(row, &["tokenAddress"]),
                        "symbol": row_str(row, &["tokenSymbol", "symbol"]),
                        "amount": row_f64(row, &["tokenAmount"]),
                        "value_usd": row_f64(row, &["valueUsd", "valueUSD"]),
                    })
                })
                .collect()
        };

        let total: f64 = holdings
            .iter()
            .filter_map(|h| h["value_usd"].as_f64())
            .sum();

        Ok(json!({
            "chain": chain.label(),
            "wallet_address": wallet,
            "count": holdings.len(),
            "total_value_usd": total,
            "holdings": holdings,
            "note": "Cambrian tracks DEX-priced tokens only; native ETH/SOL balances and unlisted tokens may be absent. Zero-value rows are unpriced tokens.",
        }))
    }
}

// ============================================================================
// cambrian_get_top_holders
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTopHoldersArgs {
    /// Token address (0x on EVM, base58 mint on Solana)
    pub(crate) token_address: String,
    /// `base` (default), `ethereum`, or `solana`
    pub(crate) chain: Option<String>,
    /// Number of holders, largest first (default 20, max 100)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct GetTopHolders;

impl DynAomiTool for GetTopHolders {
    type App = CambrianApp;
    type Args = GetTopHoldersArgs;
    const NAME: &'static str = "cambrian_get_top_holders";
    const DESCRIPTION: &'static str = "List the largest holders of a token with their balance and USD value, for concentration and whale analysis. Top addresses are often exchanges, bridges, pools, or vaults; say so rather than assuming they are individuals.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = Chain::parse(args.chain.as_deref())?;
        let token = require_address(chain, &args.token_address, "token_address")?;
        let limit = clamp_limit(args.limit, 20, 100);
        let client = CambrianClient::from_ctx(&ctx)?;

        let (symbol, holders): (Option<String>, Vec<Value>) = if chain == Chain::Solana {
            let rows = client.get_rows(
                "/solana/tokens/holders",
                &[q("program_id", &token), q("limit", limit)],
            )?;
            (
                None,
                rows.iter()
                    .map(|row| {
                        json!({
                            "address": row_str(row, &["account", "ownerAddress"]),
                            "amount": row_f64(row, &["balanceUi"]),
                            "amount_raw": row_value(row, &["balanceRaw"]),
                            "value_usd": row_f64(row, &["balanceUSD", "balanceUsd"]),
                        })
                    })
                    .collect(),
            )
        } else {
            let chain_id = chain.require_evm("top holders")?;
            let rows = client.get_rows(
                "/evm/tvl/top-owners",
                &[
                    q("chain_id", &chain_id),
                    q("token_address", &token),
                    q("limit", limit),
                ],
            )?;
            (
                rows.first().and_then(|r| row_str(r, &["tokenSymbol"])),
                rows.iter()
                    .map(|row| {
                        json!({
                            "address": row_str(row, &["ownerAddress"]),
                            "amount": row_f64(row, &["tokenAmount"]),
                            "value_usd": row_f64(row, &["valueUsd", "valueUSD"]),
                        })
                    })
                    .collect(),
            )
        };

        let top_value: f64 = holders.iter().filter_map(|h| h["value_usd"].as_f64()).sum();

        Ok(json!({
            "chain": chain.label(),
            "token_address": token,
            "symbol": symbol,
            "count": holders.len(),
            "listed_value_usd": top_value,
            "holders": holders,
        }))
    }
}

// ============================================================================
// cambrian_raw_get (escape hatch)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RawGetArgs {
    /// Documented Cambrian GET path starting with `/evm/`, `/solana/`, `/deep42/`, or `/risk/`, e.g. `/solana/tokens/security` or `/deep42/social-data/token-analysis`
    pub(crate) path: String,
    /// Query parameters exactly as documented (e.g. `{"token_address": "...", "limit": 10}`)
    pub(crate) query: Option<BTreeMap<String, Value>>,
}

pub(crate) struct RawGet;

impl DynAomiTool for RawGet {
    type App = CambrianApp;
    type Args = RawGetArgs;
    const NAME: &'static str = "cambrian_raw_get";
    const DESCRIPTION: &'static str = "Escape hatch: call any documented Cambrian GET endpoint (docs.cambrian.org) with raw query params when no curated tool covers the question — e.g. Solana token security, holder distribution, OHLCV, Deep42 social sentiment, or the perp risk engine. Columnar responses are flattened to rows.";

    fn run(_app: &CambrianApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let path = args.path.trim();
        const PREFIXES: [&str; 4] = ["/evm/", "/solana/", "/deep42/", "/risk/"];
        if !PREFIXES.iter().any(|p| path.starts_with(p))
            || path.contains("..")
            || path.contains('?')
        {
            return Err(format!(
                "[cambrian] path must be a documented endpoint starting with {} and carry no query string (use `query`)",
                PREFIXES.join(", ")
            ));
        }

        let owned: Vec<(String, String)> = args
            .query
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, v)| {
                let s = match v {
                    Value::String(s) => s,
                    Value::Array(items) => items
                        .iter()
                        .map(|i| match i {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(","),
                    other => other.to_string(),
                };
                (k, s)
            })
            .collect();
        let query: Vec<(&str, String)> =
            owned.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();

        let client = CambrianClient::from_ctx(&ctx)?;
        let value = client.get_raw(path, &query)?;

        match columnar_to_rows(&value) {
            Some(rows) => {
                let total = rows.len();
                let rows: Vec<Value> = rows
                    .into_iter()
                    .take(MAX_ROWS as usize)
                    .map(Value::Object)
                    .collect();
                Ok(json!({
                    "path": path,
                    "row_count": total,
                    "truncated": total > rows.len(),
                    "rows": rows,
                }))
            }
            None => Ok(json!({ "path": path, "data": value })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aomi_sdk::testing::{TestCtxBuilder, run_tool};
    use serde_json::json;

    #[test]
    fn evm_addresses_are_validated_and_lowercased() {
        let ok = require_address(
            Chain::Base,
            " 0x833589FCD6EDB6E08F4C7C32D4F71B54BDA02913 ",
            "t",
        )
        .unwrap();
        assert_eq!(ok, "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913");
        assert!(require_address(Chain::Base, "USDC", "token").is_err());
        let sol = require_address(
            Chain::Solana,
            "So11111111111111111111111111111111111111112",
            "t",
        )
        .unwrap();
        assert_eq!(sol, "So11111111111111111111111111111111111111112");
    }

    #[test]
    fn pct_change_handles_edges() {
        assert_eq!(pct_change(Some(100.0), Some(110.0)), Some(10.0));
        assert_eq!(pct_change(Some(0.0), Some(1.0)), None);
        assert_eq!(pct_change(None, Some(1.0)), None);
    }

    #[test]
    fn lending_rows_normalize_nullable_columns() {
        let row: Map<String, Value> = serde_json::from_value(json!({
            "chainId": 8453, "protocolId": "morpho-v2", "address": "0xbeef",
            "symbol": "steakUSDC", "vaultName": "Steakhouse Prime USDC",
            "underlyingAddress": "0x8335", "underlyingSymbol": "USDC",
            "supplyUsd": 587736219.97, "borrowUsd": null, "tvlUsd": 587736219.97,
            "supplyApy": 0.0504, "borrowApy": null, "netSupplyApy": 0.0504,
            "utilizationRate": null, "ltv": null, "borrowable": false, "depositEnabled": true,
            "updatedAt": "2026-08-05T12:21:19+00:00"
        }))
        .unwrap();
        let v = lending_row(&row);
        assert_eq!(v["name"], "Steakhouse Prime USDC");
        assert_eq!(v["supply_apy"], 0.0504);
        assert!(v["borrow_apy"].is_null());
        assert_eq!(v["borrowable"], false);
        assert_eq!(v["underlying"]["symbol"], "USDC");
    }

    #[test]
    fn missing_key_is_a_clear_error_not_a_request() {
        // No secret in ctx: the tool must fail before touching the network
        // with the onboarding hint (the SDK never reads process env).
        let ctx = TestCtxBuilder::new(GetTokenPrice::NAME).build();
        let err = run_tool::<GetTokenPrice>(
            &CambrianApp,
            json!({ "token_addresses": ["0x4200000000000000000000000000000000000006"] }),
            ctx,
        )
        .unwrap_err();
        assert!(err.contains("CAMBRIAN_API_KEY"), "{err}");
        assert!(err.contains("console.cambrian.org"), "{err}");
    }

    #[test]
    fn raw_get_rejects_undocumented_paths() {
        let ctx = TestCtxBuilder::new(RawGet::NAME)
            .secret(API_KEY_NAME, "x")
            .build();
        let err =
            run_tool::<RawGet>(&CambrianApp, json!({ "path": "/admin/keys" }), ctx).unwrap_err();
        assert!(err.contains("/evm/"), "{err}");
        let ctx = TestCtxBuilder::new(RawGet::NAME)
            .secret(API_KEY_NAME, "x")
            .build();
        let err = run_tool::<RawGet>(&CambrianApp, json!({ "path": "/evm/tokens?limit=1" }), ctx)
            .unwrap_err();
        assert!(err.contains("query"), "{err}");
    }

    #[test]
    fn chain_gating_errors_before_network() {
        let ctx = TestCtxBuilder::new(SearchTokens::NAME)
            .secret(API_KEY_NAME, "x")
            .build();
        let err = run_tool::<SearchTokens>(
            &CambrianApp,
            json!({ "query": "SOL", "chain": "solana" }),
            ctx,
        )
        .unwrap_err();
        assert!(err.contains("Solana"), "{err}");

        let ctx = TestCtxBuilder::new(GetPriceHistory::NAME)
            .secret(API_KEY_NAME, "x")
            .build();
        let err = run_tool::<GetPriceHistory>(
            &CambrianApp,
            json!({ "token_address": "0x4200000000000000000000000000000000000006", "interval": "1D" }),
            ctx,
        )
        .unwrap_err();
        assert!(err.contains("hourly"), "{err}");

        let ctx = TestCtxBuilder::new(GetPoolStats::NAME)
            .secret(API_KEY_NAME, "x")
            .build();
        let err = run_tool::<GetPoolStats>(
            &CambrianApp,
            json!({ "pool_addresses": ["abc"], "chain": "solana" }),
            ctx,
        )
        .unwrap_err();
        assert!(err.contains("orca"), "{err}");
    }

    /// Live ladder against api.cambrian.org: price → history → pools → pool
    /// stats → lending → holders. Network- and key-gated (`--ignored`), ~7
    /// upstream calls, so it stays well under the free plan's 2 rps when run
    /// alone.
    #[test]
    #[ignore = "network: hits api.cambrian.org and needs CAMBRIAN_API_KEY"]
    fn live_base_read_ladder() {
        const WETH: &str = "0x4200000000000000000000000000000000000006";
        const USDC: &str = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
        let key =
            std::env::var(API_KEY_NAME).expect("CAMBRIAN_API_KEY must be set for the live ladder");
        let ctx = || {
            TestCtxBuilder::new("live")
                .secret(API_KEY_NAME, key.clone())
                .build()
        };

        let price =
            run_tool::<GetTokenPrice>(&CambrianApp, json!({ "token_addresses": [WETH] }), ctx())
                .expect("price")
                .into_value();
        let p = price["prices"][0]["price_usd"]
            .as_f64()
            .expect("weth price");
        assert!(p > 10.0, "implausible WETH price {p}");

        let hist = run_tool::<GetPriceHistory>(
            &CambrianApp,
            json!({ "token_address": WETH, "limit": 3 }),
            ctx(),
        )
        .expect("history")
        .into_value();
        assert!(hist["count"].as_u64().unwrap() >= 1, "{hist}");

        let pools = run_tool::<FindPools>(
            &CambrianApp,
            json!({ "token_address": WETH, "limit": 3 }),
            ctx(),
        )
        .expect("pools")
        .into_value();
        let pool = pools["pools"][0]["pool_address"]
            .as_str()
            .expect("pool address")
            .to_string();

        let stats =
            run_tool::<GetPoolStats>(&CambrianApp, json!({ "pool_addresses": [pool] }), ctx())
                .expect("pool stats")
                .into_value();
        assert_eq!(stats["count"], 1, "{stats}");

        let lending = run_tool::<FindLendingYields>(
            &CambrianApp,
            json!({ "underlying_address": USDC, "min_tvl_usd": 1_000_000, "limit": 3 }),
            ctx(),
        )
        .expect("lending")
        .into_value();
        assert!(lending["count"].as_u64().unwrap() >= 1, "{lending}");
        assert!(lending["pools"][0]["supply_apy"].is_number(), "{lending}");

        let holders = run_tool::<GetTopHolders>(
            &CambrianApp,
            json!({ "token_address": USDC, "limit": 3 }),
            ctx(),
        )
        .expect("holders")
        .into_value();
        assert!(holders["count"].as_u64().unwrap() >= 1, "{holders}");
    }
}
