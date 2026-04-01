use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

impl DynAomiTool for GetLammaTokenPrice {
    type App = DefiLlamaApp;
    type Args = GetLammaTokenPriceArgs;
    const NAME: &'static str = "get_token_price";
    const DESCRIPTION: &'static str = "Get overall token price estimation from DefiLlama (informational, not an executable trade quote).";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        let response = client.get_token_price(&args.token)?;
        let source = response
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("defillama");
        let coin = response
            .get("coins")
            .and_then(Value::as_object)
            .and_then(|coins| coins.values().next())
            .ok_or_else(|| format!("Token not found: {}", args.token))?;
        let symbol = coin.get("symbol").and_then(Value::as_str).unwrap_or("N/A");
        let price = coin.get("price").and_then(Value::as_f64).unwrap_or(0.0);
        let confidence = coin.get("confidence").cloned().unwrap_or(Value::Null);
        Ok(json!({
            "symbol": symbol,
            "price_usd": format!("${:.2}", price),
            "confidence": confidence,
            "source": source,
        }))
    }
}

impl DynAomiTool for GetLammaYieldOpportunities {
    type App = DefiLlamaApp;
    type Args = GetLammaYieldOpportunitiesArgs;
    const NAME: &'static str = "get_yield_opportunities";
    const DESCRIPTION: &'static str = "Get overall yield estimation from DefiLlama and list pools sorted by APY (informational, not trade execution).";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        let response = client.get_yield_pools(args.chain.as_deref(), args.project.as_deref())?;
        let source = response
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("defillama");

        let mut pools = response
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        if args.stablecoin_only.unwrap_or(false) {
            pools.retain(|p| {
                p.get("stablecoin")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            });
        }

        pools.sort_by(|a, b| {
            let aa = a.get("apy").and_then(Value::as_f64).unwrap_or(0.0);
            let bb = b.get("apy").and_then(Value::as_f64).unwrap_or(0.0);
            bb.partial_cmp(&aa).unwrap_or(std::cmp::Ordering::Equal)
        });
        pools.truncate(args.limit.unwrap_or(20) as usize);

        let formatted: Vec<Value> = pools
            .into_iter()
            .map(|p| {
                let tvl_str = p
                    .get("tvlUsd")
                    .and_then(Value::as_f64)
                    .map(|t| format!("${:.0}M", t / 1_000_000.0));
                json!({
                    "pool": p.get("symbol").and_then(Value::as_str).unwrap_or("N/A"),
                    "project": p.get("project").and_then(Value::as_str).unwrap_or("N/A"),
                    "chain": p.get("chain").and_then(Value::as_str).unwrap_or("N/A"),
                    "apy": format!("{:.2}%", p.get("apy").and_then(Value::as_f64).unwrap_or(0.0)),
                    "tvl": tvl_str,
                    "stablecoin": p.get("stablecoin").cloned().unwrap_or(Value::Null),
                    "il_risk": p.get("ilRisk").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();

        Ok(json!({
            "pools_found": formatted.len(),
            "pools": formatted,
            "source": source,
        }))
    }
}

impl DynAomiTool for GetLammaProtocols {
    type App = DefiLlamaApp;
    type Args = GetLammaProtocolsArgs;
    const NAME: &'static str = "get_defi_protocols";
    const DESCRIPTION: &'static str = "Get overall protocol TVL estimation from DefiLlama (informational, not executable trading data).";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        let response = client.get_protocols(args.category.as_deref())?;
        let source = response
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("defillama");

        let mut protocols = response
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        protocols.truncate(args.limit.unwrap_or(20) as usize);

        let formatted: Vec<Value> = protocols
            .into_iter()
            .map(|p| {
                json!({
                    "name": p.get("name").and_then(Value::as_str).unwrap_or("N/A"),
                    "tvl": format!("${:.2}B", p.get("tvl").and_then(Value::as_f64).unwrap_or(0.0) / 1_000_000_000.0),
                    "category": p.get("category").cloned().unwrap_or(Value::Null),
                    "chains": p.get("chains").cloned().unwrap_or(Value::Null),
                    "change_1d": p.get("change_1d").and_then(Value::as_f64).map(|c| format!("{:+.1}%", c)),
                })
            })
            .collect();

        Ok(json!({
            "protocols_count": formatted.len(),
            "protocols": formatted,
            "source": source,
        }))
    }
}

impl DynAomiTool for GetLammaChainTvl {
    type App = DefiLlamaApp;
    type Args = GetLammaChainTvlArgs;
    const NAME: &'static str = "get_chain_tvl";
    const DESCRIPTION: &'static str = "Get overall chain TVL estimation from DefiLlama (informational, not executable trading data).";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        let response = client.get_chains_tvl()?;
        let source = response
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("defillama");
        let mut chains = response
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        chains.truncate(args.limit.unwrap_or(15) as usize);

        let formatted: Vec<Value> = chains
            .iter()
            .enumerate()
            .map(|(i, c)| {
                json!({
                    "rank": i + 1,
                    "chain": c.get("name").and_then(Value::as_str).unwrap_or("N/A"),
                    "tvl": format!("${:.2}B", c.get("tvl").and_then(Value::as_f64).unwrap_or(0.0) / 1_000_000_000.0),
                    "native_token": c.get("tokenSymbol").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();

        Ok(json!({
            "chains": formatted,
            "source": source,
        }))
    }
}

// ============================================================================
// Tier 1 tool impls
// ============================================================================

impl DynAomiTool for GetLammaProtocolDetail {
    type App = DefiLlamaApp;
    type Args = GetLammaProtocolDetailArgs;
    const NAME: &'static str = "get_protocol_detail";
    const DESCRIPTION: &'static str = "Get deep-dive data for a single protocol: historical TVL, chain breakdown, metadata.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_protocol_detail(&args.protocol)
    }
}

impl DynAomiTool for GetLammaDexVolumes {
    type App = DefiLlamaApp;
    type Args = GetLammaDexVolumesArgs;
    const NAME: &'static str = "get_dex_volumes";
    const DESCRIPTION: &'static str = "Get DEX volume rankings across all chains or for a specific chain.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_dex_volumes(
            args.chain.as_deref(),
            args.exclude_total_data_chart,
            args.exclude_total_data_chart_breakdown,
        )
    }
}

impl DynAomiTool for GetLammaFeesOverview {
    type App = DefiLlamaApp;
    type Args = GetLammaFeesOverviewArgs;
    const NAME: &'static str = "get_fees_overview";
    const DESCRIPTION: &'static str = "Get protocol fee and revenue rankings across all chains or for a specific chain.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_fees_overview(
            args.chain.as_deref(),
            args.exclude_total_data_chart,
            args.exclude_total_data_chart_breakdown,
            args.data_type.as_deref(),
        )
    }
}

impl DynAomiTool for GetLammaProtocolFees {
    type App = DefiLlamaApp;
    type Args = GetLammaProtocolFeesArgs;
    const NAME: &'static str = "get_protocol_fees";
    const DESCRIPTION: &'static str = "Get fee and revenue detail for a single protocol.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_protocol_fees(&args.protocol, args.data_type.as_deref())
    }
}

impl DynAomiTool for GetLammaStablecoins {
    type App = DefiLlamaApp;
    type Args = GetLammaStablecoinsArgs;
    const NAME: &'static str = "get_stablecoins";
    const DESCRIPTION: &'static str = "List all stablecoins with their circulating supply data.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_stablecoins(args.include_prices)
    }
}

impl DynAomiTool for GetLammaStablecoinChains {
    type App = DefiLlamaApp;
    type Args = GetLammaStablecoinChainsArgs;
    const NAME: &'static str = "get_stablecoin_chains";
    const DESCRIPTION: &'static str = "Get stablecoin market cap per chain.";

    fn run(_app: &DefiLlamaApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_stablecoin_chains()
    }
}

impl DynAomiTool for GetLammaHistoricalTokenPrice {
    type App = DefiLlamaApp;
    type Args = GetLammaHistoricalTokenPriceArgs;
    const NAME: &'static str = "get_historical_token_price";
    const DESCRIPTION: &'static str = "Get historical price chart for one or more tokens.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_historical_token_price(
            &args.coins,
            args.start,
            args.end,
            args.span,
            args.period.as_deref(),
        )
    }
}

impl DynAomiTool for GetLammaTokenPriceChange {
    type App = DefiLlamaApp;
    type Args = GetLammaTokenPriceChangeArgs;
    const NAME: &'static str = "get_token_price_change";
    const DESCRIPTION: &'static str = "Get percentage price change for one or more tokens over a given period.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_token_price_change(
            &args.coins,
            args.timestamp,
            args.look_forward,
            args.period.as_deref(),
        )
    }
}

// ============================================================================
// Tier 2 tool impls
// ============================================================================

impl DynAomiTool for GetLammaHistoricalChainTvl {
    type App = DefiLlamaApp;
    type Args = GetLammaHistoricalChainTvlArgs;
    const NAME: &'static str = "get_historical_chain_tvl";
    const DESCRIPTION: &'static str = "Get daily historical TVL for a specific chain.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_historical_chain_tvl(&args.chain)
    }
}

impl DynAomiTool for GetLammaDexProtocolVolume {
    type App = DefiLlamaApp;
    type Args = GetLammaDexProtocolVolumeArgs;
    const NAME: &'static str = "get_dex_protocol_volume";
    const DESCRIPTION: &'static str = "Get volume detail for a single DEX protocol.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_dex_protocol_volume(
            &args.protocol,
            args.exclude_total_data_chart,
            args.exclude_total_data_chart_breakdown,
        )
    }
}

impl DynAomiTool for GetLammaStablecoinHistory {
    type App = DefiLlamaApp;
    type Args = GetLammaStablecoinHistoryArgs;
    const NAME: &'static str = "get_stablecoin_history";
    const DESCRIPTION: &'static str = "Get historical stablecoin market cap data, optionally filtered by chain or stablecoin ID.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_stablecoin_history(args.chain.as_deref(), args.stablecoin)
    }
}

impl DynAomiTool for GetLammaYieldPoolHistory {
    type App = DefiLlamaApp;
    type Args = GetLammaYieldPoolHistoryArgs;
    const NAME: &'static str = "get_yield_pool_history";
    const DESCRIPTION: &'static str = "Get historical APY and TVL data for a specific yield pool.";

    fn run(_app: &DefiLlamaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        client.get_yield_pool_history(&args.pool)
    }
}

