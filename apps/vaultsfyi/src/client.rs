//! HTTP client, network table, and response normalisation for vaults.fyi.
//!
//! Everything vaults.fyi-specific that is not a tool entrypoint lives here:
//! auth header, error mapping, the network alias table, amount conversion,
//! and the compact JSON shapes the tools hand back to the model.

use aomi_sdk::*;
use reqwest::header::HeaderValue;
use serde_json::{Map, Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct VaultsFyiApp;

pub(crate) const TAG: &str = "[vaultsfyi]";
pub(crate) const API_BASE: &str = "https://api.vaults.fyi";
/// Optional non-secret override for the API host (proxy, mock, staging).
pub(crate) const API_BASE_ENV: &str = "VAULTS_FYI_API_BASE";
/// Secret slot name declared in `lib.rs` and read at call time. The host
/// fills it from the per-app vault; the process environment is never read.
pub(crate) const API_KEY_NAME: &str = "VAULTS_FYI_API_KEY";
const API_KEY_HEADER: &str = "x-api-key";

// ============================================================================
// Networks
// ============================================================================

pub(crate) struct Network {
    /// Canonical vaults.fyi network name used in paths and filters.
    pub name: &'static str,
    pub chain_id: u64,
    pub aliases: &'static [&'static str],
}

/// V2 EVM networks from the vaults.fyi API overview. `GET /v2/networks` is
/// the live source of truth; this table only exists so users can say
/// "ethereum" or "8453" instead of the canonical name.
pub(crate) const NETWORKS: &[Network] = &[
    Network {
        name: "mainnet",
        chain_id: 1,
        aliases: &["ethereum", "eth", "ethereum-mainnet"],
    },
    Network {
        name: "optimism",
        chain_id: 10,
        aliases: &["op", "op-mainnet"],
    },
    Network {
        name: "bsc",
        chain_id: 56,
        aliases: &["bnb", "binance", "bnb-chain"],
    },
    Network {
        name: "gnosis",
        chain_id: 100,
        aliases: &["xdai", "gnosis-chain"],
    },
    Network {
        name: "unichain",
        chain_id: 130,
        aliases: &[],
    },
    Network {
        name: "polygon",
        chain_id: 137,
        aliases: &["matic", "polygon-pos"],
    },
    Network {
        name: "monad",
        chain_id: 143,
        aliases: &[],
    },
    Network {
        name: "hyperliquid",
        chain_id: 999,
        aliases: &["hyperevm"],
    },
    Network {
        name: "swellchain",
        chain_id: 1923,
        aliases: &["swell"],
    },
    Network {
        name: "mega-eth",
        chain_id: 4326,
        aliases: &["megaeth"],
    },
    Network {
        name: "robinhood",
        chain_id: 4663,
        aliases: &["robinhood-chain"],
    },
    Network {
        name: "base",
        chain_id: 8453,
        aliases: &[],
    },
    Network {
        name: "plasma",
        chain_id: 9745,
        aliases: &[],
    },
    Network {
        name: "arbitrum",
        chain_id: 42161,
        aliases: &["arb", "arbitrum-one"],
    },
    Network {
        name: "celo",
        chain_id: 42220,
        aliases: &[],
    },
    Network {
        name: "etherlink",
        chain_id: 42793,
        aliases: &[],
    },
    Network {
        name: "avalanche",
        chain_id: 43114,
        aliases: &["avax", "avalanche-c"],
    },
    Network {
        name: "ink",
        chain_id: 57073,
        aliases: &[],
    },
    Network {
        name: "linea",
        chain_id: 59144,
        aliases: &[],
    },
    Network {
        name: "berachain",
        chain_id: 80094,
        aliases: &["bera"],
    },
    Network {
        name: "worldchain",
        chain_id: 480,
        aliases: &["world", "world-chain"],
    },
    Network {
        name: "katana",
        chain_id: 747474,
        aliases: &[],
    },
];

/// Accept a canonical name, a common alias, a numeric chain id, or a CAIP-2
/// `eip155:<id>` string and return the vaults.fyi network entry.
pub(crate) fn resolve_network(input: &str) -> Result<&'static Network, String> {
    let raw = input.trim();
    let lower = raw.to_ascii_lowercase();
    let key = lower.strip_prefix("eip155:").unwrap_or(&lower);
    NETWORKS
        .iter()
        .find(|n| n.name == key || n.aliases.contains(&key) || n.chain_id.to_string() == key)
        .ok_or_else(|| {
            format!(
                "{TAG} unsupported network `{raw}`; use a vaults.fyi network name such as \
                 base, mainnet, arbitrum, optimism, polygon (or a numeric chain id)"
            )
        })
}

pub(crate) fn resolve_networks(inputs: Option<&[String]>) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in inputs.unwrap_or_default() {
        let name = resolve_network(raw)?.name.to_string();
        if !out.contains(&name) {
            out.push(name);
        }
    }
    Ok(out)
}

// ============================================================================
// HTTP client
// ============================================================================

pub(crate) struct Client {
    http: reqwest::blocking::Client,
    base: String,
    api_key: HeaderValue,
}

impl Client {
    /// Build a client for this call. The key comes from an explicit `api_key`
    /// arg or the host secret vault (`ctx.secrets`, populated from the
    /// `VAULTS_FYI_API_KEY` slot); the process environment is never read.
    pub(crate) fn from_ctx(
        ctx: &DynToolCallCtx,
        api_key_arg: Option<&str>,
    ) -> Result<Self, String> {
        let api_key = resolve_secret_value(
            ctx,
            api_key_arg,
            API_KEY_NAME,
            "[vaultsfyi] missing VAULTS_FYI_API_KEY. Create a key at https://portal.vaults.fyi \
             and add it to the app secrets.",
        )?;
        let mut api_key = HeaderValue::from_str(&api_key)
            .map_err(|e| format!("{TAG} invalid VAULTS_FYI_API_KEY value: {e}"))?;
        api_key.set_sensitive(true);
        let base = std::env::var(API_BASE_ENV)
            .ok()
            .map(|v| v.trim().trim_end_matches('/').to_string())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| API_BASE.to_string());
        Ok(Self {
            http: shared_http()?.clone(),
            base,
            api_key,
        })
    }

    pub(crate) fn get(&self, path: &str, query: &Query) -> Result<Value, String> {
        let url = format!("{}{}", self.base, path);
        // GETs are idempotent. api.vaults.fyi occasionally drops a fresh TLS
        // handshake ("tls handshake eof"), so retry transport-level failures
        // with a short backoff before surfacing the error.
        let mut last_err = None;
        for attempt in 0..MAX_ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(Duration::from_millis(RETRY_BACKOFF_MS * attempt as u64));
            }
            let sent = self
                .http
                .get(&url)
                .header(API_KEY_HEADER, self.api_key.clone())
                .query(&query.0)
                .send();
            let response = match sent {
                Ok(response) => response,
                Err(e) => {
                    last_err = Some(describe_transport_error(&e));
                    continue;
                }
            };
            let status = response.status().as_u16();
            let body = response.text().unwrap_or_default();
            if (200..300).contains(&status) {
                return serde_json::from_str(&body)
                    .map_err(|e| format!("{TAG} could not decode response from {path}: {e}"));
            }
            return Err(map_error(status, &body, path));
        }
        Err(format!(
            "{TAG} request to {path} failed after {MAX_ATTEMPTS} attempts: {}",
            last_err.unwrap_or_default()
        ))
    }
}

const MAX_ATTEMPTS: usize = 3;
const RETRY_BACKOFF_MS: u64 = 400;

/// One process-wide HTTP client so tool calls share a connection pool
/// instead of paying (and occasionally failing) a TLS handshake per call.
fn shared_http() -> Result<&'static reqwest::blocking::Client, String> {
    static HTTP: std::sync::OnceLock<Result<reqwest::blocking::Client, String>> =
        std::sync::OnceLock::new();
    HTTP.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(90))
            .user_agent(concat!("aomi-vaultsfyi/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| format!("{TAG} failed to build HTTP client: {e}"))
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// reqwest hides the root cause behind "error sending request"; walk the
/// source chain so the user sees the actual reason.
fn describe_transport_error(err: &reqwest::Error) -> String {
    use std::error::Error as _;
    let mut parts = vec![err.to_string()];
    let mut source = err.source();
    while let Some(inner) = source {
        parts.push(inner.to_string());
        source = inner.source();
    }
    parts.dedup();
    parts.join(": ")
}

/// Convert an upstream error into one short, actionable line.
pub(crate) fn map_error(status: u16, body: &str, path: &str) -> String {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| truncate(body.trim(), 240));
    match status {
        400 => format!("{TAG} bad request for {path}: {message}"),
        401 => format!("{TAG} API key rejected (401); check the {API_KEY_NAME} secret"),
        402 => format!(
            "{TAG} payment required (402): the request reached vaults.fyi without a valid API key \
             or the account has no credits left; check {API_KEY_NAME} or top up at \
             https://portal.vaults.fyi"
        ),
        403 => format!(
            "{TAG} API key has exhausted its credits (403); top up at https://portal.vaults.fyi"
        ),
        404 => format!(
            "{TAG} not found for {path}: {message} (check the network and vault address / id)"
        ),
        429 => format!("{TAG} rate limited (429); wait a moment and retry"),
        s if s >= 500 => format!("{TAG} vaults.fyi upstream error {s} for {path}: {message}"),
        s => format!("{TAG} HTTP {s} for {path}: {message}"),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// Ordered query string. Array parameters are sent as repeated keys
/// (`allowedNetworks=base&allowedNetworks=mainnet`), the OpenAPI 3 default
/// (`style: form, explode: true`) that the vaults.fyi spec relies on.
#[derive(Default, Debug)]
pub(crate) struct Query(pub(crate) Vec<(String, String)>);

impl Query {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn push(&mut self, key: &str, value: impl ToString) -> &mut Self {
        self.0.push((key.to_string(), value.to_string()));
        self
    }

    pub(crate) fn push_opt(&mut self, key: &str, value: Option<impl ToString>) -> &mut Self {
        if let Some(v) = value {
            self.push(key, v);
        }
        self
    }

    pub(crate) fn push_list(&mut self, key: &str, values: &[String]) -> &mut Self {
        for v in values {
            let v = v.trim();
            if !v.is_empty() {
                self.push(key, v);
            }
        }
        self
    }
}

// ============================================================================
// Value helpers
// ============================================================================

/// Read a number that vaults.fyi may encode as a JSON number or a decimal string.
pub(crate) fn num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Raw APY decimal (0.0543) -> percent rounded to 4 places (5.43).
pub(crate) fn pct(v: Option<f64>) -> Option<f64> {
    v.map(|x| (x * 100.0 * 10_000.0).round() / 10_000.0)
}

fn apy_pct_block(apy: Option<&Value>) -> Value {
    json!({
        "total": pct(num(apy.and_then(|a| a.get("total")))),
        "base": pct(num(apy.and_then(|a| a.get("base")))),
        "reward": pct(num(apy.and_then(|a| a.get("reward")))),
    })
}

pub(crate) fn is_evm_address(s: &str) -> bool {
    let s = s.trim();
    s.len() == 42 && s.starts_with("0x") && s[2..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Convert a human amount such as `100.5` into base units for an asset with
/// `decimals` decimals, using exact string arithmetic (no floats).
pub(crate) fn to_base_units(amount: &str, decimals: u32) -> Result<String, String> {
    let raw = amount.trim().replace(['_', ','], "");
    if raw.is_empty() {
        return Err(format!("{TAG} amount is empty"));
    }
    if raw.starts_with('-') || raw.starts_with('+') {
        return Err(format!(
            "{TAG} amount must be a positive decimal number, got `{amount}`"
        ));
    }
    let (int_part, frac_part) = match raw.split_once('.') {
        Some((i, f)) => (i, f),
        None => (raw.as_str(), ""),
    };
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "{TAG} amount must be a plain decimal number like `100` or `0.25`, got `{amount}`"
        ));
    }
    let decimals = decimals as usize;
    if frac_part.len() > decimals {
        return Err(format!(
            "{TAG} amount `{amount}` has more decimal places than the asset supports ({decimals})"
        ));
    }
    let mut digits = String::with_capacity(int_part.len() + decimals);
    digits.push_str(int_part);
    digits.push_str(frac_part);
    for _ in frac_part.len()..decimals {
        digits.push('0');
    }
    let trimmed = digits.trim_start_matches('0');
    if trimmed.is_empty() {
        return Err(format!("{TAG} amount must be greater than zero"));
    }
    Ok(trimmed.to_string())
}

/// Base units -> human decimal string (for previews only).
pub(crate) fn from_base_units(raw: &str, decimals: u32) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || !raw.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let decimals = decimals as usize;
    let padded = if raw.len() <= decimals {
        format!("{}{}", "0".repeat(decimals + 1 - raw.len()), raw)
    } else {
        raw.to_string()
    };
    let (int_part, frac_part) = padded.split_at(padded.len() - decimals);
    let frac_trimmed = frac_part.trim_end_matches('0');
    Some(if frac_trimmed.is_empty() {
        int_part.to_string()
    } else {
        format!("{int_part}.{frac_trimmed}")
    })
}

fn arr(v: Option<&Value>) -> Vec<Value> {
    v.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn non_empty_errors(v: &Value) -> Option<Value> {
    let errors = v.get("errors")?.as_object()?;
    let filtered: Map<String, Value> = errors
        .iter()
        .filter(|(_, v)| v.as_array().map(|a| !a.is_empty()).unwrap_or(false))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    (!filtered.is_empty()).then_some(Value::Object(filtered))
}

/// Wrap a payload with the `source` marker and any non-empty upstream
/// `errors` block (unsupported networks / assets / protocols).
pub(crate) fn ok_with_errors(mut payload: Value, upstream: &Value) -> Value {
    if let (Value::Object(map), Some(errors)) = (&mut payload, non_empty_errors(upstream)) {
        map.insert("unsupported_filters".into(), errors);
    }
    ok(payload)
}

pub(crate) fn ok(payload: Value) -> Value {
    match payload {
        Value::Object(mut map) => {
            map.insert("source".into(), Value::String("vaults.fyi".into()));
            Value::Object(map)
        }
        other => json!({ "source": "vaults.fyi", "data": other }),
    }
}

// ============================================================================
// Normalisers
// ============================================================================

fn protocol_block(v: &Value) -> Value {
    json!({
        "id": v.pointer("/protocol/protocolId"),
        "name": v.pointer("/protocol/displayName").or_else(|| v.pointer("/protocol/name")),
        "product": v.pointer("/protocol/product"),
        "version": v.pointer("/protocol/version"),
    })
}

fn asset_block(asset: Option<&Value>) -> Value {
    json!({
        "symbol": asset.and_then(|a| a.get("symbol")),
        "name": asset.and_then(|a| a.get("name")),
        "address": asset.and_then(|a| a.get("address")),
        "decimals": asset.and_then(|a| a.get("decimals")),
        "price_usd": num(asset.and_then(|a| a.get("assetPriceInUsd"))),
    })
}

/// Compact list-row shape for `/v2/detailed-vaults` items.
pub(crate) fn summarize_vault(v: &Value) -> Value {
    let warnings = arr(v.get("warnings"));
    json!({
        "vault_id": v.get("vaultId"),
        "name": v.get("name"),
        "address": v.get("address"),
        "network": v.pointer("/network/name"),
        "chain_id": v.pointer("/network/chainId"),
        "protocol": protocol_block(v),
        "asset": asset_block(v.get("asset")),
        "apy_pct": {
            "1day": apy_pct_block(v.pointer("/apy/1day")),
            "7day": apy_pct_block(v.pointer("/apy/7day")),
            "30day": apy_pct_block(v.pointer("/apy/30day")),
        },
        "tvl_usd": num(v.pointer("/tvl/usd")),
        "reputation_score": v.pointer("/score/vaultScore"),
        "is_transactional": v.get("isTransactional"),
        "deposit_steps_type": v.pointer("/transactionalProperties/depositStepsType"),
        "redeem_steps_type": v.pointer("/transactionalProperties/redeemStepsType"),
        "curator": v.pointer("/curator/name"),
        "tags": v.get("tags"),
        "warning_count": warnings.len(),
        "flag_severities": arr(v.get("flags"))
            .iter()
            .filter_map(|f| f.get("severity").and_then(Value::as_str).map(str::to_string))
            .collect::<Vec<_>>(),
        "url": v.get("lendUrl"),
    })
}

/// Rich single-vault shape for `/v2/detailed-vaults/{network}/{vaultId}`.
pub(crate) fn vault_detail(v: &Value) -> Value {
    let mut out = summarize_vault(v);
    if let Value::Object(map) = &mut out {
        map.insert(
            "description".into(),
            v.get("description").cloned().unwrap_or(Value::Null),
        );
        map.insert(
            "fees_pct".into(),
            json!({
                "performance": pct(num(v.pointer("/fees/performanceFee"))),
                "management": pct(num(v.pointer("/fees/managementFee"))),
                "deposit": pct(num(v.pointer("/fees/depositFee"))),
                "withdrawal": pct(num(v.pointer("/fees/withdrawalFee"))),
                "note": "APY figures are already net of fees; do not subtract again",
            }),
        );
        map.insert(
            "score_breakdown".into(),
            v.get("score").cloned().unwrap_or(Value::Null),
        );
        map.insert(
            "holders".into(),
            json!({
                "count": v.pointer("/holdersData/totalCount"),
                "top": arr(v.pointer("/holdersData/topHolders")).into_iter().take(5).collect::<Vec<_>>(),
            }),
        );
        map.insert(
            "rewards".into(),
            Value::Array(
                arr(v.get("rewards"))
                    .iter()
                    .map(|r| {
                        json!({
                            "asset": r.pointer("/asset/symbol"),
                            "apy_pct": apy_pct_block(r.get("apy")),
                        })
                    })
                    .collect(),
            ),
        );
        map.insert(
            "capacity".into(),
            json!({
                "remaining": v.get("remainingCapacity"),
                "max": v.get("maxCapacity"),
            }),
        );
        map.insert("lp_token".into(), asset_block(v.get("lpToken")));
        map.insert(
            "additional_assets".into(),
            Value::Array(
                arr(v.get("additionalAssets"))
                    .iter()
                    .map(|a| asset_block(Some(a)))
                    .collect(),
            ),
        );
        map.insert(
            "warnings".into(),
            v.get("warnings").cloned().unwrap_or(Value::Null),
        );
        map.insert(
            "flags".into(),
            v.get("flags").cloned().unwrap_or(Value::Null),
        );
        map.insert(
            "rewards_supported".into(),
            v.pointer("/transactionalProperties/rewardsSupported")
                .cloned()
                .unwrap_or(Value::Null),
        );
        map.insert(
            "protocol_vault_url".into(),
            v.get("protocolVaultUrl").cloned().unwrap_or(Value::Null),
        );
        map.insert(
            "last_update_timestamp".into(),
            v.get("lastUpdateTimestamp").cloned().unwrap_or(Value::Null),
        );
        map.insert(
            "deployed_timestamp".into(),
            v.pointer("/creationData/deploymentTimestamp")
                .cloned()
                .unwrap_or(Value::Null),
        );
        map.remove("warning_count");
    }
    out
}

/// Row shape for `/v2/portfolio/positions/{userAddress}` items.
pub(crate) fn summarize_position(p: &Value) -> Value {
    json!({
        "vault_id": p.get("vaultId"),
        "name": p.get("name"),
        "address": p.get("address"),
        "network": p.pointer("/network/name"),
        "chain_id": p.pointer("/network/chainId"),
        "protocol": protocol_block(p),
        "asset": {
            "symbol": p.pointer("/asset/symbol"),
            "address": p.pointer("/asset/address"),
            "decimals": p.pointer("/asset/decimals"),
        },
        "position_value_in_asset": p.pointer("/asset/positionValueInAsset"),
        "balance_usd": num(p.pointer("/asset/balanceUsd")),
        "unclaimed_usd": num(p.pointer("/asset/unclaimedUsd")),
        "lp_token": {
            "symbol": p.pointer("/lpToken/symbol"),
            "address": p.pointer("/lpToken/address"),
            "balance_native": p.pointer("/lpToken/balanceNative"),
            "balance_usd": num(p.pointer("/lpToken/balanceUsd")),
        },
        "apy_pct": apy_pct_block(p.get("apy")),
        "is_transactional": p.get("isTransactional"),
    })
}

/// Row shape for `/v2/portfolio/best-deposit-options/{userAddress}` options.
pub(crate) fn summarize_deposit_option(o: &Value) -> Value {
    json!({
        "vault_id": o.get("vaultId"),
        "name": o.get("name"),
        "address": o.get("address"),
        "network": o.pointer("/network/name"),
        "chain_id": o.pointer("/network/chainId"),
        "protocol": protocol_block(o),
        "apy_pct": apy_pct_block(o.get("apy")),
        "tvl_usd": num(o.pointer("/tvl/usd")),
        "projected_usd_annual_earnings": num(o.get("projectedUsdAnnualEarnings")),
        "is_transactional": o.get("isTransactional"),
        "tags": o.get("tags"),
        "url": o.get("lendUrl"),
    })
}

fn steps_block(steps: Option<&Value>) -> Vec<Value> {
    arr(steps)
        .iter()
        .map(|s| json!({ "name": s.get("name"), "actions": s.get("actions") }))
        .collect()
}

fn balance_block(asset: Option<&Value>) -> Value {
    let decimals = asset
        .and_then(|a| a.get("decimals"))
        .and_then(Value::as_u64)
        .map(|d| d as u32);
    let balance_native = asset
        .and_then(|a| a.get("balanceNative"))
        .and_then(Value::as_str);
    let balance_human = match (balance_native, decimals) {
        (Some(raw), Some(d)) => from_base_units(raw, d),
        _ => None,
    };
    json!({
        "symbol": asset.and_then(|a| a.get("symbol")),
        "address": asset.and_then(|a| a.get("address")),
        "decimals": asset.and_then(|a| a.get("decimals")),
        "wallet_balance_native": balance_native,
        "wallet_balance": balance_human,
        "wallet_balance_usd": num(asset.and_then(|a| a.get("balanceUsd"))),
        "position_value_in_asset": asset.and_then(|a| a.get("positionValueInAsset")),
        "unclaimed_usd": num(asset.and_then(|a| a.get("unclaimedUsd"))),
        "deposit_limit": asset.and_then(|a| a.get("depositLimit")),
    })
}

/// Model-facing shape for `/v2/transactions/context/...`.
pub(crate) fn summarize_context(c: &Value) -> Value {
    json!({
        "current_deposit_step": c.get("currentDepositStep"),
        "deposit_steps": steps_block(c.get("depositSteps")),
        "current_redeem_step": c.get("currentRedeemStep"),
        "redeem_steps": steps_block(c.get("redeemSteps")),
        "available_actions": available_actions(c),
        "asset": balance_block(c.get("asset")),
        "additional_assets": arr(c.get("additionalAssets")).iter().map(|a| balance_block(Some(a))).collect::<Vec<_>>(),
        "lp_token": {
            "symbol": c.pointer("/lpToken/symbol"),
            "address": c.pointer("/lpToken/address"),
            "decimals": c.pointer("/lpToken/decimals"),
            "balance_native": c.pointer("/lpToken/balanceNative"),
            "balance_usd": num(c.pointer("/lpToken/balanceUsd")),
        },
        "pending_requests": c.get("pendingRequests"),
        "vault_specific_data": c.get("vaultSpecificData"),
        "rewards": {
            "claimable": arr(c.pointer("/rewards/claimable")).iter().map(|r| json!({
                "amount": r.get("amount"),
                "asset": r.pointer("/asset/symbol"),
                "asset_address": r.pointer("/asset/address"),
            })).collect::<Vec<_>>(),
            "current_step": c.pointer("/rewards/currentStep"),
            "steps": steps_block(c.pointer("/rewards/steps")),
        },
    })
}

/// Union of step names the vault currently exposes (deposit, redeem, rewards).
pub(crate) fn available_actions(c: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for steps in [
        c.get("depositSteps"),
        c.get("redeemSteps"),
        c.pointer("/rewards/steps"),
    ] {
        for s in arr(steps) {
            let Some(name) = s.get("name").and_then(Value::as_str) else {
                continue;
            };
            if !out.iter().any(|n| n == name) {
                out.push(name.to_string());
            }
        }
    }
    out
}

/// Find `(decimals, symbol)` for `asset_address` among the context's asset,
/// additional assets, and children positions.
pub(crate) fn asset_in_context(c: &Value, asset_address: &str) -> Option<(u32, String)> {
    let wanted = asset_address.trim().to_ascii_lowercase();
    let mut candidates: Vec<&Value> = Vec::new();
    if let Some(a) = c.get("asset") {
        candidates.push(a);
    }
    candidates.extend(
        c.get("additionalAssets")
            .and_then(Value::as_array)
            .into_iter()
            .flatten(),
    );
    for child in c
        .get("childrenPositions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(a) = child.get("asset") {
            candidates.push(a);
        }
        candidates.extend(
            child
                .get("additionalAssets")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        );
    }
    candidates.into_iter().find_map(|a| {
        let addr = a.get("address")?.as_str()?;
        if addr.to_ascii_lowercase() != wanted {
            return None;
        }
        let decimals = a.get("decimals")?.as_u64()? as u32;
        let symbol = a
            .get("symbol")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        Some((decimals, symbol))
    })
}

// ============================================================================
// Transaction payload -> staged steps
// ============================================================================

/// One ready-to-sign transaction from `/v2/transactions/{action}/...`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StagedStep {
    pub name: String,
    pub to: String,
    pub data: String,
    pub value: String,
    pub kind: String,
    pub simulation: Option<Value>,
}

/// Pull the remaining actions (from `currentActionIndex` onward) out of a
/// vaults.fyi payload and validate each one against the expected chain.
/// Calldata is copied verbatim — nothing is re-encoded.
pub(crate) fn extract_steps(
    payload: &Value,
    expected_chain_id: u64,
    action: &str,
) -> Result<Vec<StagedStep>, String> {
    let actions = payload
        .get("actions")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{TAG} transaction payload for `{action}` is missing `actions`"))?;
    let start = payload
        .get("currentActionIndex")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let mut steps = Vec::new();
    for (index, item) in actions.iter().enumerate().skip(start) {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or(action)
            .to_string();
        let tx = item
            .get("tx")
            .ok_or_else(|| format!("{TAG} action #{index} (`{name}`) has no `tx` payload"))?;
        let to = tx
            .get("to")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !is_evm_address(&to) {
            return Err(format!(
                "{TAG} action `{name}` has an invalid `to` address: `{to}`"
            ));
        }
        let chain_id = tx.get("chainId").and_then(Value::as_u64).unwrap_or(0);
        if chain_id != expected_chain_id {
            return Err(format!(
                "{TAG} action `{name}` targets chain {chain_id} but the vault network is chain {expected_chain_id}; refusing to stage"
            ));
        }
        let data = tx
            .get("data")
            .and_then(Value::as_str)
            .unwrap_or("0x")
            .to_string();
        if !data.starts_with("0x") || !data[2..].chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!("{TAG} action `{name}` has non-hex calldata"));
        }
        let value = tx
            .get("value")
            .map(|v| match v {
                Value::String(s) if !s.trim().is_empty() => s.trim().to_string(),
                Value::Number(n) => n.to_string(),
                _ => "0".to_string(),
            })
            .unwrap_or_else(|| "0".to_string());
        let lower = name.to_ascii_lowercase();
        let kind = if lower.contains("approv") {
            "erc20_approve".to_string()
        } else {
            action.to_string()
        };
        steps.push(StagedStep {
            name,
            to,
            data,
            value,
            kind,
            simulation: item.get("simulation").cloned(),
        });
    }
    if steps.is_empty() {
        return Err(format!(
            "{TAG} vaults.fyi returned no pending transactions for `{action}`; the vault may already \
             be past this step — call vaultsfyi_get_action_context to see the current state"
        ));
    }
    Ok(steps)
}

/// Turn extracted steps into a routed `ToolReturn`: one `stage_tx` per step
/// (calldata verbatim), with the host's simulate + commit enforcement bound
/// to the final step so the wallet is prompted exactly once.
pub(crate) fn stage_route(
    preview: Value,
    steps: &[StagedStep],
    summary: &str,
) -> Result<ToolReturn, String> {
    let last = steps.len().saturating_sub(1);
    ToolReturn::route(preview)
        .next(|next| {
            for (i, step) in steps.iter().enumerate() {
                let args = json!({
                    "to": step.to,
                    "description": format!("vaults.fyi {}: {summary}", step.name),
                    "data": { "raw": step.data },
                    "value": step.value,
                    "kind": step.kind,
                });
                let builder = next.add::<host::StageTx>(args);
                if i == last {
                    builder
                        .note(
                            "Stage this vaults.fyi transaction. CRITICAL: copy `to` and `data.raw` \
                             BYTE-FOR-BYTE from the args — never abbreviate, reformat, or re-encode the \
                             calldata. After this step the host simulates and commits every staged tx \
                             and waits for the wallet.",
                        )
                        .enforce(EnforcementPolicy::Continue, |enforce| {
                            enforce.add::<host::SimulateBatch>(json!({}));
                            enforce
                                .add::<host::CommitTxs>(json!({ "aa_preference": "auto" }))
                                .bind_as("transaction_hash");
                        });
                } else {
                    builder.note(
                        "Stage this prerequisite transaction (typically an ERC-20 approval) first. \
                         CRITICAL: copy `to` and `data.raw` byte-for-byte; do not modify.",
                    );
                }
            }
        })
        .try_build()
        .map_err(|e| format!("{TAG} route build: {e}"))
}
