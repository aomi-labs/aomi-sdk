use crate::types::AccountQuery;
use aomi_sdk::schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct GmxApp;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

// ============================================================================
// GMX v2 Client (blocking)
// ============================================================================

pub(crate) const ARBITRUM_API: &str = "https://arbitrum-api.gmxinfra.io";
pub(crate) const AVALANCHE_API: &str = "https://avalanche-api.gmxinfra.io";

#[derive(Clone)]
pub(crate) struct GmxClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) base_url: String,
}

impl GmxClient {
    pub(crate) fn new(chain: Option<&str>) -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[gmx] failed to build HTTP client: {e}"))?;

        let base_url = match chain.map(|s| s.to_lowercase()).as_deref() {
            Some("avalanche") | Some("avax") => std::env::var("GMX_AVALANCHE_API_ENDPOINT")
                .unwrap_or_else(|_| AVALANCHE_API.to_string()),
            _ => std::env::var("GMX_ARBITRUM_API_ENDPOINT")
                .unwrap_or_else(|_| ARBITRUM_API.to_string()),
        };

        Ok(Self { http, base_url })
    }

    pub(crate) fn get_json(&self, path: &str, op: &str) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(&url)
            .send()
            .map_err(|e| format!("[gmx] {op} request failed ({url}): {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "[gmx] {op} request failed ({url}): {status} {body}"
            ));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[gmx] {op} decode failed ({url}): {e}; body: {body}"))
    }

    pub(crate) fn get_json_with_query<Q: Serialize>(
        &self,
        path: &str,
        query: &Q,
        op: &str,
    ) -> Result<Value, String> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .http
            .get(&url)
            .query(query)
            .send()
            .map_err(|e| format!("[gmx] {op} request failed ({url}): {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "[gmx] {op} request failed ({url}): {status} {body}"
            ));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[gmx] {op} decode failed ({url}): {e}; body: {body}"))
    }

    // ========================================================================
    // API methods
    // ========================================================================

    pub(crate) fn get_prices(&self) -> Result<Value, String> {
        self.get_json("/prices/tickers", "prices")
    }

    pub(crate) fn get_signed_prices(&self) -> Result<Value, String> {
        self.get_json("/signed_prices/latest", "signed prices")
    }

    pub(crate) fn get_markets(&self) -> Result<Value, String> {
        self.get_json("/markets/info", "markets")
    }

    pub(crate) fn get_positions(&self, account: &str) -> Result<Value, String> {
        self.get_json_with_query("/positions", &AccountQuery { account }, "positions")
    }

    pub(crate) fn get_orders(&self, account: &str) -> Result<Value, String> {
        self.get_json_with_query("/orders", &AccountQuery { account }, "orders")
    }
}

pub(crate) fn resolve_chain_label(chain: Option<&str>) -> &str {
    match chain.map(|s| s.to_lowercase()).as_deref() {
        Some("avalanche") | Some("avax") => "avalanche",
        _ => "arbitrum",
    }
}

// ============================================================================
// Tool arg structs
// ============================================================================

pub(crate) struct GetGmxPrices;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxPricesArgs {
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

pub(crate) struct GetGmxSignedPrices;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxSignedPricesArgs {
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

pub(crate) struct GetGmxMarkets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxMarketsArgs {
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

pub(crate) struct GetGmxPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxPositionsArgs {
    /// Ethereum address of the account to query positions for (e.g. "0x1234...")
    pub(crate) account: String,
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

pub(crate) struct GetGmxOrders;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetGmxOrdersArgs {
    /// Ethereum address of the account to query orders for (e.g. "0x1234...")
    pub(crate) account: String,
    /// Chain to query: "arbitrum" (default) or "avalanche"
    #[serde(default)]
    pub(crate) chain: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

    #[test]
    fn open_leveraged_long_workflow() {
        let client = GmxClient::new(Some("arbitrum")).expect("should build client");

        let markets_resp = client.get_markets().expect("should fetch markets");
        let markets_arr = markets_resp
            .get("markets")
            .and_then(|v| v.as_array())
            .or_else(|| markets_resp.as_array())
            .expect("markets response should contain an array of markets");
        assert!(!markets_arr.is_empty(), "markets should not be empty");
        let first_market = &markets_arr[0];
        assert!(first_market.get("marketToken").is_some());
        assert!(
            first_market.get("indexToken").is_some()
                && first_market.get("longToken").is_some()
                && first_market.get("shortToken").is_some()
        );

        let prices = client.get_prices().expect("should fetch prices");
        let prices_arr = prices.as_array().expect("prices should be an array");
        assert!(!prices_arr.is_empty(), "prices should not be empty");
        let has_eth = prices_arr.iter().any(|p| {
            let s = p.to_string().to_uppercase();
            s.contains("ETH") || s.contains("WETH")
        });
        assert!(has_eth, "prices should include ETH or WETH");

        let signed = client
            .get_signed_prices()
            .expect("should fetch signed prices");
        let signed_prices_arr = signed
            .get("signedPrices")
            .and_then(|v| v.as_array())
            .expect("signed prices should contain a signedPrices array");
        assert!(!signed_prices_arr.is_empty());
    }

    #[test]
    fn take_profit_workflow() {
        let client = GmxClient::new(Some("arbitrum")).expect("should build client");

        let positions_resp = client
            .get_positions(ZERO_ADDRESS)
            .expect("should fetch positions");
        let positions_arr = positions_resp
            .get("positions")
            .and_then(|v| v.as_array())
            .or_else(|| positions_resp.as_array())
            .expect("positions response should contain an array");
        for pos in positions_arr.iter() {
            assert!(pos.is_object(), "each position entry should be an object");
        }

        let prices = client.get_prices().expect("should fetch prices");
        let prices_arr = prices.as_array().expect("prices should be an array");
        assert!(!prices_arr.is_empty(), "prices should not be empty");

        match client.get_orders(ZERO_ADDRESS) {
            Ok(orders_resp) => {
                let orders_arr = orders_resp
                    .get("orders")
                    .and_then(|v| v.as_array())
                    .or_else(|| orders_resp.as_array())
                    .expect("orders response should contain an array");
                for order in orders_arr.iter() {
                    assert!(order.is_object(), "each order entry should be an object");
                }
            }
            Err(e) => {
                assert!(
                    e.contains("404"),
                    "orders error should be a 404 not-found, got: {e}"
                );
            }
        }
    }
}
