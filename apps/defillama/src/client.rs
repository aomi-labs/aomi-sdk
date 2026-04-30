use crate::types::{
    ExcludedChartsQuery, FeesOverviewQuery, HistoricalTokenPriceQuery, IncludePricesQuery,
    ProtocolFeesQuery, StablecoinHistoryQuery, TokenPriceChangeQuery,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct DefiLlamaApp;

// ============================================================================
// DefiLama Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_DEFILLAMA_API: &str = "https://api.llama.fi";
pub(crate) const DEFAULT_DEFILLAMA_COINS_API: &str = "https://coins.llama.fi";
pub(crate) const DEFAULT_DEFILLAMA_YIELDS_API: &str = "https://yields.llama.fi";
pub(crate) const DEFAULT_DEFILLAMA_STABLECOINS_API: &str = "https://stablecoins.llama.fi";

#[derive(Clone)]
pub(crate) struct DefiLamaClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_endpoint: String,
    pub(crate) coins_endpoint: String,
    pub(crate) yields_endpoint: String,
    pub(crate) stablecoins_endpoint: String,
}

impl DefiLamaClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_endpoint: std::env::var("DEFILLAMA_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DEFILLAMA_API.to_string()),
            coins_endpoint: std::env::var("DEFILLAMA_COINS_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DEFILLAMA_COINS_API.to_string()),
            yields_endpoint: std::env::var("DEFILLAMA_YIELDS_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DEFILLAMA_YIELDS_API.to_string()),
            stablecoins_endpoint: std::env::var("DEFILLAMA_STABLECOINS_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_DEFILLAMA_STABLECOINS_API.to_string()),
        })
    }

    fn get_json(&self, url: &str, op: &str) -> Result<Value, String> {
        let response = self
            .http
            .get(url)
            .send()
            .map_err(|e| format!("[defillama] {op} request failed ({url}): {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "[defillama] {op} request failed ({url}): {status} {body}"
            ));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[defillama] {op} decode failed ({url}): {e}; body: {body}"))
    }

    fn get_json_with_query<Q: Serialize>(
        &self,
        url: &str,
        query: &Q,
        op: &str,
    ) -> Result<Value, String> {
        let response = self
            .http
            .get(url)
            .query(query)
            .send()
            .map_err(|e| format!("[defillama] {op} request failed ({url}): {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "[defillama] {op} request failed ({url}): {status} {body}"
            ));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[defillama] {op} decode failed ({url}): {e}; body: {body}"))
    }

    pub(crate) fn get_token_price(&self, token: &str) -> Result<Value, String> {
        let coin_id = normalize_token_id(token);
        let url = format!("{}/prices/current/{}", self.coins_endpoint, coin_id);
        self.get_json(&url, "token price")
    }

    pub(crate) fn get_yield_pools(
        &self,
        chain: Option<&str>,
        project: Option<&str>,
    ) -> Result<Value, String> {
        let url = format!("{}/pools", self.yields_endpoint);
        let mut value = self.get_json(&url, "yield pools")?;

        if let Some(data) = value.get_mut("data").and_then(Value::as_array_mut) {
            data.retain(|pool| {
                let chain_ok = chain
                    .map(|c| {
                        pool.get("chain")
                            .and_then(Value::as_str)
                            .map(|s| s.eq_ignore_ascii_case(c))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true);
                let project_ok = project
                    .map(|p| {
                        let p_lower = p.to_lowercase();
                        pool.get("project")
                            .and_then(Value::as_str)
                            .map(|s| s.to_lowercase().contains(&p_lower))
                            .unwrap_or(false)
                    })
                    .unwrap_or(true);
                let apy_ok = pool.get("apy").and_then(Value::as_f64).unwrap_or(0.0) > 0.0;
                chain_ok && project_ok && apy_ok
            });
        }

        Ok(value)
    }

    pub(crate) fn get_protocols(&self, category: Option<&str>) -> Result<Value, String> {
        let url = format!("{}/protocols", self.api_endpoint);
        let mut value = self.get_json(&url, "protocols")?;

        if let Some(arr) = value.as_array_mut() {
            if let Some(category_filter) = category {
                let category_filter = category_filter.to_lowercase();
                arr.retain(|protocol| {
                    protocol
                        .get("category")
                        .and_then(Value::as_str)
                        .map(|s| s.to_lowercase().contains(&category_filter))
                        .unwrap_or(false)
                });
            }
            arr.sort_by(|a, b| {
                let at = a.get("tvl").and_then(Value::as_f64).unwrap_or(0.0);
                let bt = b.get("tvl").and_then(Value::as_f64).unwrap_or(0.0);
                bt.partial_cmp(&at).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(value)
    }

    pub(crate) fn get_chains_tvl(&self) -> Result<Value, String> {
        let url = format!("{}/v2/chains", self.api_endpoint);
        let mut value = self.get_json(&url, "chains tvl")?;

        if let Some(arr) = value.as_array_mut() {
            arr.sort_by(|a, b| {
                let at = a.get("tvl").and_then(Value::as_f64).unwrap_or(0.0);
                let bt = b.get("tvl").and_then(Value::as_f64).unwrap_or(0.0);
                bt.partial_cmp(&at).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        Ok(value)
    }

    pub(crate) fn get_protocol_detail(&self, protocol: &str) -> Result<Value, String> {
        let url = format!("{}/protocol/{}", self.api_endpoint, protocol);
        self.get_json(&url, "protocol detail")
    }

    pub(crate) fn get_dex_volumes(
        &self,
        chain: Option<&str>,
        exclude_total_data_chart: Option<bool>,
        exclude_total_data_chart_breakdown: Option<bool>,
    ) -> Result<Value, String> {
        let url = match chain {
            Some(c) => format!("{}/overview/dexs/{}", self.api_endpoint, c),
            None => format!("{}/overview/dexs", self.api_endpoint),
        };
        self.get_json_with_query(
            &url,
            &ExcludedChartsQuery {
                exclude_total_data_chart: exclude_total_data_chart.unwrap_or(true),
                exclude_total_data_chart_breakdown: exclude_total_data_chart_breakdown
                    .unwrap_or(true),
            },
            "dex volumes",
        )
    }

    pub(crate) fn get_fees_overview(
        &self,
        chain: Option<&str>,
        exclude_total_data_chart: Option<bool>,
        exclude_total_data_chart_breakdown: Option<bool>,
        data_type: Option<&str>,
    ) -> Result<Value, String> {
        let url = match chain {
            Some(c) => format!("{}/overview/fees/{}", self.api_endpoint, c),
            None => format!("{}/overview/fees", self.api_endpoint),
        };
        self.get_json_with_query(
            &url,
            &FeesOverviewQuery {
                exclude_total_data_chart: exclude_total_data_chart.unwrap_or(true),
                exclude_total_data_chart_breakdown: exclude_total_data_chart_breakdown
                    .unwrap_or(true),
                data_type: data_type.map(str::to_string),
            },
            "fees overview",
        )
    }

    pub(crate) fn get_protocol_fees(
        &self,
        protocol: &str,
        data_type: Option<&str>,
    ) -> Result<Value, String> {
        let url = format!("{}/summary/fees/{}", self.api_endpoint, protocol);
        self.get_json_with_query(
            &url,
            &ProtocolFeesQuery {
                data_type: data_type.map(str::to_string),
            },
            "protocol fees",
        )
    }

    pub(crate) fn get_stablecoins(&self, include_prices: Option<bool>) -> Result<Value, String> {
        let url = format!("{}/stablecoins", self.stablecoins_endpoint);
        self.get_json_with_query(
            &url,
            &IncludePricesQuery {
                include_prices: include_prices.unwrap_or(true),
            },
            "stablecoins",
        )
    }

    pub(crate) fn get_stablecoin_chains(&self) -> Result<Value, String> {
        let url = format!("{}/stablecoinchains", self.stablecoins_endpoint);
        self.get_json(&url, "stablecoin chains")
    }

    pub(crate) fn get_historical_token_price(
        &self,
        coins: &str,
        start: Option<u64>,
        end: Option<u64>,
        span: Option<u64>,
        period: Option<&str>,
    ) -> Result<Value, String> {
        let url = format!("{}/chart/{}", self.coins_endpoint, coins);
        self.get_json_with_query(
            &url,
            &HistoricalTokenPriceQuery {
                start,
                end,
                span,
                period: period.map(str::to_string),
            },
            "historical token price",
        )
    }

    pub(crate) fn get_token_price_change(
        &self,
        coins: &str,
        timestamp: Option<u64>,
        look_forward: Option<bool>,
        period: Option<&str>,
    ) -> Result<Value, String> {
        let url = format!("{}/percentage/{}", self.coins_endpoint, coins);
        self.get_json_with_query(
            &url,
            &TokenPriceChangeQuery {
                timestamp,
                look_forward,
                period: period.map(str::to_string),
            },
            "token price change",
        )
    }

    pub(crate) fn get_historical_chain_tvl(&self, chain: &str) -> Result<Value, String> {
        let url = format!("{}/v2/historicalChainTvl/{}", self.api_endpoint, chain);
        self.get_json(&url, "historical chain tvl")
    }

    pub(crate) fn get_dex_protocol_volume(
        &self,
        protocol: &str,
        exclude_total_data_chart: Option<bool>,
        exclude_total_data_chart_breakdown: Option<bool>,
    ) -> Result<Value, String> {
        let url = format!("{}/summary/dexs/{}", self.api_endpoint, protocol);
        self.get_json_with_query(
            &url,
            &ExcludedChartsQuery {
                exclude_total_data_chart: exclude_total_data_chart.unwrap_or(true),
                exclude_total_data_chart_breakdown: exclude_total_data_chart_breakdown
                    .unwrap_or(true),
            },
            "dex protocol volume",
        )
    }

    pub(crate) fn get_stablecoin_history(
        &self,
        chain: Option<&str>,
        stablecoin: Option<u64>,
    ) -> Result<Value, String> {
        let url = match chain {
            Some(c) => format!("{}/stablecoincharts/{}", self.stablecoins_endpoint, c),
            None => format!("{}/stablecoincharts/all", self.stablecoins_endpoint),
        };
        self.get_json_with_query(
            &url,
            &StablecoinHistoryQuery { stablecoin },
            "stablecoin history",
        )
    }

    pub(crate) fn get_yield_pool_history(&self, pool: &str) -> Result<Value, String> {
        let url = format!("{}/chart/{}", self.yields_endpoint, pool);
        self.get_json(&url, "yield pool history")
    }
}

pub(crate) fn normalize_token_id(token: &str) -> String {
    let token_lower = token.to_lowercase();
    match token_lower.as_str() {
        "eth" | "ethereum" => "coingecko:ethereum".to_string(),
        "btc" | "bitcoin" => "coingecko:bitcoin".to_string(),
        "usdc" => "coingecko:usd-coin".to_string(),
        "usdt" | "tether" => "coingecko:tether".to_string(),
        "dai" => "coingecko:dai".to_string(),
        "sol" | "solana" => "coingecko:solana".to_string(),
        "bnb" => "coingecko:binancecoin".to_string(),
        "avax" | "avalanche" => "coingecko:avalanche-2".to_string(),
        "matic" | "polygon" => "coingecko:matic-network".to_string(),
        "arb" | "arbitrum" => "coingecko:arbitrum".to_string(),
        "op" | "optimism" => "coingecko:optimism".to_string(),
        "uni" | "uniswap" => "coingecko:uniswap".to_string(),
        "aave" => "coingecko:aave".to_string(),
        "link" | "chainlink" => "coingecko:chainlink".to_string(),
        "mkr" | "maker" => "coingecko:maker".to_string(),
        "crv" | "curve" => "coingecko:curve-dao-token".to_string(),
        "ldo" | "lido" => "coingecko:lido-dao".to_string(),
        _ => format!("coingecko:{token_lower}"),
    }
}

// ============================================================================
// Tool arg structs
// ============================================================================

pub(crate) struct GetLammaTokenPrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLammaTokenPriceArgs {
    /// Token symbol or name (e.g., "ETH", "bitcoin", "USDC")
    pub(crate) token: String,
}

pub(crate) struct GetLammaYieldOpportunities;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLammaYieldOpportunitiesArgs {
    /// Filter by chain (optional): ethereum, arbitrum, optimism, polygon, base, bsc, solana
    pub(crate) chain: Option<String>,
    /// Filter by project name (optional): aave, compound, lido, etc.
    pub(crate) project: Option<String>,
    /// Only show stablecoin pools
    pub(crate) stablecoin_only: Option<bool>,
    /// Maximum results (default: 20)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct GetLammaProtocols;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLammaProtocolsArgs {
    /// Filter by category: dexes, lending, yield, liquid-staking, bridge, derivatives
    pub(crate) category: Option<String>,
    /// Maximum results (default: 20)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct GetLammaChainTvl;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLammaChainTvlArgs {
    /// Maximum results (default: 15)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct GetLammaProtocolDetail;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaProtocolDetailArgs {
    #[schemars(description = "Protocol slug (e.g. \"aave\", \"uniswap\", \"lido\")")]
    pub protocol: String,
}

pub(crate) struct GetLammaDexVolumes;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaDexVolumesArgs {
    #[schemars(description = "Filter by chain name (e.g. \"Ethereum\", \"Arbitrum\"). Optional.")]
    #[serde(default)]
    pub chain: Option<String>,
    #[schemars(description = "Exclude total data chart from response (default: true)")]
    #[serde(default)]
    pub exclude_total_data_chart: Option<bool>,
    #[schemars(description = "Exclude total data chart breakdown from response (default: true)")]
    #[serde(default)]
    pub exclude_total_data_chart_breakdown: Option<bool>,
}

pub(crate) struct GetLammaFeesOverview;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaFeesOverviewArgs {
    #[schemars(description = "Filter by chain name (e.g. \"Ethereum\"). Optional.")]
    #[serde(default)]
    pub chain: Option<String>,
    #[schemars(description = "Exclude total data chart from response (default: true)")]
    #[serde(default)]
    pub exclude_total_data_chart: Option<bool>,
    #[schemars(description = "Exclude total data chart breakdown from response (default: true)")]
    #[serde(default)]
    pub exclude_total_data_chart_breakdown: Option<bool>,
    #[schemars(description = "Data type filter (e.g. \"dailyFees\", \"dailyRevenue\"). Optional.")]
    #[serde(default)]
    pub data_type: Option<String>,
}

pub(crate) struct GetLammaProtocolFees;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaProtocolFeesArgs {
    #[schemars(description = "Protocol slug (e.g. \"aave\", \"uniswap\")")]
    pub protocol: String,
    #[schemars(description = "Data type filter (e.g. \"dailyFees\", \"dailyRevenue\"). Optional.")]
    #[serde(default)]
    pub data_type: Option<String>,
}

pub(crate) struct GetLammaStablecoins;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaStablecoinsArgs {
    #[schemars(description = "Include price data in response (default: true)")]
    #[serde(default)]
    pub include_prices: Option<bool>,
}

pub(crate) struct GetLammaStablecoinChains;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaStablecoinChainsArgs {}

pub(crate) struct GetLammaHistoricalTokenPrice;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaHistoricalTokenPriceArgs {
    #[schemars(
        description = "Comma-separated coin identifiers (e.g. \"coingecko:ethereum,coingecko:bitcoin\")"
    )]
    pub coins: String,
    #[schemars(description = "Start unix timestamp. Optional.")]
    #[serde(default)]
    pub start: Option<u64>,
    #[schemars(description = "End unix timestamp. Optional.")]
    #[serde(default)]
    pub end: Option<u64>,
    #[schemars(description = "Number of data points to return. Optional.")]
    #[serde(default)]
    pub span: Option<u64>,
    #[schemars(
        description = "Time period between data points (e.g. \"1d\", \"1h\", \"4h\"). Optional."
    )]
    #[serde(default)]
    pub period: Option<String>,
}

pub(crate) struct GetLammaTokenPriceChange;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaTokenPriceChangeArgs {
    #[schemars(
        description = "Comma-separated coin identifiers (e.g. \"coingecko:ethereum,coingecko:bitcoin\")"
    )]
    pub coins: String,
    #[schemars(description = "Unix timestamp to calculate change from. Optional.")]
    #[serde(default)]
    pub timestamp: Option<u64>,
    #[schemars(
        description = "If true, calculate change looking forward from timestamp. Optional."
    )]
    #[serde(default)]
    pub look_forward: Option<bool>,
    #[schemars(description = "Period for price change (e.g. \"1d\", \"7d\", \"30d\"). Optional.")]
    #[serde(default)]
    pub period: Option<String>,
}

pub(crate) struct GetLammaHistoricalChainTvl;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaHistoricalChainTvlArgs {
    #[schemars(description = "Chain name (e.g. \"Ethereum\", \"Arbitrum\", \"Solana\")")]
    pub chain: String,
}

pub(crate) struct GetLammaDexProtocolVolume;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaDexProtocolVolumeArgs {
    #[schemars(description = "Protocol slug (e.g. \"uniswap\", \"curve\")")]
    pub protocol: String,
    #[schemars(description = "Exclude total data chart from response (default: true)")]
    #[serde(default)]
    pub exclude_total_data_chart: Option<bool>,
    #[schemars(description = "Exclude total data chart breakdown from response (default: true)")]
    #[serde(default)]
    pub exclude_total_data_chart_breakdown: Option<bool>,
}

pub(crate) struct GetLammaStablecoinHistory;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaStablecoinHistoryArgs {
    #[schemars(
        description = "Chain name to filter (e.g. \"Ethereum\"). Optional -- omit for all chains."
    )]
    #[serde(default)]
    pub chain: Option<String>,
    #[schemars(description = "Stablecoin ID filter. Optional.")]
    #[serde(default)]
    pub stablecoin: Option<u64>,
}

pub(crate) struct GetLammaYieldPoolHistory;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetLammaYieldPoolHistoryArgs {
    #[schemars(description = "Pool UUID (e.g. from get_yield_opportunities results)")]
    pub pool: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> DefiLamaClient {
        DefiLamaClient::new().expect("client should build")
    }

    #[test]
    fn token_price_smoke() {
        let res = client()
            .get_token_price("ethereum")
            .expect("should get ETH price");
        let coins = res.get("coins").expect("should have coins key");
        assert!(coins.as_object().map(|m| !m.is_empty()).unwrap_or(false));
    }

    #[test]
    fn protocols_smoke() {
        let res = client().get_protocols(None).expect("should get protocols");
        let arr = res.as_array();
        assert!(arr.map(|a| !a.is_empty()).unwrap_or(false));
    }

    #[test]
    fn chains_tvl_smoke() {
        let res = client().get_chains_tvl().expect("should get chains tvl");
        assert!(res.is_array());
    }

    #[test]
    fn protocol_detail_smoke() {
        let res = client()
            .get_protocol_detail("aave")
            .expect("should get aave detail");
        assert!(res.get("name").is_some(), "should have protocol name");
    }

    #[test]
    fn dex_volumes_smoke() {
        let res = client()
            .get_dex_volumes(None, None, None)
            .expect("should get dex volumes");
        assert!(res.get("protocols").is_some() || res.get("allChains").is_some());
    }

    #[test]
    fn fees_overview_smoke() {
        client()
            .get_fees_overview(None, None, None, None)
            .expect("should get fees overview");
    }

    #[test]
    fn protocol_fees_smoke() {
        client()
            .get_protocol_fees("aave", None)
            .expect("should get aave fees");
    }

    #[test]
    fn stablecoins_smoke() {
        let res = client()
            .get_stablecoins(Some(true))
            .expect("should get stablecoins");
        assert!(res.get("peggedAssets").is_some());
    }

    #[test]
    fn stablecoin_chains_smoke() {
        client()
            .get_stablecoin_chains()
            .expect("should get stablecoin chains");
    }

    #[test]
    fn historical_token_price_smoke() {
        let res = client()
            .get_historical_token_price("coingecko:ethereum", None, None, Some(5), Some("1d"))
            .expect("should get historical price chart");
        assert!(res.get("coins").is_some(), "should have coins key");
    }

    #[test]
    fn token_price_change_smoke() {
        let res = client()
            .get_token_price_change("coingecko:ethereum", None, None, Some("1d"))
            .expect("should get price change");
        assert!(res.get("coins").is_some(), "should have coins key");
    }

    #[test]
    fn historical_chain_tvl_smoke() {
        client()
            .get_historical_chain_tvl("Ethereum")
            .expect("should get historical chain tvl");
    }

    #[test]
    fn dex_protocol_volume_smoke() {
        client()
            .get_dex_protocol_volume("uniswap", None, None)
            .expect("should get uniswap volume");
    }

    #[test]
    fn stablecoin_history_smoke() {
        client()
            .get_stablecoin_history(None, None)
            .expect("should get stablecoin history");
    }

    #[test]
    fn normalize_token_id_known() {
        assert_eq!(normalize_token_id("ETH"), "coingecko:ethereum");
        assert_eq!(normalize_token_id("btc"), "coingecko:bitcoin");
        assert_eq!(normalize_token_id("USDC"), "coingecko:usd-coin");
        assert_eq!(normalize_token_id("unknown"), "coingecko:unknown");
    }
}
