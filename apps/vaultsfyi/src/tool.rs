//! Tool layer for the vaults.fyi app.
//!
//! Tool surface (8 user-centric tools):
//!   * `vaultsfyi_find_vaults`         — ranked vault discovery with filters
//!   * `vaultsfyi_get_vault`           — full detail for one vault
//!   * `vaultsfyi_get_vault_history`   — APY / TVL / share-price time series
//!   * `vaultsfyi_get_benchmark`       — USD / ETH benchmark rates
//!   * `vaultsfyi_get_positions`       — a wallet's vault positions
//!   * `vaultsfyi_get_deposit_options` — idle balances + ranked deposit options
//!   * `vaultsfyi_get_action_context`  — what a wallet can do with one vault
//!   * `vaultsfyi_build_vault_tx`      — stage deposit / redeem / claim calldata

use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};

const SORT_BY: &[&str] = &["tvl", "apy1day", "apy7day", "apy30day"];
const APY_INTERVALS: &[&str] = &["1day", "7day", "30day"];
const GRANULARITIES: &[&str] = &["1hour", "1day", "1week"];
const ACTIONS: &[&str] = &[
    "deposit",
    "redeem",
    "request-redeem",
    "request-deposit",
    "claim-redeem",
    "claim-deposit",
    "claim-rewards",
    "start-redeem-cooldown",
];

// ============================================================================
// Shared helpers
// ============================================================================

/// Prefer an explicit `address` arg, then the session's connected EVM wallet.
pub(crate) fn resolve_wallet(arg: Option<&str>, ctx: &DynToolCallCtx) -> Result<String, String> {
    let candidate = arg
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| ctx.attribute_string(&["domain", "evm", "address"]))
        .ok_or_else(|| {
            format!("{TAG} no wallet address: connect an EVM wallet or pass `address` explicitly")
        })?;
    if !is_evm_address(&candidate) {
        return Err(format!("{TAG} `{candidate}` is not a valid 0x EVM address"));
    }
    Ok(candidate)
}

fn validate_choice(value: &str, allowed: &[&str], field: &str) -> Result<String, String> {
    let v = value.trim().to_ascii_lowercase();
    if allowed.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(format!(
            "{TAG} invalid {field} `{value}`; expected one of: {}",
            allowed.join(", ")
        ))
    }
}

fn clamp_limit(limit: Option<u32>, default: u32, max: u32) -> u32 {
    limit.unwrap_or(default).clamp(1, max)
}

fn granularity_seconds(granularity: &str) -> u64 {
    match granularity {
        "1hour" => 3_600,
        "1week" => 7 * 86_400,
        _ => 86_400,
    }
}

/// Unix timestamp `limit` buckets before now (plus one bucket of slack).
fn recent_window_start(granularity: &str, limit: u32) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_sub(granularity_seconds(granularity) * (u64::from(limit) + 1))
}

fn vault_path_segment(vault: &str) -> Result<String, String> {
    let v = vault.trim();
    if v.is_empty() {
        return Err(format!(
            "{TAG} `vault` must be a vault address or vaults.fyi vault id"
        ));
    }
    Ok(v.to_string())
}

/// Shared vault filter set for the discovery and deposit-options endpoints.
struct VaultFilters<'a> {
    assets: Option<&'a [String]>,
    networks: &'a [String],
    protocols: Option<&'a [String]>,
    tags: Option<&'a [String]>,
    min_tvl_usd: Option<u64>,
    min_score: Option<f64>,
    only_transactional: Option<bool>,
}

impl VaultFilters<'_> {
    fn apply(&self, q: &mut Query) {
        q.push_list("allowedAssets", self.assets.unwrap_or_default());
        q.push_list("allowedNetworks", self.networks);
        q.push_list("allowedProtocols", self.protocols.unwrap_or_default());
        q.push_list("tags", self.tags.unwrap_or_default());
        q.push_opt("minTvl", self.min_tvl_usd);
        q.push_opt("minVaultScore", self.min_score);
        q.push_opt("onlyTransactional", self.only_transactional);
    }
}

// ============================================================================
// vaultsfyi_find_vaults
// ============================================================================

pub(crate) struct FindVaults;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FindVaultsArgs {
    /// Underlying asset symbols to include, e.g. `["USDC"]` or `["WETH", "wstETH"]`.
    /// Omit for all assets.
    #[serde(default)]
    pub assets: Option<Vec<String>>,
    /// Networks to search: names (`base`, `mainnet`, `arbitrum`, `optimism`, `polygon`, …)
    /// or chain ids. Default: base, mainnet, arbitrum, optimism.
    #[serde(default)]
    pub networks: Option<Vec<String>>,
    /// Protocol names or ids to include, e.g. `["aave", "morpho"]`. Omit for all protocols.
    #[serde(default)]
    pub protocols: Option<Vec<String>>,
    /// Vault tags to include (e.g. `Stablecoin`, `Direct Protocol Lending`, `RWA`).
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Minimum TVL in USD. Default 1,000,000 — lower it only when the user asks for small vaults.
    #[serde(default)]
    pub min_tvl_usd: Option<u64>,
    /// Minimum vaults.fyi Reputation Score (0–100). Omit for no floor.
    #[serde(default)]
    pub min_score: Option<f64>,
    /// Only vaults that support deposit/redeem through this app. Default false; set true
    /// when the user intends to deposit.
    #[serde(default)]
    pub only_transactional: Option<bool>,
    /// Ranking key: `apy7day` (default), `apy1day`, `apy30day`, or `tvl`. Descending.
    #[serde(default)]
    pub sort_by: Option<String>,
    /// Max vaults to return (1–50). Default 10.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Zero-based page for paging through a long ranking. Default 0.
    #[serde(default)]
    pub page: Option<u32>,
    /// Optional vaults.fyi API key override (normally supplied via the VAULTS_FYI_API_KEY secret).
    #[serde(default)]
    pub api_key: Option<String>,
}

impl DynAomiTool for FindVaults {
    type App = VaultsFyiApp;
    type Args = FindVaultsArgs;
    const NAME: &'static str = "vaultsfyi_find_vaults";
    const DESCRIPTION: &'static str = "Search and rank DeFi yield vaults across 80+ protocols and 20+ EVM networks via vaults.fyi. Filter by asset symbol, network, protocol, tag, minimum TVL, minimum Reputation Score, and transactional support; sort by 7-day APY (default), 1-day / 30-day APY, or TVL. Use for 'best USDC yield on Base', 'safest ETH vaults', or 'compare Morpho vs Aave vaults'. Returns compact rows (vault_id, address, network, APY %, TVL, score); pass `network` + `address` to `vaultsfyi_get_vault` for full detail. This is the most expensive read — keep `limit` small and filters tight.";

    fn run(_app: &VaultsFyiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Client::from_ctx(&ctx, args.api_key.as_deref())?;
        let networks = resolve_networks(args.networks.as_deref())?;
        let sort_by = validate_choice(
            args.sort_by.as_deref().unwrap_or("apy7day"),
            SORT_BY,
            "sort_by",
        )?;
        let limit = clamp_limit(args.limit, 10, 50);
        let min_tvl = args.min_tvl_usd.unwrap_or(1_000_000);

        let mut q = Query::new();
        VaultFilters {
            assets: args.assets.as_deref(),
            networks: &networks,
            protocols: args.protocols.as_deref(),
            tags: args.tags.as_deref(),
            min_tvl_usd: Some(min_tvl),
            min_score: args.min_score,
            only_transactional: args.only_transactional,
        }
        .apply(&mut q);
        q.push("sortBy", &sort_by)
            .push("sortOrder", "desc")
            .push("perPage", limit)
            .push("page", args.page.unwrap_or(0));

        let raw = client.get("/v2/detailed-vaults", &q)?;
        let vaults: Vec<Value> = raw
            .get("data")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(summarize_vault).collect())
            .unwrap_or_default();

        Ok(ok_with_errors(
            json!({
                "filters": {
                    "assets": args.assets,
                    "networks": if networks.is_empty() { json!("default (base, mainnet, arbitrum, optimism)") } else { json!(networks) },
                    "protocols": args.protocols,
                    "tags": args.tags,
                    "min_tvl_usd": min_tvl,
                    "min_score": args.min_score,
                    "only_transactional": args.only_transactional.unwrap_or(false),
                    "sort_by": sort_by,
                },
                "count": vaults.len(),
                "next_page": raw.get("nextPage"),
                "apy_note": "apy_pct values are percentages (already net of fees); `total` = base + reward",
                "vaults": vaults,
            }),
            &raw,
        ))
    }
}

// ============================================================================
// vaultsfyi_get_vault
// ============================================================================

pub(crate) struct GetVault;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetVaultArgs {
    /// Network name (`base`, `mainnet`, `arbitrum`, …) or chain id.
    pub network: String,
    /// Vault contract address (0x…) or vaults.fyi `vault_id`.
    pub vault: String,
    /// Optional vaults.fyi API key override.
    #[serde(default)]
    pub api_key: Option<String>,
}

impl DynAomiTool for GetVault {
    type App = VaultsFyiApp;
    type Args = GetVaultArgs;
    const NAME: &'static str = "vaultsfyi_get_vault";
    const DESCRIPTION: &'static str = "Full detail for one vault: APY breakdown over 1/7/30 days (base vs reward), TVL, fees, Reputation Score components, holder concentration, reward tokens, capacity, warnings and flags, curator, and whether deposits/redeems are instant or multi-step. Use after `vaultsfyi_find_vaults` when the user zooms in on a vault, or when they name a vault address directly.";

    fn run(_app: &VaultsFyiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Client::from_ctx(&ctx, args.api_key.as_deref())?;
        let network = resolve_network(&args.network)?;
        let vault = vault_path_segment(&args.vault)?;
        let raw = client.get(
            &format!("/v2/detailed-vaults/{}/{vault}", network.name),
            &Query::new(),
        )?;
        Ok(ok(json!({ "vault": vault_detail(&raw) })))
    }
}

// ============================================================================
// vaultsfyi_get_vault_history
// ============================================================================

pub(crate) struct GetVaultHistory;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetVaultHistoryArgs {
    /// Network name or chain id.
    pub network: String,
    /// Vault contract address (0x…) or vaults.fyi `vault_id`.
    pub vault: String,
    /// APY trailing window for each point: `1day`, `7day` (default), or `30day`.
    #[serde(default)]
    pub apy_interval: Option<String>,
    /// Point spacing: `1hour`, `1day` (default), or `1week`.
    #[serde(default)]
    pub granularity: Option<String>,
    /// Unix timestamp (seconds) lower bound. Omit for the most recent points.
    #[serde(default)]
    pub from_timestamp: Option<u64>,
    /// Unix timestamp (seconds) upper bound.
    #[serde(default)]
    pub to_timestamp: Option<u64>,
    /// Max points to return (1–365). Default 30.
    #[serde(default)]
    pub limit: Option<u32>,
    /// Optional vaults.fyi API key override.
    #[serde(default)]
    pub api_key: Option<String>,
}

impl DynAomiTool for GetVaultHistory {
    type App = VaultsFyiApp;
    type Args = GetVaultHistoryArgs;
    const NAME: &'static str = "vaultsfyi_get_vault_history";
    const DESCRIPTION: &'static str = "Time series of APY (base / reward / total), TVL, and share price for one vault, at hourly, daily, or weekly granularity with an optional timestamp range. Use for 'how stable has this yield been', 'APY over the last 90 days', or to compare a vault's history against `vaultsfyi_get_benchmark`. Returns points plus min / max / average total APY over the window.";

    fn run(_app: &VaultsFyiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Client::from_ctx(&ctx, args.api_key.as_deref())?;
        let network = resolve_network(&args.network)?;
        let vault = vault_path_segment(&args.vault)?;
        let apy_interval = validate_choice(
            args.apy_interval.as_deref().unwrap_or("7day"),
            APY_INTERVALS,
            "apy_interval",
        )?;
        let granularity = validate_choice(
            args.granularity.as_deref().unwrap_or("1day"),
            GRANULARITIES,
            "granularity",
        )?;
        let limit = clamp_limit(args.limit, 30, 365);
        // The API pages oldest-first; with no bounds, ask for the most
        // recent `limit` buckets instead of the vault's first days.
        let from_timestamp = match (args.from_timestamp, args.to_timestamp) {
            (None, None) => Some(recent_window_start(&granularity, limit)),
            (from, _) => from,
        };

        let mut q = Query::new();
        q.push("apyInterval", &apy_interval)
            .push("granularity", &granularity)
            .push("perPage", limit)
            .push("page", 0)
            .push_opt("fromTimestamp", from_timestamp)
            .push_opt("toTimestamp", args.to_timestamp);

        let raw = client.get(&format!("/v2/historical/{}/{vault}", network.name), &q)?;
        let points: Vec<Value> = raw
            .get("data")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|p| {
                        json!({
                            "timestamp": p.get("timestamp"),
                            "apy_pct": {
                                "total": pct(num(p.pointer("/apy/total"))),
                                "base": pct(num(p.pointer("/apy/base"))),
                                "reward": pct(num(p.pointer("/apy/reward"))),
                            },
                            "tvl_usd": num(p.pointer("/tvl/usd")),
                            "share_price": num(p.get("sharePrice")),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let totals: Vec<f64> = points
            .iter()
            .filter_map(|p| p.pointer("/apy_pct/total").and_then(Value::as_f64))
            .collect();
        let stats = if totals.is_empty() {
            Value::Null
        } else {
            let min = totals.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = totals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let avg = totals.iter().sum::<f64>() / totals.len() as f64;
            json!({
                "points": totals.len(),
                "total_apy_pct": { "min": min, "max": max, "avg": (avg * 10_000.0).round() / 10_000.0 },
                "first_timestamp": points.first().and_then(|p| p.get("timestamp")),
                "last_timestamp": points.last().and_then(|p| p.get("timestamp")),
            })
        };

        Ok(ok(json!({
            "network": network.name,
            "vault": vault,
            "apy_interval": apy_interval,
            "granularity": granularity,
            "summary": stats,
            "next_page": raw.get("nextPage"),
            "points": points,
        })))
    }
}

// ============================================================================
// vaultsfyi_get_benchmark
// ============================================================================

pub(crate) struct GetBenchmark;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetBenchmarkArgs {
    /// Network name or chain id. Default `mainnet`.
    #[serde(default)]
    pub network: Option<String>,
    /// Benchmark code: `usd` (default, stablecoin lending) or `eth`.
    #[serde(default)]
    pub code: Option<String>,
    /// Optional vaults.fyi API key override.
    #[serde(default)]
    pub api_key: Option<String>,
}

impl DynAomiTool for GetBenchmark {
    type App = VaultsFyiApp;
    type Args = GetBenchmarkArgs;
    const NAME: &'static str = "vaultsfyi_get_benchmark";
    const DESCRIPTION: &'static str = "The vaults.fyi USD or ETH benchmark APY for a network — a TVL-weighted rate across the largest vaults — over 1, 7, and 30 days. Use to answer 'is X% a good yield right now' or to contextualise a vault's APY against the market.";

    fn run(_app: &VaultsFyiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Client::from_ctx(&ctx, args.api_key.as_deref())?;
        let network = resolve_network(args.network.as_deref().unwrap_or("mainnet"))?;
        let code = validate_choice(
            args.code.as_deref().unwrap_or("usd"),
            &["usd", "eth"],
            "code",
        )?;
        let mut q = Query::new();
        q.push("code", &code);
        let raw = client.get(&format!("/v2/benchmarks/{}", network.name), &q)?;
        let interval = |key: &str| {
            json!({
                "total": pct(num(raw.pointer(&format!("/apy/{key}/total")))),
                "base": pct(num(raw.pointer(&format!("/apy/{key}/base")))),
                "reward": pct(num(raw.pointer(&format!("/apy/{key}/reward")))),
            })
        };
        Ok(ok(json!({
            "network": network.name,
            "code": code,
            "benchmark_apy_pct": { "1day": interval("1day"), "7day": interval("7day"), "30day": interval("30day") },
            "timestamp": raw.get("timestamp"),
        })))
    }
}

// ============================================================================
// vaultsfyi_get_positions
// ============================================================================

pub(crate) struct GetPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPositionsArgs {
    /// Wallet address (0x…). Defaults to the connected EVM wallet.
    #[serde(default)]
    pub address: Option<String>,
    /// Restrict to these networks (names or chain ids). Default: all supported networks.
    #[serde(default)]
    pub networks: Option<Vec<String>>,
    /// Hide positions worth less than this many USD. Default 1.
    #[serde(default)]
    pub min_usd: Option<f64>,
    /// Optional vaults.fyi API key override.
    #[serde(default)]
    pub api_key: Option<String>,
}

impl DynAomiTool for GetPositions {
    type App = VaultsFyiApp;
    type Args = GetPositionsArgs;
    const NAME: &'static str = "vaultsfyi_get_positions";
    const DESCRIPTION: &'static str = "List a wallet's active vault positions across every protocol and network vaults.fyi tracks: vault, network, asset, position value (USD and in-asset), unclaimed rewards, and the current APY. Also returns the total USD value. Use for 'what am I earning', 'show my DeFi positions', or before a redeem to find the vault.";

    fn run(_app: &VaultsFyiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Client::from_ctx(&ctx, args.api_key.as_deref())?;
        let address = resolve_wallet(args.address.as_deref(), &ctx)?;
        let networks = resolve_networks(args.networks.as_deref())?;
        let mut q = Query::new();
        q.push_list("allowedNetworks", &networks)
            .push("minUsdAssetValueThreshold", args.min_usd.unwrap_or(1.0))
            .push("apyInterval", "7day");
        let raw = client.get(&format!("/v2/portfolio/positions/{address}"), &q)?;
        let positions: Vec<Value> = raw
            .get("data")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(summarize_position).collect())
            .unwrap_or_default();
        let total_usd: f64 = positions
            .iter()
            .filter_map(|p| p.get("balance_usd").and_then(Value::as_f64))
            .sum();
        let unclaimed_usd: f64 = positions
            .iter()
            .filter_map(|p| p.get("unclaimed_usd").and_then(Value::as_f64))
            .sum();
        Ok(ok_with_errors(
            json!({
                "address": address,
                "networks": if networks.is_empty() { json!("all") } else { json!(networks) },
                "position_count": positions.len(),
                "total_usd": (total_usd * 100.0).round() / 100.0,
                "unclaimed_rewards_usd": (unclaimed_usd * 100.0).round() / 100.0,
                "positions": positions,
            }),
            &raw,
        ))
    }
}

// ============================================================================
// vaultsfyi_get_deposit_options
// ============================================================================

pub(crate) struct GetDepositOptions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetDepositOptionsArgs {
    /// Wallet address (0x…). Defaults to the connected EVM wallet.
    #[serde(default)]
    pub address: Option<String>,
    /// Restrict to these networks (names or chain ids). Default: all supported networks.
    #[serde(default)]
    pub networks: Option<Vec<String>>,
    /// Restrict to these asset symbols, e.g. `["USDC", "USDT"]`.
    #[serde(default)]
    pub assets: Option<Vec<String>>,
    /// Protocol names or ids to include.
    #[serde(default)]
    pub protocols: Option<Vec<String>>,
    /// Minimum vault TVL in USD. Default 1,000,000.
    #[serde(default)]
    pub min_tvl_usd: Option<u64>,
    /// Minimum Reputation Score (0–100). Omit for no floor.
    #[serde(default)]
    pub min_score: Option<f64>,
    /// Ignore wallet balances below this many USD. Default 1.
    #[serde(default)]
    pub min_balance_usd: Option<f64>,
    /// Max vault suggestions per idle asset (1–10). Default 3.
    #[serde(default)]
    pub max_vaults_per_asset: Option<u32>,
    /// Optional vaults.fyi API key override.
    #[serde(default)]
    pub api_key: Option<String>,
}

impl DynAomiTool for GetDepositOptions {
    type App = VaultsFyiApp;
    type Args = GetDepositOptionsArgs;
    const NAME: &'static str = "vaultsfyi_get_deposit_options";
    const DESCRIPTION: &'static str = "Scan a wallet's idle (non-yielding) token balances and rank transactional vaults each balance could be deposited into, with APY and projected annual USD earnings. Only vaults this app can actually deposit into are returned. Use for 'what should I do with my idle USDC', 'where can I earn on what's in my wallet', or as the discovery step before `vaultsfyi_build_vault_tx`.";

    fn run(_app: &VaultsFyiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Client::from_ctx(&ctx, args.api_key.as_deref())?;
        let address = resolve_wallet(args.address.as_deref(), &ctx)?;
        let networks = resolve_networks(args.networks.as_deref())?;
        let mut q = Query::new();
        VaultFilters {
            assets: args.assets.as_deref(),
            networks: &networks,
            protocols: args.protocols.as_deref(),
            tags: None,
            min_tvl_usd: Some(args.min_tvl_usd.unwrap_or(1_000_000)),
            min_score: args.min_score,
            only_transactional: Some(true),
        }
        .apply(&mut q);
        q.push(
            "minUsdAssetValueThreshold",
            args.min_balance_usd.unwrap_or(1.0),
        )
        .push(
            "maxVaultsPerAsset",
            clamp_limit(args.max_vaults_per_asset, 3, 10),
        )
        .push("alwaysReturnAssets", true)
        .push("apyInterval", "7day");
        let raw = client.get(&format!("/v2/portfolio/best-deposit-options/{address}"), &q)?;

        let balances: Vec<Value> = raw
            .get("userBalances")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|b| {
                        let asset = b.get("asset");
                        let decimals = asset.and_then(|a| a.get("decimals")).and_then(Value::as_u64).map(|d| d as u32);
                        let raw_balance = asset.and_then(|a| a.get("balanceNative")).and_then(Value::as_str);
                        json!({
                            "asset": {
                                "symbol": asset.and_then(|a| a.get("symbol")),
                                "address": asset.and_then(|a| a.get("address")),
                                "decimals": decimals,
                                "network": asset.and_then(|a| a.pointer("/network/name")).or_else(|| b.pointer("/network/name")),
                            },
                            "balance": match (raw_balance, decimals) { (Some(r), Some(d)) => from_base_units(r, d), _ => None },
                            "balance_native": raw_balance,
                            "balance_usd": num(asset.and_then(|a| a.get("balanceUsd"))),
                            "deposit_options": b
                                .get("depositOptions")
                                .and_then(Value::as_array)
                                .map(|o| o.iter().map(summarize_deposit_option).collect::<Vec<_>>())
                                .unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(ok_with_errors(
            json!({
                "address": address,
                "networks": if networks.is_empty() { json!("all") } else { json!(networks) },
                "idle_asset_count": balances.len(),
                "note": "deposit_options are limited to transactional vaults; apy_pct values are percentages",
                "balances": balances,
            }),
            &raw,
        ))
    }
}

// ============================================================================
// vaultsfyi_get_action_context
// ============================================================================

pub(crate) struct GetActionContext;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetActionContextArgs {
    /// Network name or chain id.
    pub network: String,
    /// Vault contract address (0x…) or vaults.fyi `vault_id`.
    pub vault: String,
    /// Wallet address (0x…). Defaults to the connected EVM wallet.
    #[serde(default)]
    pub address: Option<String>,
    /// Optional vaults.fyi API key override.
    #[serde(default)]
    pub api_key: Option<String>,
}

impl DynAomiTool for GetActionContext {
    type App = VaultsFyiApp;
    type Args = GetActionContextArgs;
    const NAME: &'static str = "vaultsfyi_get_action_context";
    const DESCRIPTION: &'static str = "What a wallet can do with one vault right now: available actions (deposit, redeem, request-redeem, claim-redeem, claim-rewards, …), the current deposit / redeem step, the wallet's asset balance, position value, deposit limit, pending redeem requests and cooldowns, and claimable rewards. Call this before `vaultsfyi_build_vault_tx` to confirm the action is available and to show the user their balance. An empty `available_actions` means the vault is analytics-only.";

    fn run(_app: &VaultsFyiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = Client::from_ctx(&ctx, args.api_key.as_deref())?;
        let network = resolve_network(&args.network)?;
        let vault = vault_path_segment(&args.vault)?;
        let address = resolve_wallet(args.address.as_deref(), &ctx)?;
        let raw = client.get(
            &format!(
                "/v2/transactions/context/{address}/{}/{vault}",
                network.name
            ),
            &Query::new(),
        )?;
        let mut out = summarize_context(&raw);
        if let Value::Object(map) = &mut out {
            map.insert("address".into(), json!(address));
            map.insert("network".into(), json!(network.name));
            map.insert("chain_id".into(), json!(network.chain_id));
            map.insert("vault".into(), json!(vault));
        }
        Ok(ok(out))
    }
}

// ============================================================================
// vaultsfyi_build_vault_tx
// ============================================================================

pub(crate) struct BuildVaultTx;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BuildVaultTxArgs {
    /// Network name or chain id of the vault.
    pub network: String,
    /// Vault contract address (0x…) or vaults.fyi `vault_id`.
    pub vault: String,
    /// Vault action: `deposit`, `redeem`, `request-redeem`, `request-deposit`, `claim-redeem`,
    /// `claim-deposit`, `claim-rewards`, or `start-redeem-cooldown`. Must appear in the
    /// vault's `available_actions` from `vaultsfyi_get_action_context`.
    pub action: String,
    /// Amount in human units of the asset (e.g. `"100"` or `"0.25"`), NOT base units.
    /// Required for deposit / request-deposit / request-redeem, and for redeem unless `all` is true.
    #[serde(default)]
    pub amount: Option<String>,
    /// Redeem the entire position (redeem action only). Overrides `amount`.
    #[serde(default)]
    pub all: Option<bool>,
    /// Asset contract address to use. Defaults to the vault's primary underlying asset;
    /// set it only for multi-asset vaults (see `additional_assets` in the action context).
    #[serde(default)]
    pub asset_address: Option<String>,
    /// Wallet address (0x…). Defaults to the connected EVM wallet.
    #[serde(default)]
    pub address: Option<String>,
    /// Optional vaults.fyi API key override.
    #[serde(default)]
    pub api_key: Option<String>,
}

fn action_needs_amount(action: &str) -> bool {
    matches!(action, "deposit" | "request-deposit" | "request-redeem")
}

impl DynAomiTool for BuildVaultTx {
    type App = VaultsFyiApp;
    type Args = BuildVaultTxArgs;
    const NAME: &'static str = "vaultsfyi_build_vault_tx";
    const DESCRIPTION: &'static str = "Build and stage the ready-to-sign transaction(s) for a vault action — deposit, redeem (partial or `all`), request/claim steps for multi-step vaults, or claim rewards — for the connected wallet. Amounts are human units (`\"100\"` = 100 USDC). Fetches the action context first to validate the action and asset decimals, then returns vaults.fyi calldata verbatim as a staging plan: the host injects [[SYSTEM:...]] next-step prompts that drive `stage_tx` (one per returned tx, approvals first) → `simulate_batch` → `commit_txs`. Call ONLY after the user has confirmed vault, network, action, and amount. Nothing is executed until the wallet signs; report success only when a transaction_hash comes back.";

    fn run_with_routes(
        _app: &VaultsFyiApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let client = Client::from_ctx(&ctx, args.api_key.as_deref())?;
        let network = resolve_network(&args.network)?;
        let vault = vault_path_segment(&args.vault)?;
        let address = resolve_wallet(args.address.as_deref(), &ctx)?;
        let action = validate_choice(&args.action, ACTIONS, "action")?;
        let redeem_all = args.all.unwrap_or(false) && action == "redeem";

        // 1. Context: confirms the action is live for this wallet and gives us
        //    the asset address + decimals for amount conversion.
        let context = client.get(
            &format!(
                "/v2/transactions/context/{address}/{}/{vault}",
                network.name
            ),
            &Query::new(),
        )?;
        let available = available_actions(&context);
        if available.is_empty() {
            return Err(format!(
                "{TAG} vault {vault} on {} is analytics-only (no transactional support); pick a vault with is_transactional = true",
                network.name
            ));
        }
        if !available.iter().any(|a| a == &action) {
            return Err(format!(
                "{TAG} action `{action}` is not available for this wallet on vault {vault} right now; available: {} (current deposit step: {}, current redeem step: {})",
                available.join(", "),
                context
                    .get("currentDepositStep")
                    .and_then(Value::as_str)
                    .unwrap_or("n/a"),
                context
                    .get("currentRedeemStep")
                    .and_then(Value::as_str)
                    .unwrap_or("n/a"),
            ));
        }

        let asset_address = args
            .asset_address
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .or_else(|| {
                context
                    .pointer("/asset/address")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .ok_or_else(|| {
                format!("{TAG} could not determine the asset address; pass `asset_address`")
            })?;
        if !is_evm_address(&asset_address) {
            return Err(format!(
                "{TAG} `asset_address` `{asset_address}` is not a valid 0x address"
            ));
        }
        let (decimals, symbol) = asset_in_context(&context, &asset_address).ok_or_else(|| {
            format!(
                "{TAG} asset {asset_address} is not one of this vault's assets; use an address from `asset` or `additional_assets` in vaultsfyi_get_action_context"
            )
        })?;

        // 2. Amount handling in exact base units.
        let amount_human = args
            .amount
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let amount_base = if redeem_all {
            None
        } else if action_needs_amount(&action) || action == "redeem" {
            let human = amount_human.ok_or_else(|| {
                if action == "redeem" {
                    format!("{TAG} redeem needs `amount` (human units) or `all: true`")
                } else {
                    format!("{TAG} `{action}` needs `amount` in human units of {symbol}")
                }
            })?;
            Some(to_base_units(human, decimals)?)
        } else {
            None
        };

        // 3. Payload: vaults.fyi returns ordered, ready-to-sign txs.
        let mut q = Query::new();
        q.push("assetAddress", &asset_address);
        if redeem_all {
            q.push("all", true);
        }
        q.push_opt("amount", amount_base.as_deref());
        let payload = client.get(
            &format!(
                "/v2/transactions/{action}/{address}/{}/{vault}",
                network.name
            ),
            &q,
        )?;
        let steps = extract_steps(&payload, network.chain_id, &action)?;

        let amount_label = if redeem_all {
            format!("entire {symbol} position")
        } else {
            amount_human
                .map(|a| format!("{a} {symbol}"))
                .unwrap_or_else(|| symbol.clone())
        };
        let summary = format!(
            "{action} {amount_label} — vault {vault} on {}",
            network.name
        );
        let preview = ok(json!({
            "status": "awaiting_wallet",
            "action": action,
            "network": network.name,
            "chain_id": network.chain_id,
            "vault": vault,
            "wallet": address,
            "asset": { "symbol": symbol, "address": asset_address, "decimals": decimals },
            "amount": amount_human,
            "amount_base_units": amount_base,
            "redeem_all": redeem_all,
            "tx_count": steps.len(),
            "steps": steps.iter().map(|s| json!({
                "name": s.name,
                "kind": s.kind,
                "to": s.to,
                "value": s.value,
                "calldata_bytes": s.data.len().saturating_sub(2) / 2,
                "simulation": s.simulation,
            })).collect::<Vec<_>>(),
            "wallet_balance_before": {
                "asset": context.pointer("/asset/balanceNative"),
                "asset_usd": num(context.pointer("/asset/balanceUsd")),
                "position_value_in_asset": context.pointer("/asset/positionValueInAsset"),
            },
            "note": format!(
                "The wallet must be on chain {} ({}). The host will stage {} transaction(s), simulate them, and prompt the wallet once.",
                network.chain_id, network.name, steps.len()
            ),
        }));
        stage_route(preview, &steps, &summary)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use aomi_sdk::testing::TestCtxBuilder;

    #[test]
    fn resolve_network_accepts_names_aliases_ids_and_caip() {
        assert_eq!(resolve_network("base").unwrap().chain_id, 8453);
        assert_eq!(resolve_network("Ethereum").unwrap().name, "mainnet");
        assert_eq!(resolve_network("eth").unwrap().name, "mainnet");
        assert_eq!(resolve_network("42161").unwrap().name, "arbitrum");
        assert_eq!(resolve_network("eip155:10").unwrap().name, "optimism");
        assert_eq!(resolve_network(" matic ").unwrap().name, "polygon");
        assert!(resolve_network("solana").is_err());
        assert!(resolve_network("").is_err());
    }

    #[test]
    fn resolve_networks_dedupes_and_canonicalises() {
        let out = resolve_networks(Some(&["eth".into(), "mainnet".into(), "8453".into()])).unwrap();
        assert_eq!(out, vec!["mainnet".to_string(), "base".to_string()]);
        assert!(resolve_networks(None).unwrap().is_empty());
    }

    #[test]
    fn to_base_units_is_exact() {
        assert_eq!(to_base_units("100", 6).unwrap(), "100000000");
        assert_eq!(to_base_units("100.5", 6).unwrap(), "100500000");
        assert_eq!(to_base_units("0.000001", 6).unwrap(), "1");
        assert_eq!(
            to_base_units("1,000.25", 18).unwrap(),
            "1000250000000000000000"
        );
        assert_eq!(to_base_units(".5", 2).unwrap(), "50");
        assert!(to_base_units("0.0000001", 6).is_err());
        assert!(to_base_units("0", 6).is_err());
        assert!(to_base_units("-1", 6).is_err());
        assert!(to_base_units("1e6", 6).is_err());
        assert!(to_base_units("", 6).is_err());
    }

    #[test]
    fn from_base_units_round_trips() {
        assert_eq!(from_base_units("100500000", 6).unwrap(), "100.5");
        assert_eq!(from_base_units("1", 6).unwrap(), "0.000001");
        assert_eq!(from_base_units("1000000", 6).unwrap(), "1");
        assert_eq!(from_base_units("0", 18).unwrap(), "0");
        assert!(from_base_units("abc", 6).is_none());
    }

    #[test]
    fn pct_converts_raw_decimals() {
        assert_eq!(pct(Some(0.0543)), Some(5.43));
        assert_eq!(pct(Some(0.123456789)), Some(12.3457));
        assert_eq!(pct(None), None);
        assert_eq!(num(Some(&json!("12.5"))), Some(12.5));
        assert_eq!(num(Some(&json!(3))), Some(3.0));
        assert_eq!(num(Some(&json!(null))), None);
    }

    #[test]
    fn resolve_wallet_prefers_arg_then_ctx_and_validates() {
        let ctx = TestCtxBuilder::new("t")
            .attribute(
                "domain",
                json!({ "evm": { "address": "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045" } }),
            )
            .build();
        assert_eq!(
            resolve_wallet(Some("0x0000000000000000000000000000000000000001"), &ctx).unwrap(),
            "0x0000000000000000000000000000000000000001"
        );
        assert_eq!(
            resolve_wallet(None, &ctx).unwrap(),
            "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"
        );
        assert!(resolve_wallet(Some("not-an-address"), &ctx).is_err());
        let empty = TestCtxBuilder::new("t").build();
        assert!(resolve_wallet(None, &empty).is_err());
    }

    #[test]
    fn query_serialises_arrays_as_repeated_keys() {
        let mut q = Query::new();
        q.push_list(
            "allowedNetworks",
            &["base".into(), "mainnet".into(), " ".into()],
        )
        .push_opt("minTvl", Some(5u64))
        .push_opt("maxTvl", None::<u64>);
        assert_eq!(
            q.0,
            vec![
                ("allowedNetworks".to_string(), "base".to_string()),
                ("allowedNetworks".to_string(), "mainnet".to_string()),
                ("minTvl".to_string(), "5".to_string()),
            ]
        );
    }

    #[test]
    fn map_error_is_short_and_actionable() {
        let body = r#"{"statusCode":400,"error":"Bad Request","message":"minTvl must be integer"}"#;
        assert!(map_error(400, body, "/v2/detailed-vaults").contains("minTvl must be integer"));
        assert!(map_error(401, "", "/x").contains("VAULTS_FYI_API_KEY"));
        assert!(map_error(402, "{}", "/x").contains("portal.vaults.fyi"));
        assert!(map_error(429, "<html>big</html>", "/x").contains("rate limited"));
        assert!(map_error(503, "<html>oops</html>", "/x").contains("503"));
    }

    fn sample_vault() -> Value {
        json!({
            "vaultId": "morpho-steakhouse-usdc-base",
            "address": "0xbeef010f9cb27031ad51e3333f9af9c6b1228183",
            "name": "Steakhouse USDC",
            "network": { "name": "base", "chainId": 8453, "networkCaip": "eip155:8453" },
            "asset": { "symbol": "USDC", "address": "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913", "decimals": 6, "assetPriceInUsd": "1.0" },
            "protocol": { "protocolId": "morpho-v1", "name": "morpho", "displayName": "Morpho", "version": "v1" },
            "apy": {
                "7day": { "base": 0.0412, "reward": 0.0101, "total": 0.0513 },
                "30day": { "base": 0.04, "reward": 0.01, "total": 0.05 }
            },
            "tvl": { "usd": "123456789.12", "native": "123456789120000" },
            "score": { "vaultScore": 92.5 },
            "isTransactional": true,
            "tags": ["Stablecoin"],
            "warnings": ["a", "b"],
            "transactionalProperties": { "depositStepsType": "instant", "redeemStepsType": "instant", "rewardsSupported": true },
            "fees": { "performanceFee": 0.1, "managementFee": 0 },
            "lendUrl": "https://app.vaults.fyi/x"
        })
    }

    #[test]
    fn summarize_vault_normalises_shape() {
        let v = summarize_vault(&sample_vault());
        assert_eq!(v["vault_id"], "morpho-steakhouse-usdc-base");
        assert_eq!(v["chain_id"], 8453);
        assert_eq!(v["apy_pct"]["7day"]["total"], 5.13);
        assert_eq!(v["apy_pct"]["1day"]["total"], Value::Null);
        assert_eq!(v["tvl_usd"], 123456789.12);
        assert_eq!(v["protocol"]["name"], "Morpho");
        assert_eq!(v["asset"]["decimals"], 6);
        assert_eq!(v["warning_count"], 2);
        let d = vault_detail(&sample_vault());
        assert_eq!(d["fees_pct"]["performance"], 10.0);
        assert_eq!(d["warnings"], json!(["a", "b"]));
        assert!(d.get("warning_count").is_none());
    }

    fn sample_context() -> Value {
        json!({
            "currentDepositStep": "deposit",
            "depositSteps": [{ "name": "deposit", "actions": ["approve", "deposit"] }],
            "currentRedeemStep": "redeem",
            "redeemSteps": [{ "name": "redeem", "actions": ["redeem"] }],
            "asset": { "symbol": "USDC", "address": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "decimals": 6, "balanceNative": "250000000", "balanceUsd": "250" },
            "additionalAssets": [{ "symbol": "USDT", "address": "0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2", "decimals": 6 }],
            "rewards": { "claimable": [], "steps": [{ "name": "claim-rewards", "actions": ["claim"] }] }
        })
    }

    #[test]
    fn context_helpers_extract_actions_and_assets() {
        let c = sample_context();
        assert_eq!(
            available_actions(&c),
            vec!["deposit", "redeem", "claim-rewards"]
        );
        assert_eq!(
            asset_in_context(&c, "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            Some((6, "USDC".to_string()))
        );
        assert_eq!(
            asset_in_context(&c, "0xFDE4C96C8593536E31F229EA8F37B2ADA2699BB2"),
            Some((6, "USDT".to_string()))
        );
        assert!(asset_in_context(&c, "0x0000000000000000000000000000000000000001").is_none());
        let s = summarize_context(&c);
        assert_eq!(s["asset"]["wallet_balance"], "250");
        assert_eq!(
            s["available_actions"],
            json!(["deposit", "redeem", "claim-rewards"])
        );
        assert!(available_actions(&json!({})).is_empty());
    }

    fn sample_payload() -> Value {
        json!({
            "currentActionIndex": 0,
            "actions": [
                { "name": "approve", "tx": { "to": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913", "chainId": 8453, "data": "0x095ea7b3000000", "value": "0" } },
                { "name": "deposit", "tx": { "to": "0xBEEF010f9cb27031ad51e3333f9af9C6B1228183", "chainId": 8453, "data": "0x6e553f6500000000", "value": "0" },
                  "simulation": { "url": "https://tenderly/x", "status": "success" } }
            ]
        })
    }

    #[test]
    fn extract_steps_keeps_calldata_verbatim_and_orders_steps() {
        let steps = extract_steps(&sample_payload(), 8453, "deposit").unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].kind, "erc20_approve");
        assert_eq!(steps[0].data, "0x095ea7b3000000");
        assert_eq!(steps[1].kind, "deposit");
        assert_eq!(steps[1].to, "0xBEEF010f9cb27031ad51e3333f9af9C6B1228183");
        assert_eq!(steps[1].simulation.as_ref().unwrap()["status"], "success");
    }

    #[test]
    fn extract_steps_skips_completed_actions_and_rejects_bad_payloads() {
        let mut p = sample_payload();
        p["currentActionIndex"] = json!(1);
        let steps = extract_steps(&p, 8453, "deposit").unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "deposit");

        assert!(
            extract_steps(&sample_payload(), 1, "deposit")
                .unwrap_err()
                .contains("chain 8453")
        );
        p["currentActionIndex"] = json!(2);
        assert!(
            extract_steps(&p, 8453, "deposit")
                .unwrap_err()
                .contains("no pending transactions")
        );
        let mut bad = sample_payload();
        bad["actions"][0]["tx"]["data"] = json!("deadbeef");
        assert!(
            extract_steps(&bad, 8453, "deposit")
                .unwrap_err()
                .contains("non-hex")
        );
        let mut bad_to = sample_payload();
        bad_to["actions"][0]["tx"]["to"] = json!("0x123");
        assert!(
            extract_steps(&bad_to, 8453, "deposit")
                .unwrap_err()
                .contains("invalid `to`")
        );
    }

    #[test]
    fn stage_route_emits_one_stage_tx_per_step_with_enforced_commit_on_last() {
        let steps = extract_steps(&sample_payload(), 8453, "deposit").unwrap();
        let ret = stage_route(
            json!({ "status": "awaiting_wallet" }),
            &steps,
            "deposit 100 USDC",
        )
        .unwrap();
        assert_eq!(ret.routes.len(), 2);
        for (route, step) in ret.routes.iter().zip(steps.iter()) {
            assert_eq!(route.tool, "stage_tx");
            assert_eq!(route.args["data"]["raw"], step.data);
            assert_eq!(route.args["to"], step.to);
            assert_eq!(route.args["kind"], step.kind);
        }
        assert!(ret.routes[0].enforcement.is_none());
        let enforcement = ret.routes[1]
            .enforcement
            .as_ref()
            .expect("last step enforced");
        let tools: Vec<&str> = enforcement.steps.iter().map(|s| s.tool.as_str()).collect();
        assert_eq!(tools, vec!["simulate_batch", "commit_txs"]);
        assert_eq!(
            enforcement.steps[1].bind_as.as_deref(),
            Some("transaction_hash")
        );
    }

    #[test]
    fn recent_window_is_limit_buckets_back() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let start = recent_window_start("1day", 30);
        assert!(now - start >= 30 * 86_400 && now - start <= 32 * 86_400);
        assert_eq!(granularity_seconds("1hour"), 3_600);
        assert_eq!(granularity_seconds("1week"), 604_800);
        assert_eq!(granularity_seconds("1day"), 86_400);
    }

    #[test]
    fn action_amount_rules() {
        assert!(action_needs_amount("deposit"));
        assert!(action_needs_amount("request-redeem"));
        assert!(!action_needs_amount("claim-rewards"));
        assert!(!action_needs_amount("redeem"));
        assert!(validate_choice("Deposit", ACTIONS, "action").is_ok());
        assert!(validate_choice("withdraw", ACTIONS, "action").is_err());
    }
    #[test]
    fn missing_key_is_a_clear_error_not_a_request() {
        // No secret in ctx: the tool must fail before touching the network
        // with the onboarding hint (the SDK never reads process env).
        let ctx = TestCtxBuilder::new(GetBenchmark::NAME).build();
        let err =
            aomi_sdk::testing::run_tool::<GetBenchmark>(&VaultsFyiApp, json!({}), ctx).unwrap_err();
        assert!(err.contains("VAULTS_FYI_API_KEY"), "{err}");
        assert!(err.contains("portal.vaults.fyi"), "{err}");
    }

    /// Live probe against api.vaults.fyi: an invalid key must surface the
    /// mapped 401 message (proves host, header, and error mapping end to end).
    #[test]
    #[ignore = "network: hits api.vaults.fyi"]
    fn live_invalid_key_maps_to_401() {
        let ctx = TestCtxBuilder::new("vaultsfyi_get_benchmark")
            .secret(API_KEY_NAME, "aomi-invalid-key-probe")
            .build();
        let err = aomi_sdk::testing::run_tool::<GetBenchmark>(&VaultsFyiApp, json!({}), ctx)
            .expect_err("invalid key must fail");
        assert!(err.contains("401"), "unexpected error: {err}");
    }

    /// Live probe: the health endpoint needs no key and must decode.
    #[test]
    #[ignore = "network: hits api.vaults.fyi"]
    fn live_health_endpoint_decodes() {
        let ctx = TestCtxBuilder::new("t")
            .secret(API_KEY_NAME, "unused")
            .build();
        let client = Client::from_ctx(&ctx, None).unwrap();
        let health = client.get("/v2/health", &Query::new()).unwrap();
        assert_eq!(health["status"], "Success");
    }
    /// Full read + build ladder against the live API. The key is read from
    /// the `VAULTS_FYI_API_KEY` env var *by the test* and injected into
    /// `ctx.secrets`, mirroring what the host vault does; the app itself
    /// never reads the environment. Consumes a few credits. Run with
    /// `cargo test -- --ignored live_e2e --nocapture`.
    #[test]
    #[ignore = "network: hits api.vaults.fyi and needs VAULTS_FYI_API_KEY"]
    fn live_e2e_discovery_portfolio_and_deposit_build() {
        use aomi_sdk::testing::run_tool;
        let wallet = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
        let key = std::env::var(API_KEY_NAME)
            .expect("VAULTS_FYI_API_KEY must be set for the live ladder");
        let ctx = || {
            TestCtxBuilder::new("live")
                .secret(API_KEY_NAME, key.clone())
                .attribute("domain", json!({ "evm": { "address": wallet } }))
                .build()
        };
        let app = VaultsFyiApp;

        let found = run_tool::<FindVaults>(
            &app,
            json!({ "assets": ["USDC"], "networks": ["base"], "only_transactional": true, "limit": 3, "min_tvl_usd": 10_000_000 }),
            ctx(),
        )
        .unwrap()
        .into_value();
        println!(
            "find_vaults => {}",
            serde_json::to_string_pretty(&found).unwrap()
        );
        let first = found["vaults"][0].clone();
        assert_eq!(first["is_transactional"], true);
        assert_eq!(first["chain_id"], 8453);
        let address = first["address"].as_str().unwrap().to_string();

        let detail =
            run_tool::<GetVault>(&app, json!({ "network": "base", "vault": address }), ctx())
                .unwrap()
                .into_value();
        println!(
            "get_vault => {}",
            serde_json::to_string_pretty(&detail["vault"]).unwrap()
        );
        assert!(detail["vault"]["apy_pct"]["7day"]["total"].is_number());

        let hist = run_tool::<GetVaultHistory>(
            &app,
            json!({ "network": "base", "vault": address, "limit": 7 }),
            ctx(),
        )
        .unwrap()
        .into_value();
        println!("history summary => {}", hist["summary"]);
        assert!(
            hist["points"]
                .as_array()
                .map(|a| !a.is_empty())
                .unwrap_or(false)
        );

        let bench = run_tool::<GetBenchmark>(&app, json!({ "network": "base" }), ctx())
            .unwrap()
            .into_value();
        println!("benchmark => {}", bench["benchmark_apy_pct"]);

        let positions = run_tool::<GetPositions>(&app, json!({}), ctx())
            .unwrap()
            .into_value();
        println!(
            "positions => count={} total_usd={}",
            positions["position_count"], positions["total_usd"]
        );

        let options = run_tool::<GetDepositOptions>(
            &app,
            json!({ "networks": ["base", "mainnet"], "max_vaults_per_asset": 2 }),
            ctx(),
        )
        .unwrap()
        .into_value();
        println!(
            "deposit_options => idle_asset_count={}",
            options["idle_asset_count"]
        );
        for b in options["balances"].as_array().unwrap().iter().take(3) {
            println!(
                "  {} {} on {} (${}) -> {} options",
                b["balance"],
                b["asset"]["symbol"],
                b["asset"]["network"],
                b["balance_usd"],
                b["deposit_options"]
                    .as_array()
                    .map(|o| o.len())
                    .unwrap_or(0)
            );
        }

        let context = run_tool::<GetActionContext>(
            &app,
            json!({ "network": "base", "vault": address }),
            ctx(),
        )
        .unwrap()
        .into_value();
        println!(
            "action_context => available={} asset={} balance={}",
            context["available_actions"],
            context["asset"]["symbol"],
            context["asset"]["wallet_balance"]
        );
        assert!(
            context["available_actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "deposit")
        );

        let built = run_tool::<BuildVaultTx>(
            &app,
            json!({ "network": "base", "vault": address, "action": "deposit", "amount": "1" }),
            ctx(),
        )
        .unwrap();
        println!(
            "build_vault_tx => preview {}",
            serde_json::to_string_pretty(&built.value).unwrap()
        );
        assert_eq!(built.value["amount_base_units"], "1000000");
        assert!(!built.routes.is_empty());
        for r in &built.routes {
            assert_eq!(r.tool, "stage_tx");
            let data = r.args["data"]["raw"].as_str().unwrap();
            assert!(data.starts_with("0x") && data.len() > 10);
            println!(
                "  route stage_tx kind={} to={} calldata_len={}",
                r.args["kind"],
                r.args["to"],
                data.len()
            );
        }
        assert!(built.routes.last().unwrap().enforcement.is_some());
    }
}
