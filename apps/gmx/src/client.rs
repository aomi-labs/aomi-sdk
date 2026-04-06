use aomi_sdk::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
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

    pub(crate) fn get_json_with_query(
        &self,
        path: &str,
        query: &[(&str, &str)],
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

    pub(crate) fn with_source(value: Value, chain: &str) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert("source".to_string(), Value::String("gmx".to_string()));
                map.insert("chain".to_string(), Value::String(chain.to_string()));
                Value::Object(map)
            }
            other => json!({
                "source": "gmx",
                "chain": chain,
                "data": other,
            }),
        }
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
        self.get_json_with_query("/positions", &[("account", account)], "positions")
    }

    pub(crate) fn get_orders(&self, account: &str) -> Result<Value, String> {
        self.get_json_with_query("/orders", &[("account", account)], "orders")
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

    /// Story: "Open a leveraged long on ETH via GMX"
    ///
    /// 1. Get markets -> assert we get market data with addresses and token info
    /// 2. Get prices/tickers -> assert we get token prices, find ETH price
    /// 3. Get signed prices -> assert oracle-signed prices for on-chain submission
    /// 4. Assert we have all data needed to build a createOrder TX
    #[test]
    fn open_leveraged_long_workflow() {
        println!("=== open_leveraged_long_workflow ===");
        println!("[step 0] Building GmxClient for arbitrum...");
        let client = GmxClient::new(Some("arbitrum")).expect("should build client");
        println!("[step 0] Client ready, base_url = {}", client.base_url);

        // Step 1: Get markets
        println!("[step 1] Fetching markets...");
        let markets_resp = client.get_markets().expect("should fetch markets");
        let markets_arr = markets_resp
            .get("markets")
            .and_then(|v| v.as_array())
            .or_else(|| markets_resp.as_array())
            .expect("markets response should contain an array of markets");
        assert!(!markets_arr.is_empty(), "markets should not be empty");
        println!("[step 1] Received {} markets", markets_arr.len());
        let first_market = &markets_arr[0];
        println!(
            "[step 1] First market: marketToken={}, indexToken={}, longToken={}, shortToken={}",
            first_market.get("marketToken").unwrap_or(&Value::Null),
            first_market.get("indexToken").unwrap_or(&Value::Null),
            first_market.get("longToken").unwrap_or(&Value::Null),
            first_market.get("shortToken").unwrap_or(&Value::Null),
        );
        assert!(
            first_market.get("marketToken").is_some(),
            "market entry should contain a marketToken address"
        );
        assert!(
            first_market.get("indexToken").is_some()
                && first_market.get("longToken").is_some()
                && first_market.get("shortToken").is_some(),
            "market entry should contain indexToken, longToken, and shortToken"
        );

        // Step 2: Get prices/tickers
        println!("[step 2] Fetching prices/tickers...");
        let prices = client.get_prices().expect("should fetch prices");
        let prices_arr = prices.as_array().expect("prices should be an array");
        assert!(!prices_arr.is_empty(), "prices should not be empty");
        println!("[step 2] Received {} price entries", prices_arr.len());
        let eth_entry = prices_arr.iter().find(|p| {
            let s = p.to_string().to_uppercase();
            s.contains("ETH") || s.contains("WETH")
        });
        let has_eth = eth_entry.is_some();
        if let Some(entry) = eth_entry {
            println!("[step 2] ETH price entry: {}", entry);
        }
        assert!(has_eth, "prices should include ETH or WETH");

        // Step 3: Get signed prices for on-chain submission
        println!("[step 3] Fetching signed prices...");
        let signed = client
            .get_signed_prices()
            .expect("should fetch signed prices");
        assert!(signed.is_object(), "signed prices should be an object");
        let signed_prices_arr = signed
            .get("signedPrices")
            .and_then(|v| v.as_array())
            .expect("signed prices should contain a signedPrices array");
        assert!(
            !signed_prices_arr.is_empty(),
            "signedPrices array should not be empty"
        );
        println!(
            "[step 3] Received {} signed price entries",
            signed_prices_arr.len()
        );
        if let Some(first_signed) = signed_prices_arr.first() {
            println!("[step 3] First signed price: {}", first_signed);
        }

        // Step 4: Verify we have all data needed to build a createOrder TX:
        //   - a market address to trade on
        //   - current token prices for sizing the order
        //   - oracle-signed prices for on-chain submission
        assert!(
            !markets_arr.is_empty() && !prices_arr.is_empty() && !signed_prices_arr.is_empty(),
            "should have markets, prices, and signed prices to build a createOrder TX"
        );
        println!(
            "[step 4] All data available: {} markets, {} prices, {} signed prices -- ready to build createOrder TX",
            markets_arr.len(),
            prices_arr.len(),
            signed_prices_arr.len(),
        );
        println!("=== open_leveraged_long_workflow PASSED ===");
    }

    /// Story: "Take profit on my GMX position"
    ///
    /// 1. Get positions for a zero address -> assert response structure (empty is fine)
    /// 2. Get prices -> assert we get current prices
    /// 3. Get orders for the zero address -> assert response structure
    /// 4. Assert we'd have the info to build a close order
    #[test]
    fn take_profit_workflow() {
        println!("=== take_profit_workflow ===");
        println!("[step 0] Building GmxClient for arbitrum...");
        let client = GmxClient::new(Some("arbitrum")).expect("should build client");
        println!("[step 0] Client ready, base_url = {}", client.base_url);

        // Step 1: Get positions for zero address
        println!(
            "[step 1] Fetching positions for zero address {}...",
            ZERO_ADDRESS
        );
        let positions_resp = client
            .get_positions(ZERO_ADDRESS)
            .expect("should fetch positions");
        let positions_arr = positions_resp
            .get("positions")
            .and_then(|v| v.as_array())
            .or_else(|| positions_resp.as_array())
            .expect("positions response should contain an array");
        println!(
            "[step 1] Received {} positions for zero address",
            positions_arr.len()
        );
        // Empty is expected for the zero address; just confirm the structure is valid
        for (i, pos) in positions_arr.iter().enumerate() {
            assert!(pos.is_object(), "each position entry should be an object");
            println!("[step 1] Position #{}: {}", i, pos);
        }
        if positions_arr.is_empty() {
            println!("[step 1] No positions found (expected for zero address)");
        }

        // Step 2: Get current prices
        println!("[step 2] Fetching current prices...");
        let prices = client.get_prices().expect("should fetch prices");
        let prices_arr = prices.as_array().expect("prices should be an array");
        assert!(!prices_arr.is_empty(), "prices should not be empty");
        println!("[step 2] Received {} price entries", prices_arr.len());
        let eth_price = prices_arr.iter().find(|p| {
            let s = p.to_string().to_uppercase();
            s.contains("ETH") || s.contains("WETH")
        });
        if let Some(entry) = eth_price {
            println!("[step 2] ETH price entry: {}", entry);
        }

        // Step 3: Get orders for zero address
        // The orders endpoint may not be available on all API versions;
        // we accept either a successful response or a known 404 error.
        println!(
            "[step 3] Fetching orders for zero address {}...",
            ZERO_ADDRESS
        );
        let orders_available = match client.get_orders(ZERO_ADDRESS) {
            Ok(orders_resp) => {
                let orders_arr = orders_resp
                    .get("orders")
                    .and_then(|v| v.as_array())
                    .or_else(|| orders_resp.as_array())
                    .expect("orders response should contain an array");
                println!("[step 3] Received {} orders", orders_arr.len());
                for (i, order) in orders_arr.iter().enumerate() {
                    assert!(order.is_object(), "each order entry should be an object");
                    println!("[step 3] Order #{}: {}", i, order);
                }
                if orders_arr.is_empty() {
                    println!("[step 3] No orders found (expected for zero address)");
                }
                true
            }
            Err(e) => {
                // Accept a 404 — the endpoint may not exist on this API version
                println!("[step 3] Orders endpoint returned error: {}", e);
                assert!(
                    e.contains("404"),
                    "orders error should be a 404 not-found, got: {e}"
                );
                println!("[step 3] Orders endpoint not available (404) -- acceptable");
                false
            }
        };

        // Step 4: Confirm we have enough info to build a close order:
        //   - position data to know what to close (empty is fine for zero address)
        //   - current prices to set the take-profit trigger
        //   - orders data to check existing TP orders (optional if endpoint unavailable)
        assert!(
            !prices_arr.is_empty(),
            "should have current prices to build a close/take-profit order"
        );
        // positions_arr is valid (even if empty) — sufficient for identifying what to close
        assert!(
            positions_arr.is_empty() || positions_arr.iter().all(|p| p.is_object()),
            "positions data should be well-formed"
        );
        // If orders endpoint was available, we validated it above; either way we can proceed
        println!(
            "[step 4] Summary: {} positions, {} prices, orders_available={}",
            positions_arr.len(),
            prices_arr.len(),
            orders_available,
        );
        println!("[step 4] All data available -- ready to build close/take-profit order");
        println!("=== take_profit_workflow PASSED ===");
    }
}
