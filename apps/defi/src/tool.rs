use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};

impl DynAomiTool for GetLammaTokenPrice {
    type App = DefiApp;
    type Args = GetLammaTokenPriceArgs;
    const NAME: &'static str = "get_token_price";
    const DESCRIPTION: &'static str = "Get overall token price estimation from DefiLama (informational, not an executable trade quote).";

    fn run(_app: &DefiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
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

// ============================================================================
// Tool 2: Get Yield Opportunities
// ============================================================================

pub(crate) struct GetLammaYieldOpportunities;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLammaYieldOpportunitiesArgs {
    /// Filter by chain (optional): ethereum, arbitrum, optimism, polygon, base, bsc, solana
    chain: Option<String>,
    /// Filter by project name (optional): aave, compound, lido, etc.
    project: Option<String>,
    /// Only show stablecoin pools
    stablecoin_only: Option<bool>,
    /// Maximum results (default: 20)
    limit: Option<u32>,
}

impl DynAomiTool for GetLammaYieldOpportunities {
    type App = DefiApp;
    type Args = GetLammaYieldOpportunitiesArgs;
    const NAME: &'static str = "get_yield_opportunities";
    const DESCRIPTION: &'static str = "Get overall yield estimation from DefiLama and list pools sorted by APY (informational, not trade execution).";

    fn run(_app: &DefiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
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

// ============================================================================
// Tool 3: Get Aggregator Swap Quote
// ============================================================================

pub(crate) struct GetAggregatorSwapQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAggregatorSwapQuoteArgs {
    /// Source chain to quote on
    chain: String,
    /// Destination chain (primarily for cross-chain aggregators like LI.FI)
    destination_chain: Option<String>,
    /// Sell token
    sell_token: String,
    /// Buy token
    buy_token: String,
    /// Amount to swap
    amount: f64,
    /// Sender/taker address
    sender_address: String,
    /// Receiver address
    receiver_address: Option<String>,
    /// Order side (sell/buy), used mainly by CoW
    order_side: Option<String>,
    /// Slippage tolerance as decimal (0.005 = 0.5%)
    slippage: Option<f64>,
    /// Optional quote validity timestamp
    valid_to: Option<u64>,
    /// Optional partial fill behavior
    partially_fillable: Option<bool>,
    /// Optional signing scheme (e.g. eip712, ethsign)
    signing_scheme: Option<String>,
    /// Preferred aggregator: 0x, lifi, cow, or all
    prefer_aggregator: Option<String>,
}

impl DynAomiTool for GetAggregatorSwapQuote {
    type App = DefiApp;
    type Args = GetAggregatorSwapQuoteArgs;
    const NAME: &'static str = "get_aggregator_swap_quote";
    const DESCRIPTION: &'static str = "Get swap quotes from one preferred aggregator or all (0x, LI.FI, CoW) using shared quote arguments.";

    fn run(_app: &DefiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Aggregator::new()?;
        let preferred = args
            .prefer_aggregator
            .as_deref()
            .unwrap_or("all")
            .to_lowercase();

        let mut quotes: Vec<Value> = Vec::new();
        let want_all = preferred == "all";
        let amount_base_units =
            client.quote_amount_base_units(&args.chain, &args.sell_token, args.amount)?;
        let destination_chain = args
            .destination_chain
            .as_deref()
            .unwrap_or(args.chain.as_str());

        if want_all || preferred == "0x" {
            match client.get_quote_0x(
                &args.chain,
                &args.sell_token,
                &args.buy_token,
                args.amount,
                Some(&args.sender_address),
                args.slippage,
            ) {
                Ok(v) => quotes.push(v),
                Err(e) => quotes.push(json!({"source":"0x","error": e})),
            }
        }

        if want_all || preferred == "lifi" {
            match client.get_quote_lifi(
                &args.chain,
                destination_chain,
                &args.sell_token,
                &args.buy_token,
                &amount_base_units,
                &args.sender_address,
                args.receiver_address.as_deref(),
            ) {
                Ok(v) => quotes.push(v),
                Err(e) => quotes.push(json!({"source":"lifi","error": e})),
            }
        }

        if want_all || preferred == "cow" {
            let receiver = args
                .receiver_address
                .clone()
                .unwrap_or_else(|| args.sender_address.clone());
            let mut payload = json!({
                "sellToken": client.resolve_token_address(&args.chain, &args.sell_token)?,
                "buyToken": client.resolve_token_address(&args.chain, &args.buy_token)?,
                "sellAmountBeforeFee": amount_base_units,
                "from": args.sender_address,
                "receiver": receiver,
                "kind": args.order_side.clone().unwrap_or_else(|| "sell".to_string()),
            });
            if let Some(valid_to) = args.valid_to {
                payload["validTo"] = json!(valid_to);
            }
            if let Some(partially_fillable) = args.partially_fillable {
                payload["partiallyFillable"] = json!(partially_fillable);
            }
            if let Some(signing_scheme) = args.signing_scheme.clone() {
                payload["signingScheme"] = Value::String(signing_scheme);
            }
            if let Some(slippage) = args.slippage {
                payload["slippageBps"] = json!((slippage * 10_000.0) as u32);
            }

            match client.get_quote_cow(&args.chain, payload) {
                Ok(v) => quotes.push(v),
                Err(e) => quotes.push(json!({"source":"cow","error": e})),
            }
        }

        if !want_all && quotes.len() == 1 {
            Ok(quotes.remove(0))
        } else {
            Ok(Value::Array(quotes))
        }
    }
}

// ============================================================================
// Tool 4: Place Aggregator EVM Order (0x + LI.FI)
// ============================================================================

pub(crate) struct PlaceAggregatorEvmOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceAggregatorEvmOrderArgs {
    /// Source chain
    chain: String,
    /// Destination chain (used by LI.FI)
    destination_chain: Option<String>,
    /// Sell token
    sell_token: String,
    /// Buy token
    buy_token: String,
    /// Sell amount (human units)
    amount: f64,
    /// Sender/taker wallet address
    sender_address: String,
    /// Receiver wallet address
    receiver_address: Option<String>,
    /// Slippage tolerance as decimal (0.005 = 0.5%)
    slippage: Option<f64>,
    /// Preferred EVM aggregator: 0x or lifi
    prefer_aggregator: String,
}

impl DynAomiTool for PlaceAggregatorEvmOrder {
    type App = DefiApp;
    type Args = PlaceAggregatorEvmOrderArgs;
    const NAME: &'static str = "place_aggregator_evm_order";
    const DESCRIPTION: &'static str = "Get executable order tx data via 0x or LI.FI. Returns transaction data (to, data, value) that the host should verify with `encode_and_simulate` and send with `send_transaction_to_wallet`. LI.FI may return an approval_tx that must be executed first.";

    fn run(_app: &DefiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Aggregator::new()?;
        let preferred = args.prefer_aggregator.to_lowercase();
        if preferred == "cow" {
            return Err(
                "Cow is not supported by place_aggregator_evm_order. Use place_cow_order."
                    .to_string(),
            );
        }
        if preferred == "all" {
            return Err(
                "place_aggregator_evm_order requires a single aggregator: 0x or lifi".to_string(),
            );
        }

        if preferred == "0x" {
            let quote = client.place_order_0x(
                &args.chain,
                &args.sell_token,
                &args.buy_token,
                args.amount,
                &args.sender_address,
                args.slippage,
            )?;
            let tx = quote
                .get("transaction")
                .cloned()
                .ok_or_else(|| "0x response missing transaction payload".to_string())?;

            return Ok(json!({
                "source": "0x",
                "quote": quote,
                "transaction": tx,
                "note": "Use the host's encode_and_simulate tool to verify this transaction, then use send_transaction_to_wallet to execute it.",
            }));
        }

        if preferred == "lifi" {
            let amount_base_units =
                client.quote_amount_base_units(&args.chain, &args.sell_token, args.amount)?;
            let to_chain = args
                .destination_chain
                .clone()
                .unwrap_or_else(|| args.chain.clone());
            let payload = client.place_order_lifi(
                &args.chain,
                &to_chain,
                &args.sell_token,
                &args.buy_token,
                &amount_base_units,
                &args.sender_address,
                args.receiver_address.as_deref(),
                args.slippage,
            )?;

            return Ok(json!({
                "source": "lifi",
                "payload": payload,
                "approval_tx": payload.get("approval_tx").cloned().unwrap_or(Value::Null),
                "main_tx": payload.get("main_tx").cloned().unwrap_or(Value::Null),
                "note": "If approval_tx is present, use the host's encode_and_simulate and send_transaction_to_wallet tools for the approval first, then do the same for main_tx.",
            }));
        }

        Err(format!(
            "Unsupported aggregator '{preferred}'. Use '0x' or 'lifi'."
        ))
    }
}

// ============================================================================
// Tool 5: Place CoW Order
// ============================================================================

pub(crate) struct PlaceCowOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceCowOrderArgs {
    /// CoW chain (mainnet, xdai, arbitrum_one, base, polygon, etc.)
    chain: String,
    /// Signed order payload to submit to CoW /orders endpoint
    signed_order: Value,
}

impl DynAomiTool for PlaceCowOrder {
    type App = DefiApp;
    type Args = PlaceCowOrderArgs;
    const NAME: &'static str = "place_cow_order";
    const DESCRIPTION: &'static str = "Submit a signed CoW Protocol orderbook payload to CoW /orders API on the requested chain. This posts signed order data to CoW. Use the host's wallet/signing tools for any required user approval.";

    fn run(_app: &DefiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Aggregator::new()?;
        let response = client.place_order_cow(&args.chain, args.signed_order)?;
        Ok(response)
    }
}

// ============================================================================
// Tool 6: Get Protocols
// ============================================================================

pub(crate) struct GetLammaProtocols;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLammaProtocolsArgs {
    /// Filter by category: dexes, lending, yield, liquid-staking, bridge, derivatives
    category: Option<String>,
    /// Maximum results (default: 20)
    limit: Option<u32>,
}

impl DynAomiTool for GetLammaProtocols {
    type App = DefiApp;
    type Args = GetLammaProtocolsArgs;
    const NAME: &'static str = "get_defi_protocols";
    const DESCRIPTION: &'static str = "Get overall protocol TVL estimation from DefiLama (informational, not executable trading data).";

    fn run(_app: &DefiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
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
                    "change_1d": p.get("change_1d").and_then(Value::as_f64).map(|c| format!("{c:+.1}%")),
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

// ============================================================================
// Tool 7: Get Chain TVL
// ============================================================================

pub(crate) struct GetLammaChainTvl;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLammaChainTvlArgs {
    /// Maximum results (default: 15)
    limit: Option<u32>,
}

impl DynAomiTool for GetLammaChainTvl {
    type App = DefiApp;
    type Args = GetLammaChainTvlArgs;
    const NAME: &'static str = "get_chain_tvl";
    const DESCRIPTION: &'static str = "Get overall chain TVL estimation from DefiLama (informational, not executable trading data).";

    fn run(_app: &DefiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
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
// Tool 8: Get Bridges
// ============================================================================

pub(crate) struct GetLammaBridges;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLammaBridgesArgs {
    /// Maximum results (default: 10)
    limit: Option<u32>,
}

impl DynAomiTool for GetLammaBridges {
    type App = DefiApp;
    type Args = GetLammaBridgesArgs;
    const NAME: &'static str = "get_bridges";
    const DESCRIPTION: &'static str = "Get overall bridge volume estimation from DefiLama (informational, not executable transfer advice).";

    fn run(_app: &DefiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DefiLamaClient::new()?;
        let response = client.get_bridges()?;
        let source = response
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("defillama");
        let mut bridges = response
            .get("bridges")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        bridges.truncate(args.limit.unwrap_or(10) as usize);

        let formatted: Vec<Value> = bridges
            .into_iter()
            .map(|b| {
                json!({
                    "name": b.get("displayName")
                        .or_else(|| b.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("N/A"),
                    "volume_24h": b.get("lastDailyVolume").and_then(Value::as_f64).map(|v| format!("${:.1}M", v / 1_000_000.0)),
                    "volume_7d": b.get("weeklyVolume").and_then(Value::as_f64).map(|v| format!("${:.1}M", v / 1_000_000.0)),
                    "chains": b.get("chains").cloned().unwrap_or(Value::Null),
                })
            })
            .collect();

        Ok(json!({
            "bridges": formatted,
            "source": source,
            "warning": "Always verify bridge security before transferring large amounts"
        }))
    }
}

pub(crate) struct GetBridgeQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetBridgeQuoteArgs {
    /// Source chain
    from_chain: String,
    /// Destination chain
    to_chain: String,
    /// Source token symbol/address
    from_token: String,
    /// Destination token symbol/address
    to_token: String,
    /// Amount to bridge
    amount: f64,
    /// Sender wallet address; needed for executable quote
    from_address: Option<String>,
    /// Receiver wallet address; needed for executable quote
    to_address: Option<String>,
    /// Slippage tolerance in basis points (default 50)
    slippage_bps: Option<u32>,
}

impl DynAomiTool for GetBridgeQuote {
    type App = DefiApp;
    type Args = GetBridgeQuoteArgs;
    const NAME: &'static str = "get_bridge_quote";
    const DESCRIPTION: &'static str = "Get bridge routing quote. Returns executable bridge payload when available; otherwise planning-only estimate.";

    fn run(_app: &DefiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let aggregator = Aggregator::new()?;
        aggregator.get_bridge_quote(
            &args.from_chain,
            &args.to_chain,
            &args.from_token,
            &args.to_token,
            args.amount,
            args.from_address.as_deref(),
            args.to_address.as_deref(),
            args.slippage_bps,
        )
    }
}
