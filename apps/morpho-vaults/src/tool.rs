//! Curated tool layer for the Morpho Vault Monitor app.
//!
//! Read tools normalize the Morpho REST + GraphQL surfaces into stable JSON.
//! `morpho_deposit` / `morpho_withdraw` emit routed plans that stage plain
//! ERC-4626 calls (`deposit` / `withdraw` / `redeem`) plus the ERC-20
//! approval through the host wallet (`stage_tx` with `data.encode`), with
//! host-enforced `simulate_batch` → `commit_txs`.

use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Clone, Default)]
pub(crate) struct MorphoVaultsApp;

const ERC20_APPROVE: &str = "approve(address,uint256)";
const ERC4626_DEPOSIT: &str = "deposit(uint256,address)";
const ERC4626_WITHDRAW: &str = "withdraw(uint256,address,address)";
const ERC4626_REDEEM: &str = "redeem(uint256,address,address)";

/// Timelocks shorter than this on capital-affecting functions get flagged.
const SHORT_TIMELOCK_SECONDS: u64 = 3 * 24 * 3600;

/// V2 functions whose timelock matters most for depositors.
const V2_CRITICAL_TIMELOCKS: &[&str] = &[
    "addAdapter",
    "increaseAbsoluteCap",
    "increaseRelativeCap",
    "setPerformanceFee",
    "setManagementFee",
    "setReceiveAssetsGate",
    "setSendSharesGate",
    "setForceDeallocatePenalty",
    "setAdapterRegistry",
];

// ============================================================================
// Shared helpers
// ============================================================================

fn default_chain_id() -> u64 {
    1
}

fn ok(mut map: Map<String, Value>) -> Result<Value, String> {
    map.insert("source".to_string(), Value::String("morpho".to_string()));
    Ok(Value::Object(map))
}

fn obj(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(m) => m,
        other => {
            let mut m = Map::new();
            m.insert("data".to_string(), other);
            m
        }
    }
}

fn check_chain(chain_id: u64) -> Result<(), String> {
    if is_supported_chain(chain_id) {
        Ok(())
    } else {
        Err(format!(
            "[morpho] chain {chain_id} is not indexed by the Morpho API. Supported: {}",
            SUPPORTED_CHAINS
                .iter()
                .map(|(id, name)| format!("{id} ({name})"))
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }
}

/// Explicit arg wins, then the session's connected EVM wallet.
fn resolve_wallet(arg: Option<&str>, ctx: &DynToolCallCtx) -> Result<String, String> {
    let candidate = arg
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .or_else(|| ctx.attribute_string(&["domain", "evm", "address"]))
        .ok_or_else(|| {
            "[morpho] no wallet address: pass `address` or connect an EVM wallet".to_string()
        })?;
    require_address(&candidate, "wallet address")
}

fn identity(version: VaultVersion, chain_id: u64, config: &Value) -> Value {
    json!({
        "version": version.label(),
        "chain_id": chain_id,
        "chain": chain_name(chain_id),
        "address": config.get("address"),
        "name": config.get("name"),
        "symbol": config.get("symbol"),
        "asset": config.get("asset"),
    })
}

fn f64_field(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(Value::as_f64)
}

fn timelock_human(seconds: Option<u64>) -> Option<String> {
    let s = seconds?;
    Some(if s == 0 {
        "none".to_string()
    } else if s % 86_400 == 0 {
        format!("{} days", s / 86_400)
    } else if s % 3_600 == 0 {
        format!("{} hours", s / 3_600)
    } else {
        format!("{s} seconds")
    })
}

// ============================================================================
// morpho_find_vaults
// ============================================================================

pub(crate) struct FindVaults;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FindVaultsArgs {
    /// Chain ID (1 Ethereum, 8453 Base, 42161 Arbitrum, 10 OP, 137 Polygon,
    /// 130 Unichain, 480 World Chain, 999 HyperEVM, 747474 Katana, 143 Monad).
    /// Default: 1.
    #[serde(default = "default_chain_id")]
    pub chain_id: u64,
    /// Underlying asset filter: a symbol (`USDC`, `WETH`, `cbBTC`) or a
    /// 0x token address. Omit for all assets.
    #[serde(default)]
    pub asset: Option<String>,
    /// `v1` (MetaMorpho), `v2`, or omit / `all` for both.
    #[serde(default)]
    pub version: Option<String>,
    /// Minimum TVL in USD. Default 100000.
    #[serde(default)]
    pub min_tvl_usd: Option<f64>,
    /// Ranking key: `net_apy` (default) or `tvl`.
    #[serde(default)]
    pub sort_by: Option<String>,
    /// Include vaults Morpho does not list in its own UI. Default false.
    #[serde(default)]
    pub include_unlisted: Option<bool>,
    /// Max vaults returned. Default 10, max 50.
    #[serde(default)]
    pub limit: Option<usize>,
}

fn normalize_v1_row(v: &Value) -> Value {
    let state = v.get("state").cloned().unwrap_or(Value::Null);
    json!({
        "version": "v1",
        "chain_id": v.pointer("/chain/id"),
        "address": v.get("address"),
        "name": v.get("name"),
        "symbol": v.get("symbol"),
        "listed": v.get("listed"),
        "asset": v.get("asset"),
        "apy": state.get("apy"),
        "net_apy": state.get("netApy"),
        "avg_net_apy": state.get("avgNetApy"),
        "tvl_usd": state.get("totalAssetsUsd"),
        "fee": state.get("fee"),
        "timelock_seconds": state.get("timelock"),
        "curator": state.get("curator"),
        "rewards": rewards_list(state.get("allRewards")),
        "warnings": warnings_list(v.get("warnings")),
    })
}

fn normalize_v2_row(v: &Value) -> Value {
    json!({
        "version": "v2",
        "chain_id": v.pointer("/chain/id"),
        "address": v.get("address"),
        "name": v.get("name"),
        "symbol": v.get("symbol"),
        "listed": v.get("listed"),
        "type": v.get("type"),
        "asset": v.get("asset"),
        "apy": v.get("apy"),
        "net_apy": v.get("netApy"),
        "net_apy_excluding_rewards": v.get("netApyExcludingRewards"),
        "avg_net_apy": v.get("avgNetApy"),
        "tvl_usd": v.get("totalAssetsUsd"),
        "liquidity_usd": v.get("liquidityUsd"),
        "idle_usd": v.get("idleAssetsUsd"),
        "performance_fee": v.get("performanceFee"),
        "management_fee": v.get("managementFee"),
        "curator": v.pointer("/curator/address"),
        "rewards": rewards_list(v.get("rewards")),
        "warnings": warnings_list(v.get("warnings")),
    })
}

impl DynAomiTool for FindVaults {
    type App = MorphoVaultsApp;
    type Args = FindVaultsArgs;
    const NAME: &'static str = "morpho_find_vaults";
    const DESCRIPTION: &'static str = "Discover and rank Morpho vaults (V1 MetaMorpho and V2) on a chain by net APY or TVL, optionally filtered by underlying asset and version. Use for 'best USDC vault on Base', 'which Morpho vaults hold WETH', or to resolve a vault address before calling the per-vault tools. Returns a compact row per vault (address, curator, APY, TVL, fees, rewards, warnings).";

    fn run(
        _app: &MorphoVaultsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        check_chain(args.chain_id)?;
        let version = VaultVersion::parse(args.version.as_deref())?;
        let asset = args.asset.as_deref().and_then(AssetFilter::parse);
        let min_tvl = args.min_tvl_usd.unwrap_or(100_000.0).max(0.0);
        let listed_only = !args.include_unlisted.unwrap_or(false);
        let limit = args.limit.unwrap_or(10).clamp(1, 50);
        let sort_key = match args
            .sort_by
            .as_deref()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref()
        {
            None | Some("net_apy") | Some("apy") => "net_apy",
            Some("tvl") | Some("tvl_usd") | Some("total_assets") => "tvl_usd",
            Some(other) => {
                return Err(format!(
                    "[morpho] unknown sort_by `{other}`; use `net_apy` or `tvl`"
                ));
            }
        };
        let order_by = if sort_key == "net_apy" {
            "NetApy"
        } else {
            "TotalAssetsUsd"
        };

        let client = MorphoClient::new()?;
        let mut rows: Vec<Value> = Vec::new();
        let mut notes: Vec<String> = Vec::new();

        if version != Some(VaultVersion::V2) {
            match client.graphql_vaults_v1(
                args.chain_id,
                listed_only,
                asset.as_ref(),
                min_tvl,
                order_by,
                100,
            ) {
                Ok(items) => rows.extend(items.iter().map(normalize_v1_row)),
                Err(e) => notes.push(format!("v1 discovery failed: {e}")),
            }
        }
        if version != Some(VaultVersion::V1) {
            let address_filter = match &asset {
                Some(AssetFilter::Address(a)) => Some(a.as_str()),
                _ => None,
            };
            match client.graphql_vaults_v2(
                args.chain_id,
                listed_only,
                address_filter,
                min_tvl,
                order_by,
                100,
            ) {
                Ok(items) => {
                    let symbol_filter = match &asset {
                        Some(AssetFilter::Symbol(s)) => Some(s.as_str()),
                        _ => None,
                    };
                    rows.extend(
                        items
                            .iter()
                            .filter(|v| {
                                symbol_filter.is_none_or(|sym| {
                                    v.pointer("/asset/symbol")
                                        .and_then(Value::as_str)
                                        .map(|s| s.eq_ignore_ascii_case(sym))
                                        .unwrap_or(false)
                                })
                            })
                            .map(normalize_v2_row),
                    );
                }
                Err(e) => notes.push(format!("v2 discovery failed: {e}")),
            }
        }
        if rows.is_empty() && !notes.is_empty() {
            return Err(notes.join(" | "));
        }

        rows.sort_by(|a, b| {
            let av = f64_field(a, sort_key).unwrap_or(f64::NEG_INFINITY);
            let bv = f64_field(b, sort_key).unwrap_or(f64::NEG_INFINITY);
            bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
        });
        let total = rows.len();
        rows.truncate(limit);

        ok(obj(json!({
            "chain_id": args.chain_id,
            "chain": chain_name(args.chain_id),
            "asset_filter": args.asset,
            "version_filter": version.map(VaultVersion::label).unwrap_or("all"),
            "listed_only": listed_only,
            "min_tvl_usd": min_tvl,
            "sort_by": sort_key,
            "matched": total,
            "returned": rows.len(),
            "vaults": rows,
            "notes": notes,
        })))
    }
}

// ============================================================================
// morpho_vault_overview
// ============================================================================

pub(crate) struct VaultOverview;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct VaultArgs {
    /// Chain ID the vault lives on. Default: 1 (Ethereum).
    #[serde(default = "default_chain_id")]
    pub chain_id: u64,
    /// Vault contract address (0x-prefixed).
    pub address: String,
    /// `v1` or `v2` if known; omit to auto-detect.
    #[serde(default)]
    pub version: Option<String>,
}

fn v2_withdrawal_liquidity(opts: &Value, decimals: u32) -> Value {
    let penalties: Vec<Value> = opts
        .get("adapter_penalties")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|p| {
                    json!({
                        "adapter": p.get("adapter_address"),
                        "kind": p.get("adapter_kind"),
                        "force_deallocatable_assets": amount_pair(p.get("force_deallocatable_assets"), decimals),
                        "penalty_rate": wad_to_fraction(p.get("penalty_rate_wad")),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let liquid = parse_u128(opts.get("liquidity_adapter_available_assets")).unwrap_or(0)
        + parse_u128(opts.get("idle_assets")).unwrap_or(0);
    json!({
        "instant_exit_capacity": { "raw": liquid.to_string(), "human": from_base_units(&liquid.to_string(), decimals) },
        "liquidity_adapter_available": amount_pair(opts.get("liquidity_adapter_available_assets"), decimals),
        "idle_assets": amount_pair(opts.get("idle_assets"), decimals),
        "force_deallocate": penalties,
    })
}

impl DynAomiTool for VaultOverview {
    type App = MorphoVaultsApp;
    type Args = VaultArgs;
    const NAME: &'static str = "morpho_vault_overview";
    const DESCRIPTION: &'static str = "Full snapshot of one Morpho vault (V1 or V2, auto-detected): asset, roles (owner/curator/guardian/allocators/sentinels), fees, timelock, live accounting state (total assets, share price, withdrawable/idle/allocated), current and trailing APY (7d/30d), reward APRs, USD analytics, exit liquidity incl. V2 force-deallocate penalties, gates, and Morpho risk warnings. Call before recommending or depositing into a vault.";

    fn run(
        _app: &MorphoVaultsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        check_chain(args.chain_id)?;
        let address = require_address(&args.address, "vault address")?;
        let pinned = VaultVersion::parse(args.version.as_deref())?;
        let client = MorphoClient::new()?;
        let (version, config) = client.resolve_vault(pinned, args.chain_id, &address)?;
        let decimals = asset_decimals(&config);
        let mut notes: Vec<String> = Vec::new();

        let state = match client.vault_state(version, args.chain_id, &address) {
            Ok(s) => s,
            Err(e) => {
                notes.push(format!("live state unavailable: {e}"));
                Value::Null
            }
        };
        let apy_7d = client
            .vault_apy_average(version, args.chain_id, &address, "seven_days")
            .unwrap_or_else(|e| {
                notes.push(format!("7d APY average unavailable: {e}"));
                None
            });
        let apy_30d = client
            .vault_apy_average(version, args.chain_id, &address, "thirty_days")
            .unwrap_or_else(|e| {
                notes.push(format!("30d APY average unavailable: {e}"));
                None
            });
        let detail = match client.graphql_vault_detail(version, args.chain_id, &address) {
            Ok(d) => d,
            Err(e) => {
                notes.push(format!("indexed analytics unavailable: {e}"));
                Value::Null
            }
        };

        let (roles, fees, apy, analytics_usd, liquidity_extra, gates) = match version {
            VaultVersion::V1 => {
                let st = detail.get("state").cloned().unwrap_or(Value::Null);
                (
                    json!({
                        "owner": config.get("owner"),
                        "curator": config.get("curator"),
                        "guardian": config.get("guardian"),
                        "fee_recipient": config.get("fee_recipient"),
                        "skim_recipient": config.get("skim_recipient"),
                        "curator_names": detail.pointer("/state/curators"),
                    }),
                    json!({ "fee": wad_to_fraction(config.get("fee_wad")) }),
                    json!({
                        "current": st.get("apy"),
                        "net": st.get("netApy"),
                        "net_excluding_rewards": st.get("netApyExcludingRewards"),
                        "avg_net_indexed": st.get("avgNetApy"),
                        "avg_7d": apy_7d,
                        "avg_30d": apy_30d,
                        "rewards": rewards_list(st.get("allRewards")),
                    }),
                    json!({
                        "tvl_usd": st.get("totalAssetsUsd"),
                        "liquidity_usd": detail.pointer("/liquidity/usd"),
                        "share_price_usd": st.get("sharePriceUsd"),
                    }),
                    Value::Null,
                    Value::Null,
                )
            }
            VaultVersion::V2 => {
                let withdrawal = client
                    .vault_withdrawal_options(args.chain_id, &address)
                    .map(|o| v2_withdrawal_liquidity(&o, decimals))
                    .unwrap_or_else(|e| {
                        notes.push(format!("withdrawal options unavailable: {e}"));
                        Value::Null
                    });
                let gates = config.get("gates").cloned().unwrap_or(Value::Null);
                (
                    json!({
                        "owner": config.get("owner"),
                        "curator": config.get("curator"),
                        "curator_names": detail.pointer("/curators/items"),
                        "allocators": detail.get("allocators").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.pointer("/allocator/address").cloned()).collect::<Vec<_>>()),
                        "sentinels": detail.get("sentinels").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.pointer("/sentinel/address").cloned()).collect::<Vec<_>>()),
                        "performance_fee_recipient": config.get("performance_fee_recipient"),
                        "management_fee_recipient": config.get("management_fee_recipient"),
                        "adapter_registry": config.get("adapter_registry"),
                        "liquidity_adapter": config.get("liquidity_adapter"),
                    }),
                    json!({
                        "performance_fee": wad_to_fraction(config.get("performance_fee_wad")),
                        "management_fee": wad_to_fraction(config.get("management_fee_wad")),
                        "max_rate_per_second_wad": config.get("max_rate_per_second_wad"),
                        "max_apy": detail.get("maxApy"),
                    }),
                    json!({
                        "current": detail.get("apy"),
                        "net": detail.get("netApy"),
                        "net_excluding_rewards": detail.get("netApyExcludingRewards"),
                        "avg_net_indexed": detail.get("avgNetApy"),
                        "avg_7d": apy_7d,
                        "avg_30d": apy_30d,
                        "rewards": rewards_list(detail.get("rewards")),
                    }),
                    json!({
                        "tvl_usd": detail.get("totalAssetsUsd"),
                        "liquidity_usd": detail.get("liquidityUsd"),
                        "idle_usd": detail.get("idleAssetsUsd"),
                        "force_deallocatable_usd": detail.get("forceDeallocatableLiquidityUsd"),
                        "share_price": detail.get("sharePrice"),
                    }),
                    withdrawal,
                    gates,
                )
            }
        };

        let mut liquidity = obj(json!({
            "withdrawable_assets": amount_pair(state.get("withdrawable_assets"), decimals),
            "idle_assets": amount_pair(state.get("idle_assets"), decimals),
            "allocated_assets": amount_pair(state.get("allocated_assets"), decimals),
        }));
        merge_into(&mut liquidity, liquidity_extra);

        let timelock = config.get("timelock_seconds").and_then(Value::as_u64);
        ok(obj(json!({
            "vault": identity(version, args.chain_id, &config),
            "listed": detail.get("listed"),
            "type": detail.get("type"),
            "description": detail.pointer("/metadata/description"),
            "factory": config.get("factory_address"),
            "roles": roles,
            "fees": fees,
            "timelock_seconds": timelock,
            "timelock": timelock_human(timelock),
            "state": {
                "total_assets": amount_pair(state.get("total_assets"), decimals),
                "total_supply_shares": state.get("total_supply"),
                "share_price": ray_to_f64(state.get("share_price_ray")),
                "last_indexed_block": state.get("last_indexed_block"),
                "last_accrual_timestamp": state.get("last_accrual_timestamp"),
            },
            "apy": apy,
            "analytics_usd": analytics_usd,
            "liquidity": Value::Object(liquidity),
            "gates": gates,
            "warnings": warnings_list(detail.get("warnings")),
            "notes": notes,
        })))
    }
}

// ============================================================================
// morpho_vault_allocations
// ============================================================================

pub(crate) struct VaultAllocations;

/// Total assets an adapter currently holds. V2 caps nest: an `adapter` cap
/// aggregates its market / collateral caps, so prefer it and otherwise take
/// the largest cap allocation rather than summing overlapping caps.
fn adapter_allocated_assets(caps: &[Value]) -> Option<u128> {
    let adapter_cap = caps
        .iter()
        .find(|c| c.get("cap_type").and_then(Value::as_str) == Some("adapter"))
        .and_then(|c| parse_u128(c.get("allocated_assets")));
    adapter_cap.or_else(|| {
        caps.iter()
            .filter_map(|c| parse_u128(c.get("allocated_assets")))
            .max()
    })
}

fn pct(part: Option<f64>, total: Option<f64>) -> Option<f64> {
    match (part, total) {
        (Some(p), Some(t)) if t > 0.0 => Some(p / t * 100.0),
        _ => None,
    }
}

impl DynAomiTool for VaultAllocations {
    type App = MorphoVaultsApp;
    type Args = VaultArgs;
    const NAME: &'static str = "morpho_vault_allocations";
    const DESCRIPTION: &'static str = "Where a Morpho vault's assets are deployed. V1: per-market allocation with supply, cap, queue order, market APY/utilization/liquidity and pending cap changes. V2: per-adapter allocation with type, USD, force-deallocate penalty and caps. Includes each line's share of the vault and the top concentration. Use for exposure and concentration questions.";

    fn run(
        _app: &MorphoVaultsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        check_chain(args.chain_id)?;
        let address = require_address(&args.address, "vault address")?;
        let pinned = VaultVersion::parse(args.version.as_deref())?;
        let client = MorphoClient::new()?;
        let (version, config) = client.resolve_vault(pinned, args.chain_id, &address)?;
        let decimals = asset_decimals(&config);
        let mut notes: Vec<String> = Vec::new();

        let state = client
            .vault_state(version, args.chain_id, &address)
            .unwrap_or_else(|e| {
                notes.push(format!("live state unavailable: {e}"));
                Value::Null
            });
        let total_raw = parse_u128(state.get("total_assets"));
        let total_f = total_raw.map(|t| t as f64);
        let detail = client
            .graphql_vault_detail(version, args.chain_id, &address)
            .unwrap_or_else(|e| {
                notes.push(format!("indexed analytics unavailable: {e}"));
                Value::Null
            });
        let tvl_usd = match version {
            VaultVersion::V1 => detail.pointer("/state/totalAssetsUsd"),
            VaultVersion::V2 => detail.get("totalAssetsUsd"),
        }
        .and_then(Value::as_f64);

        let mut rows: Vec<Value> = Vec::new();
        match version {
            VaultVersion::V1 => {
                let indexed = detail
                    .pointer("/state/allocation")
                    .and_then(Value::as_array)
                    .cloned();
                if let Some(items) = indexed {
                    for a in items {
                        let supply = a.get("supplyAssets").and_then(|v| {
                            v.as_str()
                                .and_then(|s| s.parse::<f64>().ok())
                                .or(v.as_f64())
                        });
                        let market = a.get("market").cloned().unwrap_or(Value::Null);
                        rows.push(json!({
                            "market_id": market.get("marketId"),
                            "loan_asset": market.pointer("/loanAsset/symbol"),
                            "collateral_asset": market.pointer("/collateralAsset/symbol"),
                            "lltv": wad_to_fraction(market.get("lltv")),
                            "supply_assets": amount_pair(a.get("supplyAssets"), decimals),
                            "supply_usd": a.get("supplyAssetsUsd"),
                            "pct_of_vault": pct(supply, total_f),
                            "supply_cap": amount_pair(a.get("supplyCap"), decimals),
                            "supply_cap_usd": a.get("supplyCapUsd"),
                            "pending_supply_cap_usd": a.get("pendingSupplyCapUsd"),
                            "removable_at": a.get("removableAt"),
                            "supply_queue_index": a.get("supplyQueueIndex"),
                            "withdraw_queue_index": a.get("withdrawQueueIndex"),
                            "market_supply_apy": market.pointer("/state/supplyApy"),
                            "market_utilization": market.pointer("/state/utilization"),
                            "market_liquidity_usd": market.pointer("/state/liquidityAssetsUsd"),
                        }));
                    }
                } else {
                    let rest = client.vault_allocations(version, args.chain_id, &address)?;
                    for a in rest
                        .get("allocations")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default()
                    {
                        let supply = parse_u128(a.get("supply_assets")).map(|x| x as f64);
                        rows.push(json!({
                            "market_id": a.get("market_id"),
                            "supply_assets": amount_pair(a.get("supply_assets"), decimals),
                            "pct_of_vault": pct(supply, total_f),
                            "supply_cap": amount_pair(a.get("supply_cap"), decimals),
                            "supply_queue_index": a.get("supply_queue_index"),
                            "withdraw_queue_index": a.get("withdraw_queue_index"),
                            "pending": a.get("pending"),
                        }));
                    }
                }
            }
            VaultVersion::V2 => {
                let rest = client.vault_allocations(version, args.chain_id, &address)?;
                let indexed_adapters: Vec<Value> = detail
                    .pointer("/adapters/items")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for a in rest
                    .get("allocations")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default()
                {
                    let adapter = str_field(&a, "adapter_address").unwrap_or_default();
                    let indexed = indexed_adapters.iter().find(|x| {
                        x.get("address")
                            .and_then(Value::as_str)
                            .map(|s| s.eq_ignore_ascii_case(&adapter))
                            .unwrap_or(false)
                    });
                    let assets_usd = indexed.and_then(|x| f64_field(x, "assetsUsd"));
                    let raw_caps: Vec<Value> = a
                        .get("caps")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default();
                    let allocated = adapter_allocated_assets(&raw_caps);
                    let caps: Vec<Value> = raw_caps
                        .iter()
                        .map(|c| {
                            json!({
                                "cap_id": c.get("cap_id"),
                                "cap_type": c.get("cap_type"),
                                "market_id": c.get("market_id"),
                                "allocated_assets": amount_pair(c.get("allocated_assets"), decimals),
                                "absolute_cap": amount_pair(c.get("absolute_cap"), decimals),
                                "relative_cap": wad_to_fraction(c.get("relative_cap_wad")),
                                "pending": c.get("pending"),
                            })
                        })
                        .collect();
                    let pct_of_vault = pct(allocated.map(|x| x as f64), total_f)
                        .or_else(|| pct(assets_usd, tvl_usd));
                    rows.push(json!({
                        "adapter": adapter,
                        "adapter_kind": a.get("adapter_kind"),
                        "adapter_type": indexed.and_then(|x| x.get("type").cloned()),
                        "target": a.get("target"),
                        "allocated_assets": allocated.map(|x| json!({ "raw": x.to_string(), "human": from_base_units(&x.to_string(), decimals) })),
                        "assets_usd": assets_usd,
                        "pct_of_vault": pct_of_vault,
                        "force_deallocate_penalty": indexed.and_then(|x| wad_to_fraction(x.get("forceDeallocatePenalty"))),
                        "pending": a.get("pending"),
                        "caps": caps,
                    }));
                }
            }
        }
        rows.sort_by(|a, b| {
            let av = f64_field(a, "pct_of_vault").unwrap_or(-1.0);
            let bv = f64_field(b, "pct_of_vault").unwrap_or(-1.0);
            bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
        });
        let top = rows.first().and_then(|r| f64_field(r, "pct_of_vault"));
        let allocated_pct: f64 = rows
            .iter()
            .filter_map(|r| f64_field(r, "pct_of_vault"))
            .sum();

        ok(obj(json!({
            "vault": identity(version, args.chain_id, &config),
            "total_assets": amount_pair(state.get("total_assets"), decimals),
            "tvl_usd": tvl_usd,
            "idle_assets": amount_pair(state.get("idle_assets"), decimals),
            "allocation_count": rows.len(),
            "top_allocation_pct": top,
            "allocated_pct": allocated_pct,
            "allocations": rows,
            "last_indexed_block": state.get("last_indexed_block"),
            "notes": notes,
        })))
    }
}

// ============================================================================
// morpho_vault_history
// ============================================================================

pub(crate) struct VaultHistory;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct VaultHistoryArgs {
    /// Chain ID the vault lives on. Default: 1 (Ethereum).
    #[serde(default = "default_chain_id")]
    pub chain_id: u64,
    /// Vault contract address (0x-prefixed).
    pub address: String,
    /// Trailing window: `one_day`, `seven_days` (default), `thirty_days`,
    /// `ninety_days`, `one_year`, `inception` (also accepts `7d`, `30d`, `1y`).
    #[serde(default)]
    pub lookback: Option<String>,
    /// `v1` or `v2` if known; omit to auto-detect.
    #[serde(default)]
    pub version: Option<String>,
    /// Max points per series after downsampling. Default 48, max 500.
    #[serde(default)]
    pub max_points: Option<usize>,
}

impl DynAomiTool for VaultHistory {
    type App = MorphoVaultsApp;
    type Args = VaultHistoryArgs;
    const NAME: &'static str = "morpho_vault_history";
    const DESCRIPTION: &'static str = "APY, total assets, withdrawable liquidity and share-price history for one Morpho vault over a lookback window, downsampled, with min/max/avg/change summaries and the realized average APY for the window. Use for 'how has this vault performed', trend, or drawdown questions.";

    fn run(
        _app: &MorphoVaultsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        check_chain(args.chain_id)?;
        let address = require_address(&args.address, "vault address")?;
        let pinned = VaultVersion::parse(args.version.as_deref())?;
        let lookback = normalize_lookback(args.lookback.as_deref())?;
        let max_points = args.max_points.unwrap_or(48).clamp(2, 500);
        let client = MorphoClient::new()?;
        let (version, config) = client.resolve_vault(pinned, args.chain_id, &address)?;
        let decimals = asset_decimals(&config);
        let mut notes: Vec<String> = Vec::new();

        let apy_points =
            client.vault_history(version, args.chain_id, &address, "apy", &lookback)?;
        let state_points = client
            .vault_history(version, args.chain_id, &address, "state", &lookback)
            .unwrap_or_else(|e| {
                notes.push(format!("state history unavailable: {e}"));
                Vec::new()
            });
        let avg_apy = client
            .vault_apy_average(version, args.chain_id, &address, &lookback)
            .unwrap_or_else(|e| {
                notes.push(format!("apy average unavailable: {e}"));
                None
            });

        let to_human_f64 = |v: Option<&Value>| -> Option<f64> {
            parse_u128(v).map(|raw| {
                from_base_units(&raw.to_string(), decimals)
                    .parse::<f64>()
                    .unwrap_or(0.0)
            })
        };
        let state_human: Vec<Value> = state_points
            .iter()
            .map(|p| {
                json!({
                    "timestamp": p.get("timestamp"),
                    "total_assets": to_human_f64(p.get("total_assets")),
                    "withdrawable_assets": to_human_f64(p.get("withdrawable_assets")),
                    "idle_assets": to_human_f64(p.get("idle_assets")),
                    "share_price": ray_to_f64(p.get("share_price_ray")),
                })
            })
            .collect();

        ok(obj(json!({
            "vault": identity(version, args.chain_id, &config),
            "lookback": lookback,
            "realized_avg_apy": avg_apy,
            "summary": {
                "apy": series_summary(&apy_points, "apy"),
                "total_assets": series_summary(&state_human, "total_assets"),
                "withdrawable_assets": series_summary(&state_human, "withdrawable_assets"),
                "share_price": series_summary(&state_human, "share_price"),
            },
            "apy_series": downsample(&apy_points, max_points),
            "state_series": downsample(&state_human, max_points),
            "notes": notes,
        })))
    }
}

// ============================================================================
// morpho_vault_governance
// ============================================================================

pub(crate) struct VaultGovernance;

/// Pure risk-flag derivation so it can be unit tested without the network.
fn governance_flags(
    version: VaultVersion,
    timelock_seconds: Option<u64>,
    timelocks: &[Value],
    pending_count: usize,
    warnings: &[Value],
    gates: &Value,
) -> Vec<String> {
    let mut flags = Vec::new();
    for w in warnings {
        let level = w.get("level").and_then(Value::as_str).unwrap_or("");
        let kind = w.get("type").and_then(Value::as_str).unwrap_or("unknown");
        if level == "RED" || level == "YELLOW" {
            flags.push(format!("warning:{level}:{kind}"));
        }
    }
    if pending_count > 0 {
        flags.push(format!("pending_governance_actions:{pending_count}"));
    }
    match version {
        VaultVersion::V1 => {
            if let Some(t) = timelock_seconds
                && t < SHORT_TIMELOCK_SECONDS
            {
                flags.push(format!("short_timelock:{t}s"));
            }
        }
        VaultVersion::V2 => {
            for tl in timelocks {
                let name = tl.get("functionName").and_then(Value::as_str).unwrap_or("");
                let duration = tl.get("duration").and_then(Value::as_u64).unwrap_or(0);
                let abdicated = tl.get("abdicatedAt").map(|v| !v.is_null()).unwrap_or(false);
                if V2_CRITICAL_TIMELOCKS.contains(&name)
                    && !abdicated
                    && duration < SHORT_TIMELOCK_SECONDS
                {
                    flags.push(format!("short_timelock:{name}:{duration}s"));
                }
            }
            if let Some(g) = gates.as_object() {
                for (name, v) in g {
                    if !v.is_null() {
                        flags.push(format!("gated:{name}"));
                    }
                }
            }
        }
    }
    flags
}

impl DynAomiTool for VaultGovernance {
    type App = MorphoVaultsApp;
    type Args = VaultArgs;
    const NAME: &'static str = "morpho_vault_governance";
    const DESCRIPTION: &'static str = "Monitor a Morpho vault's governance and risk posture: pending timelocked actions (cap changes, adapter/market changes, fee changes, role changes), timelock durations (per function for V2), roles, sentinels, gates, Morpho warnings, and derived risk flags (RED/YELLOW warnings, short timelocks on capital-affecting functions, pending changes, active gates). Use for 'is anything changing in this vault' or periodic health checks.";

    fn run(
        _app: &MorphoVaultsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        check_chain(args.chain_id)?;
        let address = require_address(&args.address, "vault address")?;
        let pinned = VaultVersion::parse(args.version.as_deref())?;
        let client = MorphoClient::new()?;
        let (version, config) = client.resolve_vault(pinned, args.chain_id, &address)?;
        let mut notes: Vec<String> = Vec::new();

        let pending = client
            .vault_pending_governance(version, args.chain_id, &address)
            .unwrap_or_else(|e| {
                notes.push(format!("pending governance actions unavailable: {e}"));
                Vec::new()
            });
        let detail = client
            .graphql_vault_detail(version, args.chain_id, &address)
            .unwrap_or_else(|e| {
                notes.push(format!("indexed analytics unavailable: {e}"));
                Value::Null
            });
        let warnings = warnings_list(detail.get("warnings"));
        let timelock_seconds = config.get("timelock_seconds").and_then(Value::as_u64);

        let (roles, timelocks, pending_configs, gates) = match version {
            VaultVersion::V1 => (
                json!({
                    "owner": config.get("owner"),
                    "curator": config.get("curator"),
                    "guardian": config.get("guardian"),
                }),
                Vec::new(),
                detail
                    .pointer("/state/pendingConfigs/items")
                    .cloned()
                    .unwrap_or(json!([])),
                Value::Null,
            ),
            VaultVersion::V2 => (
                json!({
                    "owner": config.get("owner"),
                    "curator": config.get("curator"),
                    "allocators": detail.get("allocators").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.pointer("/allocator/address").cloned()).collect::<Vec<_>>()),
                    "sentinels": detail.get("sentinels").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.pointer("/sentinel/address").cloned()).collect::<Vec<_>>()),
                    "adapter_registry": config.get("adapter_registry"),
                }),
                detail
                    .get("timelocks")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default(),
                detail
                    .pointer("/pendingConfigs/items")
                    .cloned()
                    .unwrap_or(json!([])),
                config.get("gates").cloned().unwrap_or(Value::Null),
            ),
        };
        let flags = governance_flags(
            version,
            timelock_seconds,
            &timelocks,
            pending.len(),
            &warnings,
            &gates,
        );
        let timelocks_out: Vec<Value> = timelocks
            .iter()
            .map(|t| {
                let d = t.get("duration").and_then(Value::as_u64);
                json!({
                    "function": t.get("functionName"),
                    "duration_seconds": d,
                    "duration": timelock_human(d),
                    "abdicated_at": t.get("abdicatedAt"),
                })
            })
            .collect();

        ok(obj(json!({
            "vault": identity(version, args.chain_id, &config),
            "listed": detail.get("listed"),
            "roles": roles,
            "timelock_seconds": timelock_seconds,
            "timelock": timelock_human(timelock_seconds),
            "timelocks": timelocks_out,
            "pending_actions_count": pending.len(),
            "pending_actions": pending,
            "pending_configs_indexed": pending_configs,
            "gates": gates,
            "warnings": warnings,
            "risk_flags": flags,
            "healthy": flags.is_empty(),
            "notes": notes,
        })))
    }
}

// ============================================================================
// morpho_user_vault_positions
// ============================================================================

/// Position row built from the REST list plus the vault config (no USD).
fn rest_position_row(client: &MorphoClient, version: VaultVersion, p: &Value) -> Value {
    let chain = p.get("chain_id").and_then(Value::as_u64).unwrap_or(1);
    let vault_addr = str_field(p, "vault_address").unwrap_or_default();
    let cfg = client
        .vault_config(version, chain, &vault_addr)
        .unwrap_or(Value::Null);
    let decimals = asset_decimals(&cfg);
    json!({
        "version": version.label(),
        "chain_id": chain,
        "vault_address": vault_addr,
        "name": cfg.get("name"),
        "symbol": cfg.get("symbol"),
        "asset": cfg.pointer("/asset/symbol"),
        "asset_decimals": decimals,
        "shares": p.get("shares"),
        "assets": amount_pair(p.get("assets"), decimals),
    })
}

pub(crate) struct UserVaultPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct UserVaultPositionsArgs {
    /// Wallet address (0x-prefixed). Defaults to the connected wallet.
    #[serde(default)]
    pub address: Option<String>,
    /// Restrict to one chain ID. Omit for every chain Morpho indexes.
    #[serde(default)]
    pub chain_id: Option<u64>,
    /// Also fetch realized earnings per position (extra calls, capped at 10
    /// positions). Default true.
    #[serde(default)]
    pub include_earnings: Option<bool>,
}

impl DynAomiTool for UserVaultPositions {
    type App = MorphoVaultsApp;
    type Args = UserVaultPositionsArgs;
    const NAME: &'static str = "morpho_user_vault_positions";
    const DESCRIPTION: &'static str = "A wallet's Morpho vault positions (V1 and V2) across chains: shares, asset value, USD value, indexed P&L / return, the vault's current net APY, and realized net earnings. Defaults to the connected wallet. Use for 'what do I have on Morpho', 'how much have I earned', or before a withdrawal.";

    fn run(_app: &MorphoVaultsApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let user = resolve_wallet(args.address.as_deref(), &ctx)?;
        if let Some(c) = args.chain_id {
            check_chain(c)?;
        }
        let include_earnings = args.include_earnings.unwrap_or(true);
        let client = MorphoClient::new()?;
        let mut notes: Vec<String> = Vec::new();
        let mut rows: Vec<Value> = Vec::new();

        // V1: one indexed query covers every chain.
        match client.graphql_v1_positions(&user, args.chain_id) {
            Ok(items) => {
                for p in items {
                    let vault = p.get("vault").cloned().unwrap_or(Value::Null);
                    let st = p.get("state").cloned().unwrap_or(Value::Null);
                    let decimals = vault
                        .pointer("/asset/decimals")
                        .and_then(Value::as_u64)
                        .unwrap_or(18) as u32;
                    if parse_u128(st.get("shares")).unwrap_or(0) == 0 {
                        continue;
                    }
                    rows.push(json!({
                        "version": "v1",
                        "chain_id": vault.pointer("/chain/id"),
                        "vault_address": vault.get("address"),
                        "name": vault.get("name"),
                        "symbol": vault.get("symbol"),
                        "asset": vault.pointer("/asset/symbol"),
                        "asset_decimals": decimals,
                        "shares": st.get("shares"),
                        "assets": amount_pair(st.get("assets"), decimals),
                        "assets_usd": st.get("assetsUsd"),
                        "pnl_usd": st.get("pnlUsd"),
                        "roe": st.get("roe"),
                        "vault_net_apy": vault.pointer("/state/netApy"),
                    }));
                }
            }
            Err(e) => {
                notes.push(format!("indexed V1 positions unavailable, using REST: {e}"));
                for p in client
                    .user_positions(VaultVersion::V1, &user, args.chain_id)?
                    .into_iter()
                    .take(25)
                {
                    rows.push(rest_position_row(&client, VaultVersion::V1, &p));
                }
            }
        }

        // V2: REST list (all chains), enriched per position with indexed USD /
        // P&L because the API exposes no cross-vault V2 position list.
        for p in client
            .user_positions(VaultVersion::V2, &user, args.chain_id)?
            .into_iter()
            .take(25)
        {
            let chain = p.get("chain_id").and_then(Value::as_u64).unwrap_or(1);
            let vault_addr = str_field(&p, "vault_address").unwrap_or_default();
            match client.graphql_v2_position(&user, chain, &vault_addr) {
                Ok(pos) if !pos.is_null() => {
                    let vault = pos.get("vault").cloned().unwrap_or(Value::Null);
                    let decimals = vault
                        .pointer("/asset/decimals")
                        .and_then(Value::as_u64)
                        .unwrap_or(18) as u32;
                    rows.push(json!({
                        "version": "v2",
                        "chain_id": chain,
                        "vault_address": vault_addr,
                        "name": vault.get("name"),
                        "symbol": vault.get("symbol"),
                        "asset": vault.pointer("/asset/symbol"),
                        "asset_decimals": decimals,
                        "shares": p.get("shares"),
                        "assets": amount_pair(p.get("assets"), decimals),
                        "assets_usd": pos.get("assetsUsd"),
                        "pnl_usd": pos.get("pnlUsd"),
                        "roe": pos.get("roe"),
                        "vault_net_apy": vault.get("netApy"),
                    }));
                }
                Ok(_) => rows.push(rest_position_row(&client, VaultVersion::V2, &p)),
                Err(e) => {
                    notes.push(format!(
                        "indexed V2 position unavailable for {vault_addr}: {e}"
                    ));
                    rows.push(rest_position_row(&client, VaultVersion::V2, &p));
                }
            }
        }
        if let Some(c) = args.chain_id {
            rows.retain(|r| r.get("chain_id").and_then(Value::as_u64) == Some(c));
        }
        rows.sort_by(|a, b| {
            let av = f64_field(a, "assets_usd").unwrap_or(0.0);
            let bv = f64_field(b, "assets_usd").unwrap_or(0.0);
            bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
        });

        if include_earnings {
            for row in rows.iter_mut().take(10) {
                let version = if row.get("version") == Some(&json!("v2")) {
                    VaultVersion::V2
                } else {
                    VaultVersion::V1
                };
                let chain = row.get("chain_id").and_then(Value::as_u64).unwrap_or(1);
                let vault_addr = str_field(row, "vault_address").unwrap_or_default();
                match client.user_position_performance(version, chain, &vault_addr, &user) {
                    Ok(perf) => {
                        let decimals = row
                            .get("asset_decimals")
                            .and_then(Value::as_u64)
                            .unwrap_or(18) as u32;
                        row["net_earnings"] =
                            amount_pair(perf.get("net_earnings_assets"), decimals);
                        row["return_on_capital"] = perf
                            .get("return_on_capital")
                            .cloned()
                            .unwrap_or(Value::Null);
                        row["accounting_method"] = perf
                            .get("accounting_method")
                            .cloned()
                            .unwrap_or(Value::Null);
                    }
                    Err(e) => notes.push(format!("earnings unavailable for {vault_addr}: {e}")),
                }
            }
        }

        let total_usd: f64 = rows.iter().filter_map(|r| f64_field(r, "assets_usd")).sum();
        ok(obj(json!({
            "address": user,
            "chain_filter": args.chain_id,
            "position_count": rows.len(),
            "total_usd": total_usd,
            "positions": rows,
            "notes": notes,
        })))
    }
}

// ============================================================================
// morpho_deposit
// ============================================================================

pub(crate) struct Deposit;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct DepositArgs {
    /// Chain ID the vault lives on. The connected wallet must be on this chain.
    #[serde(default = "default_chain_id")]
    pub chain_id: u64,
    /// Vault contract address (0x-prefixed).
    pub vault_address: String,
    /// Amount of the vault's underlying asset in human units (e.g. "250" or
    /// "0.5"). Pass as a string to preserve precision.
    pub amount: String,
    /// Address that receives the vault shares. Defaults to the wallet.
    #[serde(default)]
    pub receiver: Option<String>,
    /// Wallet that pays. Defaults to the connected wallet.
    #[serde(default)]
    pub wallet: Option<String>,
    /// `v1` or `v2` if known; omit to auto-detect.
    #[serde(default)]
    pub version: Option<String>,
}

struct DepositPlanInput<'a> {
    version: VaultVersion,
    chain_id: u64,
    config: &'a Value,
    state: &'a Value,
    detail: &'a Value,
    wallet: &'a str,
    receiver: &'a str,
    amount_raw: &'a str,
    amount_human: &'a str,
}

/// Build the deposit preview + routed plan from already-fetched data.
fn build_deposit_plan(input: DepositPlanInput<'_>) -> Result<ToolReturn, String> {
    let DepositPlanInput {
        version,
        chain_id,
        config,
        state,
        detail,
        wallet,
        receiver,
        amount_raw,
        amount_human,
    } = input;
    let vault_address = str_field(config, "address")
        .ok_or_else(|| "[morpho] vault config missing address".to_string())?;
    let asset_address = config
        .pointer("/asset/address")
        .and_then(Value::as_str)
        .ok_or_else(|| "[morpho] vault config missing asset address".to_string())?
        .to_string();
    let asset_symbol = config
        .pointer("/asset/symbol")
        .and_then(Value::as_str)
        .unwrap_or("asset")
        .to_string();
    let vault_name = str_field(config, "name").unwrap_or_else(|| vault_address.clone());
    let decimals = asset_decimals(config);

    let share_price = ray_to_f64(state.get("share_price_ray"));
    let expected_shares = share_price.and_then(|sp| {
        if sp > 0.0 {
            amount_human.parse::<f64>().ok().map(|a| a / sp)
        } else {
            None
        }
    });

    let mut warnings = warnings_list(detail.get("warnings"));
    if version == VaultVersion::V2 {
        if let Some(gate) = config.pointer("/gates/receive_shares")
            && !gate.is_null()
        {
            warnings.push(json!({
                "type": "receive_shares_gate",
                "level": "YELLOW",
                "detail": format!("Vault V2 has a receive-shares gate at {gate}; the deposit may revert if the wallet is not allowed. Simulation will confirm."),
            }));
        }
        if let Some(gate) = config.pointer("/gates/receive_assets")
            && !gate.is_null()
        {
            warnings.push(json!({
                "type": "receive_assets_gate",
                "level": "YELLOW",
                "detail": format!("Vault V2 has a receive-assets gate at {gate}."),
            }));
        }
    }
    let net_apy = match version {
        VaultVersion::V1 => detail.pointer("/state/netApy"),
        VaultVersion::V2 => detail.get("netApy"),
    }
    .cloned()
    .unwrap_or(Value::Null);

    let approve_args = json!({
        "to": asset_address,
        "description": format!(
            "Approve {amount_human} {asset_symbol} for Morpho vault {vault_name} ({vault_address}) on {} (chain {chain_id})",
            chain_name(chain_id)
        ),
        "data": {
            "encode": {
                "signature": ERC20_APPROVE,
                "args": [vault_address.clone(), amount_raw],
            }
        },
        "value": "0",
        "kind": "erc20_approve",
    });
    let deposit_args = json!({
        "to": vault_address,
        "description": format!(
            "Deposit {amount_human} {asset_symbol} into Morpho vault {vault_name} ({vault_address}) for {receiver} on {} (chain {chain_id})",
            chain_name(chain_id)
        ),
        "data": {
            "encode": {
                "signature": ERC4626_DEPOSIT,
                "args": [amount_raw, receiver],
            }
        },
        "value": "0",
        "kind": "vault_deposit",
    });

    let preview = obj(json!({
        "status": "awaiting_wallet",
        "action": "deposit",
        "vault": {
            "version": version.label(),
            "chain_id": chain_id,
            "chain": chain_name(chain_id),
            "address": vault_address,
            "name": vault_name,
            "symbol": config.get("symbol"),
        },
        "asset": { "address": asset_address, "symbol": asset_symbol, "decimals": decimals },
        "amount": { "raw": amount_raw, "human": amount_human },
        "wallet": wallet,
        "receiver": receiver,
        "share_price": share_price,
        "expected_shares_approx": expected_shares,
        "vault_net_apy": net_apy,
        "warnings": warnings,
        "tx_count": 2,
        "requires_chain_id": chain_id,
        "note": "Plain ERC-4626 deposit staged through the host wallet: approve(vault, amount) on the asset, then deposit(amount, receiver) on the vault. The host simulates before asking for signatures.",
    }));
    let mut preview = preview;
    preview.insert("source".to_string(), json!("morpho"));

    ToolReturn::route(Value::Object(preview))
        .next(|next| {
            next.add::<host::StageTx>(approve_args).note(
                "Stage the ERC-20 approval for the Morpho vault. CRITICAL: copy `to` and \
                 `data.encode.args` byte-for-byte; the spender is the vault address and the \
                 amount is exact base units.",
            );
            next.add::<host::StageTx>(deposit_args)
                .note(
                    "Stage the ERC-4626 `deposit(assets, receiver)` call. CRITICAL: copy `to` \
                     and `data.encode.args` byte-for-byte. After this step the host simulates \
                     and commits both staged txs and waits for the wallet.",
                )
                .enforce(EnforcementPolicy::Continue, |enforce| {
                    enforce.add::<host::SimulateBatch>(json!({}));
                    enforce
                        .add::<host::CommitTxs>(json!({ "aa_preference": "auto" }))
                        .bind_as("transaction_hash");
                });
        })
        .try_build()
        .map_err(|e| format!("[morpho] deposit route build: {e}"))
}

impl DynAomiTool for Deposit {
    type App = MorphoVaultsApp;
    type Args = DepositArgs;
    const NAME: &'static str = "morpho_deposit";
    const DESCRIPTION: &'static str = "USE THIS to deposit into a Morpho vault after the user has confirmed vault, chain, asset and amount. Composite: resolves the vault (V1/V2) and its underlying asset, converts the human amount to base units, previews expected shares and warnings (incl. V2 gates), then routes through the host wallet an ERC-20 approval followed by the ERC-4626 `deposit(assets, receiver)` call (host ABI-encodes via `data.encode`). DO NOT call `stage_tx`, `simulate_batch` or `commit_txs` yourself; the route enforces simulate + commit and binds the transaction hash. The wallet must be connected to the vault's chain.";

    fn run_with_routes(
        _app: &MorphoVaultsApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        check_chain(args.chain_id)?;
        let vault_address = require_address(&args.vault_address, "vault address")?;
        let wallet = resolve_wallet(args.wallet.as_deref(), &ctx)?;
        let receiver = match args.receiver.as_deref() {
            Some(r) if !r.trim().is_empty() => require_address(r, "receiver")?,
            _ => wallet.clone(),
        };
        let pinned = VaultVersion::parse(args.version.as_deref())?;
        let client = MorphoClient::new()?;
        let (version, config) = client.resolve_vault(pinned, args.chain_id, &vault_address)?;
        let decimals = asset_decimals(&config);
        let amount_raw = to_base_units(&args.amount, decimals)?;
        let amount_human = from_base_units(&amount_raw, decimals);
        let state = client
            .vault_state(version, args.chain_id, &vault_address)
            .unwrap_or(Value::Null);
        let detail = client
            .graphql_vault_detail(version, args.chain_id, &vault_address)
            .unwrap_or(Value::Null);
        build_deposit_plan(DepositPlanInput {
            version,
            chain_id: args.chain_id,
            config: &config,
            state: &state,
            detail: &detail,
            wallet: &wallet,
            receiver: &receiver,
            amount_raw: &amount_raw,
            amount_human: &amount_human,
        })
    }
}

// ============================================================================
// morpho_withdraw
// ============================================================================

pub(crate) struct Withdraw;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct WithdrawArgs {
    /// Chain ID the vault lives on. The connected wallet must be on this chain.
    #[serde(default = "default_chain_id")]
    pub chain_id: u64,
    /// Vault contract address (0x-prefixed).
    pub vault_address: String,
    /// Amount of underlying asset to withdraw in human units (e.g. "100").
    /// Ignored when `all` is true.
    #[serde(default)]
    pub amount: Option<String>,
    /// Withdraw the entire position by redeeming all shares. Default false.
    #[serde(default)]
    pub all: Option<bool>,
    /// Address that receives the assets. Defaults to the wallet.
    #[serde(default)]
    pub receiver: Option<String>,
    /// Wallet that owns the shares. Defaults to the connected wallet.
    #[serde(default)]
    pub wallet: Option<String>,
    /// `v1` or `v2` if known; omit to auto-detect.
    #[serde(default)]
    pub version: Option<String>,
}

struct WithdrawPlanInput<'a> {
    version: VaultVersion,
    chain_id: u64,
    config: &'a Value,
    position: &'a Value,
    /// Instant exit capacity in base units (vault-wide).
    available_assets: Option<u128>,
    /// V2 force-deallocate options (informational only).
    force_options: Value,
    wallet: &'a str,
    receiver: &'a str,
    /// `None` means redeem everything.
    amount_raw: Option<&'a str>,
}

/// Build the withdraw preview + routed plan (or a no-route
/// `insufficient_liquidity` / `insufficient_balance` result).
fn build_withdraw_plan(input: WithdrawPlanInput<'_>) -> Result<ToolReturn, String> {
    let WithdrawPlanInput {
        version,
        chain_id,
        config,
        position,
        available_assets,
        force_options,
        wallet,
        receiver,
        amount_raw,
    } = input;
    let vault_address = str_field(config, "address")
        .ok_or_else(|| "[morpho] vault config missing address".to_string())?;
    let vault_name = str_field(config, "name").unwrap_or_else(|| vault_address.clone());
    let asset_symbol = config
        .pointer("/asset/symbol")
        .and_then(Value::as_str)
        .unwrap_or("asset")
        .to_string();
    let decimals = asset_decimals(config);

    let held_shares = parse_u128(position.get("shares")).unwrap_or(0);
    let held_assets = parse_u128(position.get("assets")).unwrap_or(0);
    if held_shares == 0 {
        return Err(format!(
            "[morpho] {wallet} holds no shares in {vault_name} ({vault_address}) on chain {chain_id}"
        ));
    }

    let (mode, signature, call_args, requested_assets) = match amount_raw {
        None => (
            "redeem_all",
            ERC4626_REDEEM,
            json!([held_shares.to_string(), receiver, wallet]),
            held_assets,
        ),
        Some(raw) => {
            let requested = raw
                .parse::<u128>()
                .map_err(|_| format!("[morpho] invalid base-unit amount `{raw}`"))?;
            if requested > held_assets {
                return Ok(ToolReturn::value(json!({
                    "source": "morpho",
                    "status": "insufficient_balance",
                    "action": "withdraw",
                    "vault": { "version": version.label(), "chain_id": chain_id, "address": vault_address, "name": vault_name },
                    "requested": { "raw": raw, "human": from_base_units(raw, decimals) },
                    "position": {
                        "shares": held_shares.to_string(),
                        "assets": { "raw": held_assets.to_string(), "human": from_base_units(&held_assets.to_string(), decimals) },
                    },
                    "hint": "Requested amount exceeds the position. Use `all: true` to redeem everything.",
                })));
            }
            (
                "withdraw_exact",
                ERC4626_WITHDRAW,
                json!([raw, receiver, wallet]),
                requested,
            )
        }
    };

    let position_json = json!({
        "shares": held_shares.to_string(),
        "assets": { "raw": held_assets.to_string(), "human": from_base_units(&held_assets.to_string(), decimals) },
    });
    let requested_json = json!({
        "raw": requested_assets.to_string(),
        "human": from_base_units(&requested_assets.to_string(), decimals),
    });
    let available_json = available_assets.map(
        |a| json!({ "raw": a.to_string(), "human": from_base_units(&a.to_string(), decimals) }),
    );

    if let Some(avail) = available_assets
        && requested_assets > avail
    {
        return Ok(ToolReturn::value(json!({
            "source": "morpho",
            "status": "insufficient_liquidity",
            "action": "withdraw",
            "mode": mode,
            "vault": { "version": version.label(), "chain_id": chain_id, "address": vault_address, "name": vault_name },
            "asset": asset_symbol,
            "requested": requested_json,
            "instant_exit_capacity": available_json,
            "position": position_json,
            "force_deallocate_options": force_options,
            "hint": "The vault cannot release this much instantly. Options: withdraw up to the instant capacity now, wait for liquidity to return, or (V2 only) a force-deallocate exit that pays the listed penalty. Ask the user before choosing a forced exit; this tool never stages one silently.",
        })));
    }

    let tx_args = json!({
        "to": vault_address,
        "description": format!(
            "{} {} {asset_symbol} from Morpho vault {vault_name} ({vault_address}) to {receiver} on {} (chain {chain_id})",
            if mode == "redeem_all" { "Redeem all shares:" } else { "Withdraw" },
            requested_json["human"].as_str().unwrap_or(""),
            chain_name(chain_id)
        ),
        "data": {
            "encode": {
                "signature": signature,
                "args": call_args,
            }
        },
        "value": "0",
        "kind": if mode == "redeem_all" { "vault_redeem" } else { "vault_withdraw" },
    });

    let mut preview = obj(json!({
        "status": "awaiting_wallet",
        "action": "withdraw",
        "mode": mode,
        "vault": {
            "version": version.label(),
            "chain_id": chain_id,
            "chain": chain_name(chain_id),
            "address": vault_address,
            "name": vault_name,
            "symbol": config.get("symbol"),
        },
        "asset": { "address": config.pointer("/asset/address"), "symbol": asset_symbol, "decimals": decimals },
        "requested": requested_json,
        "instant_exit_capacity": available_json,
        "position": position_json,
        "wallet": wallet,
        "receiver": receiver,
        "tx_count": 1,
        "requires_chain_id": chain_id,
        "note": "Plain ERC-4626 call staged through the host wallet. The host simulates before asking for a signature; a revert usually means liquidity or a gate changed since this preview.",
    }));
    preview.insert("source".to_string(), json!("morpho"));

    ToolReturn::route(Value::Object(preview))
        .next(|next| {
            next.add::<host::StageTx>(tx_args)
                .note(
                    "Stage the ERC-4626 withdraw/redeem call. CRITICAL: copy `to` and \
                     `data.encode.args` byte-for-byte (amount or shares, receiver, owner). \
                     After this step the host simulates and commits the staged tx and waits \
                     for the wallet.",
                )
                .enforce(EnforcementPolicy::Continue, |enforce| {
                    enforce.add::<host::SimulateBatch>(json!({}));
                    enforce
                        .add::<host::CommitTxs>(json!({ "aa_preference": "auto" }))
                        .bind_as("transaction_hash");
                });
        })
        .try_build()
        .map_err(|e| format!("[morpho] withdraw route build: {e}"))
}

impl DynAomiTool for Withdraw {
    type App = MorphoVaultsApp;
    type Args = WithdrawArgs;
    const NAME: &'static str = "morpho_withdraw";
    const DESCRIPTION: &'static str = "USE THIS to withdraw from a Morpho vault after the user has confirmed vault, chain and amount (or `all`). Composite: resolves the vault (V1/V2), reads the wallet's position and the vault's instant exit capacity (V1 withdrawable assets; V2 liquidity adapter + idle), then routes through the host wallet a single ERC-4626 `withdraw(assets, receiver, owner)` or, with `all: true`, `redeem(shares, receiver, owner)`. If the request exceeds the position or the instant liquidity it returns `insufficient_balance` / `insufficient_liquidity` with the V2 force-deallocate options instead of staging anything. DO NOT call `stage_tx`, `simulate_batch` or `commit_txs` yourself.";

    fn run_with_routes(
        _app: &MorphoVaultsApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        check_chain(args.chain_id)?;
        let vault_address = require_address(&args.vault_address, "vault address")?;
        let wallet = resolve_wallet(args.wallet.as_deref(), &ctx)?;
        let receiver = match args.receiver.as_deref() {
            Some(r) if !r.trim().is_empty() => require_address(r, "receiver")?,
            _ => wallet.clone(),
        };
        let all = args.all.unwrap_or(false);
        if !all
            && args
                .amount
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        {
            return Err("[morpho] pass `amount` or set `all: true`".to_string());
        }
        let pinned = VaultVersion::parse(args.version.as_deref())?;
        let client = MorphoClient::new()?;
        let (version, config) = client.resolve_vault(pinned, args.chain_id, &vault_address)?;
        let decimals = asset_decimals(&config);
        let amount_raw = if all {
            None
        } else {
            Some(to_base_units(
                args.amount.as_deref().unwrap_or(""),
                decimals,
            )?)
        };
        let position = client.user_position(version, args.chain_id, &vault_address, &wallet)?;

        let (available, force_options) = match version {
            VaultVersion::V1 => {
                let state = client
                    .vault_state(version, args.chain_id, &vault_address)
                    .unwrap_or(Value::Null);
                (parse_u128(state.get("withdrawable_assets")), Value::Null)
            }
            VaultVersion::V2 => {
                match client.vault_withdrawal_options(args.chain_id, &vault_address) {
                    Ok(opts) => {
                        let liquid = parse_u128(opts.get("liquidity_adapter_available_assets"))
                            .unwrap_or(0)
                            + parse_u128(opts.get("idle_assets")).unwrap_or(0);
                        let force = v2_withdrawal_liquidity(&opts, decimals);
                        (
                            Some(liquid),
                            force
                                .get("force_deallocate")
                                .cloned()
                                .unwrap_or(Value::Null),
                        )
                    }
                    Err(_) => {
                        let state = client
                            .vault_state(version, args.chain_id, &vault_address)
                            .unwrap_or(Value::Null);
                        (parse_u128(state.get("withdrawable_assets")), Value::Null)
                    }
                }
            }
        };

        build_withdraw_plan(WithdrawPlanInput {
            version,
            chain_id: args.chain_id,
            config: &config,
            position: &position,
            available_assets: available,
            force_options,
            wallet: &wallet,
            receiver: &receiver,
            amount_raw: amount_raw.as_deref(),
        })
    }
}

// ============================================================================
// Tests (no network)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use aomi_sdk::testing::TestCtxBuilder;

    const WALLET: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
    const VAULT: &str = "0xBEEF01735c132Ada46AA9aA4c54623cAA92A64CB";
    const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

    fn v1_config() -> Value {
        json!({
            "chain_id": 1, "address": VAULT, "version": "1.0",
            "name": "Steakhouse USDC", "symbol": "steakUSDC",
            "asset": { "address": USDC, "decimals": 6, "symbol": "USDC" },
            "fee_wad": "50000000000000000", "owner": "0x0A0e", "curator": "0x827e",
            "guardian": "0xaa05", "timelock_seconds": 604800
        })
    }

    fn v2_config() -> Value {
        json!({
            "chain_id": 1, "address": "0x04422053aDDbc9bB2759b248B574e3FCA76Bc145", "version": "2.0",
            "name": "Keyrock USDC", "symbol": "kUSDC",
            "asset": { "address": USDC, "decimals": 6, "symbol": "USDC" },
            "performance_fee_wad": "50000000000000000", "timelock_seconds": 604800,
            "gates": { "send_shares": null, "receive_shares": "0xGATE", "send_assets": null, "receive_assets": null }
        })
    }

    fn stage_steps(ret: &ToolReturn) -> Vec<&RouteStep> {
        ret.routes.iter().filter(|r| r.tool == "stage_tx").collect()
    }

    #[test]
    fn resolve_wallet_prefers_arg_then_ctx() {
        let ctx = TestCtxBuilder::new("morpho_deposit")
            .attribute("domain", json!({ "evm": { "address": WALLET } }))
            .build();
        assert_eq!(resolve_wallet(Some(USDC), &ctx).unwrap(), USDC);
        assert_eq!(resolve_wallet(None, &ctx).unwrap(), WALLET);
        let empty = TestCtxBuilder::new("t").build();
        assert!(resolve_wallet(None, &empty).is_err());
        assert!(resolve_wallet(Some("vitalik.eth"), &ctx).is_err());
    }

    #[test]
    fn chain_and_args_validation() {
        assert!(check_chain(8453).is_ok());
        assert!(check_chain(56).is_err());
        assert_eq!(timelock_human(Some(604800)).as_deref(), Some("7 days"));
        assert_eq!(timelock_human(Some(3600)).as_deref(), Some("1 hours"));
        assert_eq!(timelock_human(Some(0)).as_deref(), Some("none"));
    }

    #[test]
    fn deposit_plan_stages_approve_then_deposit() {
        let state = json!({ "share_price_ray": "1138622606219080802861231595" });
        let detail = json!({ "state": { "netApy": 0.039 }, "warnings": [] });
        let ret = build_deposit_plan(DepositPlanInput {
            version: VaultVersion::V1,
            chain_id: 1,
            config: &v1_config(),
            state: &state,
            detail: &detail,
            wallet: WALLET,
            receiver: WALLET,
            amount_raw: "25000000",
            amount_human: "25",
        })
        .unwrap();

        assert_eq!(ret.value["status"], "awaiting_wallet");
        assert_eq!(ret.value["amount"]["human"], "25");
        assert_eq!(ret.value["vault_net_apy"], 0.039);
        let shares = ret.value["expected_shares_approx"].as_f64().unwrap();
        assert!((shares - 21.956).abs() < 0.01);

        let steps = stage_steps(&ret);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].args["to"], USDC);
        assert_eq!(steps[0].args["data"]["encode"]["signature"], ERC20_APPROVE);
        assert_eq!(
            steps[0].args["data"]["encode"]["args"],
            json!([VAULT, "25000000"])
        );
        assert!(steps[0].enforcement.is_none());
        assert_eq!(steps[1].args["to"], VAULT);
        assert_eq!(
            steps[1].args["data"]["encode"]["signature"],
            ERC4626_DEPOSIT
        );
        assert_eq!(
            steps[1].args["data"]["encode"]["args"],
            json!(["25000000", WALLET])
        );
        let enforcement = steps[1]
            .enforcement
            .as_ref()
            .expect("deposit step enforced");
        let tools: Vec<&str> = enforcement.steps.iter().map(|s| s.tool.as_str()).collect();
        assert_eq!(tools, vec!["simulate_batch", "commit_txs"]);
        assert!(enforcement.binds_alias("transaction_hash"));
    }

    #[test]
    fn deposit_plan_flags_v2_gate() {
        let ret = build_deposit_plan(DepositPlanInput {
            version: VaultVersion::V2,
            chain_id: 1,
            config: &v2_config(),
            state: &json!({}),
            detail: &json!({ "netApy": 0.05, "warnings": [{ "type": "x", "level": "YELLOW" }] }),
            wallet: WALLET,
            receiver: WALLET,
            amount_raw: "1000000",
            amount_human: "1",
        })
        .unwrap();
        let warnings = ret.value["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[1]["type"], "receive_shares_gate");
        assert!(ret.value["expected_shares_approx"].is_null());
        assert_eq!(ret.value["vault_net_apy"], 0.05);
    }

    #[test]
    fn withdraw_exact_stages_withdraw_call() {
        let position = json!({ "shares": "1000000000000000000000", "assets": "1050000000" });
        let ret = build_withdraw_plan(WithdrawPlanInput {
            version: VaultVersion::V1,
            chain_id: 1,
            config: &v1_config(),
            position: &position,
            available_assets: Some(999_000_000_000),
            force_options: Value::Null,
            wallet: WALLET,
            receiver: WALLET,
            amount_raw: Some("100000000"),
        })
        .unwrap();
        assert_eq!(ret.value["status"], "awaiting_wallet");
        assert_eq!(ret.value["mode"], "withdraw_exact");
        assert_eq!(ret.value["requested"]["human"], "100");
        let steps = stage_steps(&ret);
        assert_eq!(steps.len(), 1);
        assert_eq!(
            steps[0].args["data"]["encode"]["signature"],
            ERC4626_WITHDRAW
        );
        assert_eq!(
            steps[0].args["data"]["encode"]["args"],
            json!(["100000000", WALLET, WALLET])
        );
        assert!(
            steps[0]
                .enforcement
                .as_ref()
                .unwrap()
                .binds_alias("transaction_hash")
        );
    }

    #[test]
    fn withdraw_all_redeems_shares() {
        let position = json!({ "shares": "123456789", "assets": "50000000" });
        let ret = build_withdraw_plan(WithdrawPlanInput {
            version: VaultVersion::V2,
            chain_id: 8453,
            config: &v2_config(),
            position: &position,
            available_assets: Some(60_000_000),
            force_options: Value::Null,
            wallet: WALLET,
            receiver: USDC,
            amount_raw: None,
        })
        .unwrap();
        assert_eq!(ret.value["mode"], "redeem_all");
        let steps = stage_steps(&ret);
        assert_eq!(steps[0].args["data"]["encode"]["signature"], ERC4626_REDEEM);
        assert_eq!(
            steps[0].args["data"]["encode"]["args"],
            json!(["123456789", USDC, WALLET])
        );
    }

    #[test]
    fn withdraw_refuses_without_liquidity_or_balance() {
        let position = json!({ "shares": "10", "assets": "50000000" });
        let force = json!([{ "adapter": "0xA", "penalty_rate": 0.005 }]);
        let ret = build_withdraw_plan(WithdrawPlanInput {
            version: VaultVersion::V2,
            chain_id: 1,
            config: &v2_config(),
            position: &position,
            available_assets: Some(10_000_000),
            force_options: force.clone(),
            wallet: WALLET,
            receiver: WALLET,
            amount_raw: Some("20000000"),
        })
        .unwrap();
        assert_eq!(ret.value["status"], "insufficient_liquidity");
        assert_eq!(ret.value["force_deallocate_options"], force);
        assert!(ret.routes.is_empty());

        let ret = build_withdraw_plan(WithdrawPlanInput {
            version: VaultVersion::V1,
            chain_id: 1,
            config: &v1_config(),
            position: &position,
            available_assets: None,
            force_options: Value::Null,
            wallet: WALLET,
            receiver: WALLET,
            amount_raw: Some("60000000"),
        })
        .unwrap();
        assert_eq!(ret.value["status"], "insufficient_balance");
        assert!(ret.routes.is_empty());

        let empty = json!({ "shares": "0", "assets": "0" });
        assert!(
            build_withdraw_plan(WithdrawPlanInput {
                version: VaultVersion::V1,
                chain_id: 1,
                config: &v1_config(),
                position: &empty,
                available_assets: None,
                force_options: Value::Null,
                wallet: WALLET,
                receiver: WALLET,
                amount_raw: None,
            })
            .is_err()
        );
    }

    #[test]
    fn governance_flags_cover_v1_and_v2() {
        let warnings = vec![json!({ "type": "bad_debt", "level": "RED" })];
        let flags = governance_flags(
            VaultVersion::V1,
            Some(86_400),
            &[],
            2,
            &warnings,
            &Value::Null,
        );
        assert_eq!(
            flags,
            vec![
                "warning:RED:bad_debt".to_string(),
                "pending_governance_actions:2".to_string(),
                "short_timelock:86400s".to_string(),
            ]
        );

        let timelocks = vec![
            json!({ "functionName": "addAdapter", "duration": 0, "abdicatedAt": null }),
            json!({ "functionName": "setReceiveSharesGate", "duration": 0, "abdicatedAt": 1773146471 }),
            json!({ "functionName": "increaseAbsoluteCap", "duration": 259200, "abdicatedAt": null }),
            json!({ "functionName": "setIsAllocator", "duration": 0, "abdicatedAt": null }),
        ];
        let gates = json!({ "send_shares": null, "receive_shares": "0xGATE" });
        let flags = governance_flags(VaultVersion::V2, None, &timelocks, 0, &[], &gates);
        assert_eq!(
            flags,
            vec![
                "short_timelock:addAdapter:0s".to_string(),
                "gated:receive_shares".to_string()
            ]
        );
    }

    #[test]
    fn find_vault_rows_normalize_both_versions() {
        let v1 = json!({
            "address": VAULT, "name": "Steak", "symbol": "steakUSDC", "listed": true,
            "chain": { "id": 1 }, "asset": { "symbol": "USDC" },
            "state": { "apy": 0.05, "netApy": 0.045, "totalAssetsUsd": 1e6, "fee": 0.05, "timelock": 604800,
                       "curator": "0x1", "allRewards": [{ "asset": { "symbol": "MORPHO" }, "supplyApr": 0.01 }] },
            "warnings": [{ "type": "x", "level": "YELLOW" }]
        });
        let row = normalize_v1_row(&v1);
        assert_eq!(row["version"], "v1");
        assert_eq!(row["net_apy"], 0.045);
        assert_eq!(row["rewards"][0]["asset"], "MORPHO");
        assert_eq!(row["warnings"][0]["level"], "YELLOW");

        let v2 = json!({
            "address": "0x2", "name": "K", "symbol": "k", "listed": true, "type": "MorphoVault",
            "chain": { "id": 8453 }, "asset": { "symbol": "USDC" },
            "apy": 0.05, "netApy": 0.052, "netApyExcludingRewards": 0.048, "totalAssetsUsd": 5e6,
            "liquidityUsd": 3e5, "performanceFee": 0.05, "curator": { "address": "0xC" }, "rewards": [], "warnings": []
        });
        let row = normalize_v2_row(&v2);
        assert_eq!(row["version"], "v2");
        assert_eq!(row["curator"], "0xC");
        assert_eq!(row["liquidity_usd"], 3e5);
    }

    // ------------------------------------------------------------------
    // Live validation ladder against api.morpho.org. Run with:
    //   cargo test --manifest-path apps/morpho-vaults/Cargo.toml -- --ignored --nocapture
    // ------------------------------------------------------------------

    use aomi_sdk::testing::run_tool;

    const CURATOR_WALLET: &str = "0x255c7705e8BB334DfCae438197f7C4297988085a";
    const V2_VAULT: &str = "0x04422053aDDbc9bB2759b248B574e3FCA76Bc145";

    fn live_ctx(tool: &str, wallet: &str) -> DynToolCallCtx {
        TestCtxBuilder::new(tool)
            .attribute("domain", json!({ "evm": { "address": wallet } }))
            .build()
    }

    fn show(label: &str, v: &Value) {
        let text = serde_json::to_string(v).unwrap();
        let cut: String = text.chars().take(1200).collect();
        println!(
            "--- {label}: {cut}{}",
            if text.len() > 1200 { " ..." } else { "" }
        );
    }

    #[test]
    #[ignore]
    fn live_find_vaults() {
        let ret = run_tool::<FindVaults>(
            &MorphoVaultsApp,
            json!({ "chain_id": 1, "asset": "USDC", "limit": 5 }),
            live_ctx("morpho_find_vaults", WALLET),
        )
        .unwrap();
        show("find_vaults", &ret.value);
        assert!(ret.value["returned"].as_u64().unwrap() > 0);
        let first = &ret.value["vaults"][0];
        assert_eq!(first["asset"]["symbol"], "USDC");
        assert!(first["net_apy"].is_number());
        let by_tvl = run_tool::<FindVaults>(
            &MorphoVaultsApp,
            json!({ "chain_id": 8453, "version": "v2", "sort_by": "tvl", "limit": 3 }),
            live_ctx("morpho_find_vaults", WALLET),
        )
        .unwrap();
        show("find_vaults_v2_base", &by_tvl.value);
        assert_eq!(by_tvl.value["vaults"][0]["version"], "v2");
    }

    #[test]
    #[ignore]
    fn live_overview_allocations_history_governance() {
        for (label, addr) in [("v1", VAULT), ("v2", V2_VAULT)] {
            let args = json!({ "chain_id": 1, "address": addr });
            let ov =
                run_tool::<VaultOverview>(&MorphoVaultsApp, args.clone(), live_ctx("o", WALLET))
                    .unwrap();
            show(&format!("overview_{label}"), &ov.value);
            assert_eq!(ov.value["vault"]["version"], label);
            assert!(ov.value["state"]["total_assets"]["human"].is_string());
            assert!(ov.value["apy"]["net"].is_number());

            let al =
                run_tool::<VaultAllocations>(&MorphoVaultsApp, args.clone(), live_ctx("a", WALLET))
                    .unwrap();
            show(&format!("allocations_{label}"), &al.value);
            assert!(al.value["allocation_count"].as_u64().unwrap() > 0);
            assert!(al.value["allocations"][0]["pct_of_vault"].is_number());

            let hi = run_tool::<VaultHistory>(
                &MorphoVaultsApp,
                json!({ "chain_id": 1, "address": addr, "lookback": "7d", "max_points": 6 }),
                live_ctx("h", WALLET),
            )
            .unwrap();
            show(&format!("history_{label}"), &hi.value);
            assert!(hi.value["summary"]["apy"]["avg"].is_number());
            assert!(hi.value["apy_series"].as_array().unwrap().len() <= 6);

            let gov =
                run_tool::<VaultGovernance>(&MorphoVaultsApp, args, live_ctx("g", WALLET)).unwrap();
            show(&format!("governance_{label}"), &gov.value);
            assert!(gov.value["risk_flags"].is_array());
        }
    }

    #[test]
    #[ignore]
    fn live_user_positions() {
        let ret = run_tool::<UserVaultPositions>(
            &MorphoVaultsApp,
            json!({ "address": CURATOR_WALLET }),
            live_ctx("morpho_user_vault_positions", WALLET),
        )
        .unwrap();
        show("positions", &ret.value);
        assert!(ret.value["position_count"].as_u64().unwrap() > 0);
        assert!(ret.value["total_usd"].as_f64().unwrap() > 0.0);
        let with_v2 = ret.value["positions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["version"] == "v2");
        assert!(with_v2, "expected at least one v2 position");
    }

    #[test]
    #[ignore]
    fn live_deposit_and_withdraw_plans() {
        let dep = run_tool::<Deposit>(
            &MorphoVaultsApp,
            json!({ "chain_id": 1, "vault_address": VAULT, "amount": "25" }),
            live_ctx("morpho_deposit", WALLET),
        )
        .unwrap();
        show("deposit", &dep.value);
        assert_eq!(dep.value["status"], "awaiting_wallet");
        assert_eq!(dep.value["amount"]["raw"], "25000000");
        assert_eq!(stage_steps(&dep).len(), 2);

        // Curator fee recipient holds bbqUSDC on mainnet: exact withdraw stages a call.
        let wd = run_tool::<Withdraw>(
            &MorphoVaultsApp,
            json!({ "chain_id": 1, "vault_address": "0xBEeFFF209270748ddd194831b3fa287a5386f5bC", "amount": "100" }),
            live_ctx("morpho_withdraw", CURATOR_WALLET),
        )
        .unwrap();
        show("withdraw", &wd.value);
        assert_eq!(wd.value["status"], "awaiting_wallet");
        assert_eq!(stage_steps(&wd).len(), 1);

        // A wallet with no shares must be refused without staging.
        let none = run_tool::<Withdraw>(
            &MorphoVaultsApp,
            json!({ "chain_id": 1, "vault_address": VAULT, "all": true }),
            live_ctx("morpho_withdraw", WALLET),
        );
        assert!(none.is_err(), "expected no-position error, got {none:?}");
    }
}
