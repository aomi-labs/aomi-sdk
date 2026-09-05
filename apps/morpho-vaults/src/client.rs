//! Morpho API client + normalization helpers.
//!
//! Two public, unauthenticated surfaces on `https://api.morpho.org`:
//!   * REST (`/v0/...`, `/v1/...`) — one-vault config, live state, histories,
//!     positions, performance, withdrawal options, pending governance.
//!   * GraphQL (`/graphql`) — indexed discovery, USD analytics, APY, rewards,
//!     warnings, roles, timelocks.
//!
//! REST selectors are `<chain_id>:<vault_address>`. Vault V1 (MetaMorpho) and
//! Vault V2 live under different route families, so every call is keyed by
//! [`VaultVersion`]. When the caller does not know the version we probe V2
//! first, then V1 (`resolve_vault`).

use serde_json::{Map, Value, json};
use std::time::Duration;

pub(crate) const DEFAULT_API_BASE: &str = "https://api.morpho.org";

/// Chains the Morpho API indexes (from the Core API OpenAPI `chain_ids` enum).
pub(crate) const SUPPORTED_CHAINS: &[(u64, &str)] = &[
    (1, "ethereum"),
    (8453, "base"),
    (42161, "arbitrum"),
    (10, "optimism"),
    (137, "polygon"),
    (130, "unichain"),
    (480, "worldchain"),
    (999, "hyperevm"),
    (747474, "katana"),
    (143, "monad"),
    (988, "stable"),
    (4217, "tempo"),
    (4663, "robinhood"),
    (5042, "chain-5042"),
];

pub(crate) fn chain_name(chain_id: u64) -> String {
    SUPPORTED_CHAINS
        .iter()
        .find(|(id, _)| *id == chain_id)
        .map(|(_, n)| (*n).to_string())
        .unwrap_or_else(|| format!("chain-{chain_id}"))
}

pub(crate) fn is_supported_chain(chain_id: u64) -> bool {
    SUPPORTED_CHAINS.iter().any(|(id, _)| *id == chain_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VaultVersion {
    V1,
    V2,
}

impl VaultVersion {
    pub(crate) fn label(self) -> &'static str {
        match self {
            VaultVersion::V1 => "v1",
            VaultVersion::V2 => "v2",
        }
    }

    pub(crate) fn parse(input: Option<&str>) -> Result<Option<Self>, String> {
        match input.map(|s| s.trim().to_ascii_lowercase()) {
            None => Ok(None),
            Some(s) if s.is_empty() || s == "auto" || s == "all" => Ok(None),
            Some(s) if s == "v1" || s == "1" || s == "metamorpho" => Ok(Some(VaultVersion::V1)),
            Some(s) if s == "v2" || s == "2" => Ok(Some(VaultVersion::V2)),
            Some(other) => Err(format!(
                "[morpho] unknown vault version `{other}`; use `v1`, `v2`, or omit for auto"
            )),
        }
    }

    fn family(self) -> &'static str {
        match self {
            VaultVersion::V1 => "vaults-v1",
            VaultVersion::V2 => "vaults-v2",
        }
    }
}

// ============================================================================
// HTTP client
// ============================================================================

#[derive(Clone)]
pub(crate) struct MorphoClient {
    http: reqwest::blocking::Client,
    base: String,
}

impl MorphoClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[morpho] failed to build HTTP client: {e}"))?;
        let base = std::env::var("MORPHO_API_BASE")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_API_BASE.to_string())
            .trim_end_matches('/')
            .to_string();
        Ok(Self { http, base })
    }

    /// GET a REST route. Returns the parsed JSON body. Morpho wraps most
    /// single-entity responses in `{ "data": ... }`; callers unwrap as needed.
    pub(crate) fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        let url = format!("{}{}", self.base, path);
        let response = with_transport_retry(|| self.http.get(&url).query(query).send())
            .map_err(|e| format!("[morpho] GET {path} failed: {e}"))?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(rest_error(path, status.as_u16(), &text));
        }
        serde_json::from_str(&text).map_err(|e| format!("[morpho] GET {path}: decode error: {e}"))
    }

    /// GET and unwrap the `data` envelope.
    pub(crate) fn get_data(&self, path: &str, query: &[(&str, String)]) -> Result<Value, String> {
        let body = self.get(path, query)?;
        Ok(body.get("data").cloned().unwrap_or(body))
    }

    pub(crate) fn graphql(&self, query: &str, variables: Value) -> Result<Value, String> {
        let url = format!("{}/graphql", self.base);
        let body = json!({ "query": query, "variables": variables });
        let response = with_transport_retry(|| self.http.post(&url).json(&body).send())
            .map_err(|e| format!("[morpho] GraphQL request failed: {e}"))?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(rest_error("/graphql", status.as_u16(), &text));
        }
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| format!("[morpho] GraphQL decode error: {e}"))?;
        if let Some(errors) = parsed.get("errors").and_then(Value::as_array)
            && !errors.is_empty()
        {
            let messages: Vec<String> = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(Value::as_str))
                .map(str::to_string)
                .collect();
            return Err(format!("[morpho] GraphQL errors: {}", messages.join("; ")));
        }
        Ok(parsed.get("data").cloned().unwrap_or(Value::Null))
    }

    // ------------------------------------------------------------------
    // Vault config / state (REST)
    // ------------------------------------------------------------------

    /// Vault configuration (roles, fees, timelock, asset). `/v0/<family>/<sel>`.
    pub(crate) fn vault_config(
        &self,
        version: VaultVersion,
        chain_id: u64,
        address: &str,
    ) -> Result<Value, String> {
        self.get_data(
            &format!("/v0/{}/{}", version.family(), selector(chain_id, address)),
            &[],
        )
    }

    /// Probe V2 then V1 unless the caller pinned a version. Returns the
    /// resolved version plus the config payload.
    pub(crate) fn resolve_vault(
        &self,
        version: Option<VaultVersion>,
        chain_id: u64,
        address: &str,
    ) -> Result<(VaultVersion, Value), String> {
        if let Some(v) = version {
            return self.vault_config(v, chain_id, address).map(|c| (v, c));
        }
        match self.vault_config(VaultVersion::V2, chain_id, address) {
            Ok(cfg) => Ok((VaultVersion::V2, cfg)),
            Err(v2_err) if is_not_found(&v2_err) => {
                match self.vault_config(VaultVersion::V1, chain_id, address) {
                    Ok(cfg) => Ok((VaultVersion::V1, cfg)),
                    Err(v1_err) if is_not_found(&v1_err) => Err(format!(
                        "[morpho] no Vault V1 or V2 found at {address} on chain {chain_id} ({}). \
                         Check the address and chain, or use morpho_find_vaults.",
                        chain_name(chain_id)
                    )),
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Live accounting state. V2 lives under `/v1/vaults-v2/.../state`,
    /// V1 under `/v0/vaults-v1/.../state`.
    pub(crate) fn vault_state(
        &self,
        version: VaultVersion,
        chain_id: u64,
        address: &str,
    ) -> Result<Value, String> {
        let prefix = match version {
            VaultVersion::V1 => "/v0",
            VaultVersion::V2 => "/v1",
        };
        self.get_data(
            &format!(
                "{prefix}/{}/{}/state",
                version.family(),
                selector(chain_id, address)
            ),
            &[],
        )
    }

    /// Realized average APY over a trailing window.
    pub(crate) fn vault_apy_average(
        &self,
        version: VaultVersion,
        chain_id: u64,
        address: &str,
        lookback: &str,
    ) -> Result<Option<f64>, String> {
        let prefix = match version {
            VaultVersion::V1 => "/v0",
            VaultVersion::V2 => "/v1",
        };
        let data = self.get_data(
            &format!(
                "{prefix}/{}/{}/apy-averages",
                version.family(),
                selector(chain_id, address)
            ),
            &[("lookback", lookback.to_string())],
        )?;
        Ok(data.get("apy").and_then(Value::as_f64))
    }

    pub(crate) fn vault_history(
        &self,
        version: VaultVersion,
        chain_id: u64,
        address: &str,
        kind: &str,
        lookback: &str,
    ) -> Result<Vec<Value>, String> {
        let body = self.get(
            &format!(
                "/v0/{}/{}/{kind}/history",
                version.family(),
                selector(chain_id, address)
            ),
            &[("lookback", lookback.to_string())],
        )?;
        Ok(body
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    pub(crate) fn vault_allocations(
        &self,
        version: VaultVersion,
        chain_id: u64,
        address: &str,
    ) -> Result<Value, String> {
        self.get_data(
            &format!(
                "/v0/{}/{}/allocations",
                version.family(),
                selector(chain_id, address)
            ),
            &[],
        )
    }

    /// Vault V2 only: liquid vs. force-deallocatable exit capacity.
    pub(crate) fn vault_withdrawal_options(
        &self,
        chain_id: u64,
        address: &str,
    ) -> Result<Value, String> {
        self.get_data(
            &format!(
                "/v0/vaults-v2/{}/withdrawal-options",
                selector(chain_id, address)
            ),
            &[],
        )
    }

    pub(crate) fn vault_pending_governance(
        &self,
        version: VaultVersion,
        chain_id: u64,
        address: &str,
    ) -> Result<Vec<Value>, String> {
        let body = self.get(
            &format!(
                "/v0/{}/{}/pending-governance-actions",
                version.family(),
                selector(chain_id, address)
            ),
            &[("limit", "50".to_string())],
        )?;
        Ok(body
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    // ------------------------------------------------------------------
    // User positions (REST)
    // ------------------------------------------------------------------

    pub(crate) fn user_position(
        &self,
        version: VaultVersion,
        chain_id: u64,
        address: &str,
        user: &str,
    ) -> Result<Value, String> {
        self.get_data(
            &format!(
                "/v0/{}/{}/users/{user}/position",
                version.family(),
                selector(chain_id, address)
            ),
            &[],
        )
    }

    pub(crate) fn user_position_performance(
        &self,
        version: VaultVersion,
        chain_id: u64,
        address: &str,
        user: &str,
    ) -> Result<Value, String> {
        self.get_data(
            &format!(
                "/v0/{}/{}/users/{user}/position/performance",
                version.family(),
                selector(chain_id, address)
            ),
            &[],
        )
    }

    /// All active positions for a user in one vault family, optionally
    /// restricted to a chain. Follows the cursor up to a small page cap.
    pub(crate) fn user_positions(
        &self,
        version: VaultVersion,
        user: &str,
        chain_id: Option<u64>,
    ) -> Result<Vec<Value>, String> {
        let path = format!("/v1/{}/users/{user}/positions", version.family());
        let mut out = Vec::new();
        let mut cursor: Option<String> = None;
        for _ in 0..5 {
            let mut query: Vec<(&str, String)> = vec![
                ("active_only", "true".to_string()),
                ("limit", "100".to_string()),
            ];
            if let Some(c) = chain_id {
                query.push(("chain_ids", c.to_string()));
            }
            if let Some(c) = cursor.take() {
                query.push(("cursor", c));
            }
            let body = self.get(&path, &query)?;
            if let Some(items) = body.get("data").and_then(Value::as_array) {
                out.extend(items.iter().cloned());
            }
            cursor = body
                .get("cursor")
                .and_then(Value::as_str)
                .filter(|c| !c.is_empty())
                .map(str::to_string);
            if cursor.is_none() {
                break;
            }
        }
        Ok(out)
    }

    // ------------------------------------------------------------------
    // GraphQL analytics
    // ------------------------------------------------------------------

    /// Indexed V1 vault list with USD + APY analytics.
    pub(crate) fn graphql_vaults_v1(
        &self,
        chain_id: u64,
        listed_only: bool,
        asset: Option<&AssetFilter>,
        min_tvl_usd: f64,
        order_by: &str,
        first: usize,
    ) -> Result<Vec<Value>, String> {
        let mut where_clause = json!({
            "chainId_in": [chain_id],
            "totalAssetsUsd_gte": min_tvl_usd,
        });
        if listed_only {
            where_clause["listed"] = json!(true);
        }
        match asset {
            Some(AssetFilter::Address(a)) => where_clause["assetAddress_in"] = json!([a]),
            Some(AssetFilter::Symbol(s)) => where_clause["assetSymbol_in"] = json!([s]),
            None => {}
        }
        let query = r#"
            query VaultsV1($first: Int!, $orderBy: VaultOrderBy!, $where: VaultFilters!) {
              vaults(first: $first, orderBy: $orderBy, orderDirection: Desc, where: $where) {
                items {
                  address name symbol listed
                  chain { id }
                  asset { address symbol decimals }
                  state {
                    apy netApy avgNetApy totalAssetsUsd fee timelock curator
                    allRewards { asset { symbol } supplyApr }
                  }
                  warnings { type level }
                }
              }
            }"#;
        let data = self.graphql(
            query,
            json!({ "first": first, "orderBy": order_by, "where": where_clause }),
        )?;
        Ok(data
            .pointer("/vaults/items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    /// Indexed V2 vault list. `VaultV2sFilters` has no symbol filter, so
    /// symbol matching happens client-side in the caller.
    pub(crate) fn graphql_vaults_v2(
        &self,
        chain_id: u64,
        listed_only: bool,
        asset_address: Option<&str>,
        min_tvl_usd: f64,
        order_by: &str,
        first: usize,
    ) -> Result<Vec<Value>, String> {
        let mut where_clause = json!({
            "chainId_in": [chain_id],
            "totalAssetsUsd_gte": min_tvl_usd,
        });
        if listed_only {
            where_clause["listed"] = json!(true);
        }
        if let Some(a) = asset_address {
            where_clause["assetAddress_in"] = json!([a]);
        }
        let query = r#"
            query VaultsV2($first: Int!, $orderBy: VaultV2OrderBy!, $where: VaultV2sFilters!) {
              vaultV2s(first: $first, orderBy: $orderBy, orderDirection: Desc, where: $where) {
                items {
                  address name symbol listed type
                  chain { id }
                  asset { address symbol decimals }
                  apy netApy netApyExcludingRewards avgNetApy
                  totalAssetsUsd liquidityUsd idleAssetsUsd
                  performanceFee managementFee
                  curator { address }
                  rewards { asset { symbol } supplyApr }
                  warnings { type level }
                }
              }
            }"#;
        let data = self.graphql(
            query,
            json!({ "first": first, "orderBy": order_by, "where": where_clause }),
        )?;
        Ok(data
            .pointer("/vaultV2s/items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    /// Indexed analytics for one vault: APY, USD, rewards, warnings, roles,
    /// pending governance, and (V1) per-market allocation with USD.
    pub(crate) fn graphql_vault_detail(
        &self,
        version: VaultVersion,
        chain_id: u64,
        address: &str,
    ) -> Result<Value, String> {
        let (query, root) = match version {
            VaultVersion::V1 => (
                r#"
                query VaultV1($address: String!, $chainId: Int!) {
                  vaultByAddress(address: $address, chainId: $chainId) {
                    name symbol listed featured
                    asset { address symbol decimals }
                    warnings { type level }
                    metadata { description }
                    state {
                      curators { name verified }
                      apy netApy avgNetApy netApyExcludingRewards
                      totalAssetsUsd sharePriceUsd fee timelock
                      curator guardian owner
                      allRewards { asset { symbol } supplyApr }
                      pendingConfigs { items { validAt functionName } }
                      allocation {
                        supplyAssets supplyAssetsUsd supplyCap supplyCapUsd
                        supplyQueueIndex withdrawQueueIndex
                        pendingSupplyCapUsd removableAt
                        market {
                          marketId lltv
                          loanAsset { symbol }
                          collateralAsset { symbol }
                          state { supplyApy utilization liquidityAssetsUsd }
                        }
                      }
                    }
                    liquidity { underlying usd }
                  }
                }"#,
                "vaultByAddress",
            ),
            VaultVersion::V2 => (
                r#"
                query VaultV2($address: String!, $chainId: Int!) {
                  vaultV2ByAddress(address: $address, chainId: $chainId) {
                    name symbol listed type
                    asset { address symbol decimals }
                    apy netApy netApyExcludingRewards avgNetApy maxApy
                    totalAssetsUsd liquidityUsd idleAssetsUsd forceDeallocatableLiquidityUsd
                    sharePrice performanceFee managementFee
                    curator { address }
                    curators(first: 5) { items { name verified } }
                    owner { address }
                    metadata { description }
                    warnings { type level }
                    rewards { asset { symbol } supplyApr }
                    allocators { allocator { address } }
                    sentinels { sentinel { address } }
                    timelocks { functionName duration abdicatedAt }
                    pendingConfigs(first: 50) { items { functionName validAt txHash } }
                    adapters(first: 50) {
                      items { address type assetsUsd forceDeallocatePenalty }
                    }
                    liquidityAdapter { address type }
                    caps(first: 100) {
                      items { id type absoluteCap relativeCap allocation }
                    }
                  }
                }"#,
                "vaultV2ByAddress",
            ),
        };
        let data = self.graphql(query, json!({ "address": address, "chainId": chain_id }))?;
        let detail = data.get(root).cloned().unwrap_or(Value::Null);
        if detail.is_null() {
            return Err(format!(
                "[morpho] GraphQL has no {} analytics for {address} on chain {chain_id}",
                version.label()
            ));
        }
        Ok(detail)
    }

    /// V1 positions for a user (GraphQL `vaultPositions`), all chains unless
    /// filtered. Includes USD value, indexed P&L and the vault's net APY.
    pub(crate) fn graphql_v1_positions(
        &self,
        user: &str,
        chain_id: Option<u64>,
    ) -> Result<Vec<Value>, String> {
        let mut where_clause = json!({ "userAddress_in": [user], "shares_gte": "1" });
        if let Some(c) = chain_id {
            where_clause["chainId_in"] = json!([c]);
        }
        let query = r#"
            query UserVaultsV1($where: VaultPositionFilters!) {
              vaultPositions(first: 100, where: $where) {
                items {
                  vault { address name symbol chain { id } asset { symbol decimals } state { netApy } }
                  state { assets assetsUsd shares pnlUsd roe }
                }
              }
            }"#;
        let data = self.graphql(query, json!({ "where": where_clause }))?;
        Ok(data
            .pointer("/vaultPositions/items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    /// One V2 position with USD / P&L plus vault metadata. The API has no
    /// cross-vault V2 position list, so callers pair this with the REST
    /// `user_positions` list.
    pub(crate) fn graphql_v2_position(
        &self,
        user: &str,
        chain_id: u64,
        vault: &str,
    ) -> Result<Value, String> {
        let query = r#"
            query UserVaultV2($user: String!, $vault: String!, $chainId: Int!) {
              vaultV2PositionByAddress(userAddress: $user, vaultAddress: $vault, chainId: $chainId) {
                assets assetsUsd shares pnlUsd roe
                vault { name symbol netApy asset { symbol decimals } }
              }
            }"#;
        let data = self.graphql(
            query,
            json!({ "user": user, "vault": vault, "chainId": chain_id }),
        )?;
        Ok(data
            .get("vaultV2PositionByAddress")
            .cloned()
            .unwrap_or(Value::Null))
    }
}

pub(crate) enum AssetFilter {
    Address(String),
    Symbol(String),
}

impl AssetFilter {
    pub(crate) fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return None;
        }
        if is_hex_address(trimmed) {
            Some(AssetFilter::Address(trimmed.to_string()))
        } else {
            Some(AssetFilter::Symbol(trimmed.to_uppercase()))
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// api.morpho.org occasionally drops a connection mid-handshake. Retry a
/// transport-level failure once after a short pause; HTTP error statuses are
/// not retried here.
fn with_transport_retry<F>(mut send: F) -> reqwest::Result<reqwest::blocking::Response>
where
    F: FnMut() -> reqwest::Result<reqwest::blocking::Response>,
{
    match send() {
        Ok(r) => Ok(r),
        Err(first) if first.is_connect() || first.is_timeout() || first.is_request() => {
            std::thread::sleep(Duration::from_millis(750));
            send().or(Err(first))
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn selector(chain_id: u64, address: &str) -> String {
    format!("{chain_id}:{address}")
}

fn rest_error(path: &str, status: u16, body: &str) -> String {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.get("message")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    v.get("errors").and_then(Value::as_array).map(|errs| {
                        errs.iter()
                            .filter_map(|e| e.get("message").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join("; ")
                    })
                })
        })
        .unwrap_or_else(|| body.chars().take(200).collect());
    match status {
        404 => format!("[morpho] not found: {path} ({message})"),
        429 => "[morpho] rate limited by api.morpho.org (750 req/min); retry shortly".to_string(),
        _ => format!("[morpho] HTTP {status} on {path}: {message}"),
    }
}

pub(crate) fn is_not_found(err: &str) -> bool {
    err.starts_with("[morpho] not found:")
}

pub(crate) fn is_hex_address(s: &str) -> bool {
    s.len() == 42 && s.starts_with("0x") && s[2..].chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn require_address(s: &str, what: &str) -> Result<String, String> {
    let trimmed = s.trim();
    if is_hex_address(trimmed) {
        Ok(trimmed.to_string())
    } else {
        Err(format!(
            "[morpho] {what} must be a 0x-prefixed 20-byte hex address, got `{trimmed}`"
        ))
    }
}

pub(crate) const LOOKBACKS: &[&str] = &[
    "one_hour",
    "six_hours",
    "one_day",
    "seven_days",
    "thirty_days",
    "ninety_days",
    "one_year",
    "inception",
];

pub(crate) fn normalize_lookback(input: Option<&str>) -> Result<String, String> {
    let raw = input.unwrap_or("seven_days").trim().to_ascii_lowercase();
    let mapped = match raw.as_str() {
        "1h" | "hour" => "one_hour",
        "6h" => "six_hours",
        "1d" | "24h" | "day" => "one_day",
        "7d" | "1w" | "week" => "seven_days",
        "30d" | "1m" | "month" => "thirty_days",
        "90d" | "3m" | "quarter" => "ninety_days",
        "1y" | "365d" | "year" => "one_year",
        "all" | "max" => "inception",
        other => other,
    };
    if LOOKBACKS.contains(&mapped) {
        Ok(mapped.to_string())
    } else {
        Err(format!(
            "[morpho] unknown lookback `{raw}`; use one of {}",
            LOOKBACKS.join(", ")
        ))
    }
}

/// Parse a human token amount (`"1250.5"`, `"1,000"`, `"0.000001"`) into an
/// integer base-unit string using pure integer math — no floats.
pub(crate) fn to_base_units(amount: &str, decimals: u32) -> Result<String, String> {
    let cleaned: String = amount
        .trim()
        .chars()
        .filter(|c| *c != ',' && *c != '_' && !c.is_whitespace())
        .collect();
    if cleaned.is_empty() {
        return Err("[morpho] amount is empty".to_string());
    }
    let (int_part, frac_part) = match cleaned.split_once('.') {
        Some((i, f)) => (i, f),
        None => (cleaned.as_str(), ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(format!("[morpho] amount `{amount}` is not a number"));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "[morpho] amount `{amount}` must be a positive decimal number (no sign, no exponent)"
        ));
    }
    if frac_part.len() > decimals as usize {
        return Err(format!(
            "[morpho] amount `{amount}` has more than {decimals} decimal places, which this asset cannot represent"
        ));
    }
    let mut digits = String::with_capacity(int_part.len() + decimals as usize);
    digits.push_str(int_part.trim_start_matches('0'));
    digits.push_str(frac_part);
    for _ in frac_part.len()..decimals as usize {
        digits.push('0');
    }
    let digits = digits.trim_start_matches('0');
    if digits.is_empty() {
        return Err("[morpho] amount must be greater than zero".to_string());
    }
    if digits.len() > 38 {
        return Err("[morpho] amount is too large".to_string());
    }
    Ok(digits.to_string())
}

/// Render an integer base-unit string as a human decimal string.
pub(crate) fn from_base_units(raw: &str, decimals: u32) -> String {
    let digits = raw.trim().trim_start_matches('0');
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return "0".to_string();
    }
    let d = decimals as usize;
    if d == 0 {
        return digits.to_string();
    }
    let padded = if digits.len() <= d {
        format!("{}{}", "0".repeat(d - digits.len() + 1), digits)
    } else {
        digits.to_string()
    };
    let (int_part, frac_part) = padded.split_at(padded.len() - d);
    let frac_trimmed = frac_part.trim_end_matches('0');
    if frac_trimmed.is_empty() {
        int_part.to_string()
    } else {
        format!("{int_part}.{frac_trimmed}")
    }
}

/// Parse a base-unit string into u128 (for comparisons / arithmetic).
pub(crate) fn parse_u128(v: Option<&Value>) -> Option<u128> {
    match v? {
        Value::String(s) => s.trim().parse::<u128>().ok(),
        Value::Number(n) => n.as_u64().map(u128::from),
        _ => None,
    }
}

/// 1e18-scaled ("wad") integer string → fraction (0.05 for 5%).
pub(crate) fn wad_to_fraction(v: Option<&Value>) -> Option<f64> {
    let s = match v? {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    s.parse::<f64>().ok().map(|x| x / 1e18)
}

/// 1e27-scaled ("ray") share price → f64.
pub(crate) fn ray_to_f64(v: Option<&Value>) -> Option<f64> {
    let s = match v? {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    s.parse::<f64>().ok().map(|x| x / 1e27)
}

/// Human string + raw string pair for a base-unit amount.
pub(crate) fn amount_pair(raw: Option<&Value>, decimals: u32) -> Value {
    match raw.and_then(|v| {
        v.as_str()
            .map(str::to_string)
            .or_else(|| v.as_u64().map(|n| n.to_string()))
    }) {
        Some(s) => json!({ "raw": s, "human": from_base_units(&s, decimals) }),
        None => Value::Null,
    }
}

pub(crate) fn asset_decimals(config: &Value) -> u32 {
    config
        .pointer("/asset/decimals")
        .and_then(Value::as_u64)
        .map(|d| d as u32)
        .unwrap_or(18)
}

pub(crate) fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn rewards_list(v: Option<&Value>) -> Vec<Value> {
    v.and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|r| {
                    json!({
                        "asset": r.pointer("/asset/symbol"),
                        "apr": r.get("supplyApr"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn warnings_list(v: Option<&Value>) -> Vec<Value> {
    v.and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|w| json!({ "type": w.get("type"), "level": w.get("level") }))
                .collect()
        })
        .unwrap_or_default()
}

/// Keep at most `max` evenly spaced points (always keeping first and last).
pub(crate) fn downsample(points: &[Value], max: usize) -> Vec<Value> {
    if points.len() <= max || max < 2 {
        return points.to_vec();
    }
    let last = points.len() - 1;
    (0..max)
        .map(|i| points[i * last / (max - 1)].clone())
        .collect()
}

pub(crate) fn series_summary(points: &[Value], key: &str) -> Value {
    let values: Vec<f64> = points
        .iter()
        .filter_map(|p| {
            p.get(key).and_then(|v| {
                v.as_f64()
                    .or_else(|| v.as_str().and_then(|s| s.parse::<f64>().ok()))
            })
        })
        .collect();
    if values.is_empty() {
        return Value::Null;
    }
    let first = values[0];
    let last = values[values.len() - 1];
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let avg = values.iter().sum::<f64>() / values.len() as f64;
    let change_pct = if first != 0.0 {
        Some((last - first) / first * 100.0)
    } else {
        None
    };
    json!({
        "first": first, "last": last, "min": min, "max": max, "avg": avg,
        "change_pct": change_pct, "points": values.len(),
    })
}

pub(crate) fn merge_into(target: &mut Map<String, Value>, extra: Value) {
    if let Value::Object(map) = extra {
        for (k, v) in map {
            target.insert(k, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_units_round_trip() {
        assert_eq!(to_base_units("1250.5", 6).unwrap(), "1250500000");
        assert_eq!(to_base_units("1,000", 6).unwrap(), "1000000000");
        assert_eq!(to_base_units("0.000001", 6).unwrap(), "1");
        assert_eq!(to_base_units(".5", 18).unwrap(), "500000000000000000");
        assert_eq!(to_base_units("7", 0).unwrap(), "7");
        assert!(to_base_units("0.0000001", 6).is_err());
        assert!(to_base_units("-5", 6).is_err());
        assert!(to_base_units("0", 6).is_err());
        assert!(to_base_units("1e6", 6).is_err());
        assert_eq!(from_base_units("1250500000", 6), "1250.5");
        assert_eq!(from_base_units("1", 6), "0.000001");
        assert_eq!(from_base_units("5664529377824", 6), "5664529.377824");
        assert_eq!(from_base_units("0", 6), "0");
        assert_eq!(from_base_units("42", 0), "42");
    }

    #[test]
    fn version_and_lookback_parsing() {
        assert_eq!(VaultVersion::parse(None).unwrap(), None);
        assert_eq!(
            VaultVersion::parse(Some("V2")).unwrap(),
            Some(VaultVersion::V2)
        );
        assert_eq!(
            VaultVersion::parse(Some("metamorpho")).unwrap(),
            Some(VaultVersion::V1)
        );
        assert!(VaultVersion::parse(Some("v3")).is_err());
        assert_eq!(normalize_lookback(None).unwrap(), "seven_days");
        assert_eq!(normalize_lookback(Some("30d")).unwrap(), "thirty_days");
        assert_eq!(normalize_lookback(Some("inception")).unwrap(), "inception");
        assert!(normalize_lookback(Some("2y")).is_err());
    }

    #[test]
    fn scaled_number_helpers() {
        let wad = json!("50000000000000000");
        assert!((wad_to_fraction(Some(&wad)).unwrap() - 0.05).abs() < 1e-12);
        let ray = json!("1054356122506410105715360912");
        assert!((ray_to_f64(Some(&ray)).unwrap() - 1.0543561).abs() < 1e-6);
        assert_eq!(parse_u128(Some(&json!("312891151370"))), Some(312891151370));
        assert_eq!(chain_name(8453), "base");
        assert_eq!(chain_name(4242), "chain-4242");
    }

    #[test]
    fn downsample_keeps_endpoints() {
        let pts: Vec<Value> = (0..100).map(|i| json!({ "apy": i as f64 })).collect();
        let ds = downsample(&pts, 5);
        assert_eq!(ds.len(), 5);
        assert_eq!(ds[0]["apy"], 0.0);
        assert_eq!(ds[4]["apy"], 99.0);
        let summary = series_summary(&pts, "apy");
        assert_eq!(summary["min"], 0.0);
        assert_eq!(summary["max"], 99.0);
        assert_eq!(summary["points"], 100);
    }

    #[test]
    fn asset_filter_detects_addresses() {
        match AssetFilter::parse("usdc").unwrap() {
            AssetFilter::Symbol(s) => assert_eq!(s, "USDC"),
            _ => panic!("expected symbol"),
        }
        match AssetFilter::parse("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap() {
            AssetFilter::Address(a) => assert!(a.starts_with("0xA0b8")),
            _ => panic!("expected address"),
        }
        assert!(AssetFilter::parse("  ").is_none());
    }
}
