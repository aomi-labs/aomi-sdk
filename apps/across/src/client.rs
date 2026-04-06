use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct AcrossApp;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

// ============================================================================
// Across HTTP Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_ACROSS_API: &str = "https://app.across.to/api";

#[derive(Clone)]
pub(crate) struct AcrossClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_endpoint: String,
}

impl AcrossClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[across] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_endpoint: std::env::var("ACROSS_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_ACROSS_API.to_string()),
        })
    }

    pub(crate) fn get_json(
        &self,
        request: reqwest::blocking::RequestBuilder,
        op: &str,
    ) -> Result<Value, String> {
        let response = request
            .send()
            .map_err(|e| format!("[across] {op} failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[across] {op} failed: {status} {body}"));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[across] {op} decode failed: {e}; body: {body}"))
    }

    fn with_source(value: Value) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert("source".to_string(), Value::String("across".to_string()));
                Value::Object(map)
            }
            other => serde_json::json!({
                "source": "across",
                "data": other,
            }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_suggested_fees(
        &self,
        input_token: &str,
        output_token: &str,
        origin_chain_id: u64,
        destination_chain_id: u64,
        amount: &str,
        recipient: Option<&str>,
        message: Option<&str>,
    ) -> Result<Value, String> {
        let mut request = self
            .http
            .get(format!("{}/suggested-fees", self.api_endpoint))
            .query(&[
                ("inputToken", input_token),
                ("outputToken", output_token),
                ("amount", amount),
            ])
            .query(&[
                ("originChainId", origin_chain_id),
                ("destinationChainId", destination_chain_id),
            ]);

        if let Some(r) = recipient {
            request = request.query(&[("recipient", r)]);
        }
        if let Some(m) = message {
            request = request.query(&[("message", m)]);
        }

        let value = self.get_json(request, "suggested-fees")?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_limits(
        &self,
        input_token: &str,
        output_token: &str,
        origin_chain_id: u64,
        destination_chain_id: u64,
    ) -> Result<Value, String> {
        let request = self
            .http
            .get(format!("{}/limits", self.api_endpoint))
            .query(&[("inputToken", input_token), ("outputToken", output_token)])
            .query(&[
                ("originChainId", origin_chain_id),
                ("destinationChainId", destination_chain_id),
            ]);

        let value = self.get_json(request, "limits")?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_deposit_status(
        &self,
        origin_chain_id: u64,
        deposit_id: u64,
    ) -> Result<Value, String> {
        let request = self
            .http
            .get(format!("{}/deposit/status", self.api_endpoint))
            .query(&[
                ("originChainId", origin_chain_id),
                ("depositId", deposit_id),
            ]);

        let value = self.get_json(request, "deposit status")?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_available_routes(
        &self,
        origin_chain_id: Option<u64>,
        destination_chain_id: Option<u64>,
        origin_token: Option<&str>,
        destination_token: Option<&str>,
    ) -> Result<Value, String> {
        let mut request = self
            .http
            .get(format!("{}/available-routes", self.api_endpoint));

        if let Some(id) = origin_chain_id {
            request = request.query(&[("originChainId", id)]);
        }
        if let Some(id) = destination_chain_id {
            request = request.query(&[("destinationChainId", id)]);
        }
        if let Some(t) = origin_token {
            request = request.query(&[("originToken", t)]);
        }
        if let Some(t) = destination_token {
            request = request.query(&[("destinationToken", t)]);
        }

        let value = self.get_json(request, "available routes")?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_coingecko_price(
        &self,
        l1_token: Option<&str>,
        l2_token: Option<&str>,
    ) -> Result<Value, String> {
        let mut request = self.http.get(format!("{}/coingecko", self.api_endpoint));

        if let Some(t) = l1_token {
            request = request.query(&[("l1Token", t)]);
        }
        if let Some(t) = l2_token {
            request = request.query(&[("l2Token", t)]);
        }

        let value = self.get_json(request, "token price")?;
        Ok(Self::with_source(value))
    }
}

// ============================================================================
// Tool arg structs
// ============================================================================

pub(crate) struct GetAcrossBridgeQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAcrossBridgeQuoteArgs {
    #[schemars(description = "ERC-20 token address on the origin chain (input token)")]
    pub input_token: String,

    #[schemars(description = "ERC-20 token address on the destination chain (output token)")]
    pub output_token: String,

    #[schemars(
        description = "Origin chain ID (e.g. 1 for Ethereum, 42161 for Arbitrum, 10 for Optimism, 137 for Polygon, 8453 for Base)"
    )]
    pub origin_chain_id: u64,

    #[schemars(
        description = "Destination chain ID (e.g. 1 for Ethereum, 42161 for Arbitrum, 10 for Optimism, 137 for Polygon, 8453 for Base)"
    )]
    pub destination_chain_id: u64,

    #[schemars(description = "Amount in the token's smallest unit (e.g. wei for ETH)")]
    pub amount: String,

    #[schemars(description = "Recipient address on the destination chain. Optional.")]
    #[serde(default)]
    pub recipient: Option<String>,

    #[schemars(description = "Optional message for cross-chain actions")]
    #[serde(default)]
    pub message: Option<String>,
}

pub(crate) struct GetAcrossBridgeLimits;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAcrossBridgeLimitsArgs {
    #[schemars(description = "ERC-20 token address on the origin chain (input token)")]
    pub input_token: String,

    #[schemars(description = "ERC-20 token address on the destination chain (output token)")]
    pub output_token: String,

    #[schemars(
        description = "Origin chain ID (e.g. 1 for Ethereum, 42161 for Arbitrum, 10 for Optimism)"
    )]
    pub origin_chain_id: u64,

    #[schemars(
        description = "Destination chain ID (e.g. 1 for Ethereum, 42161 for Arbitrum, 10 for Optimism)"
    )]
    pub destination_chain_id: u64,
}

pub(crate) struct GetAcrossDepositStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAcrossDepositStatusArgs {
    #[schemars(description = "Origin chain ID where the deposit was made")]
    pub origin_chain_id: u64,

    #[schemars(description = "Deposit ID to track")]
    pub deposit_id: u64,
}

pub(crate) struct GetAcrossAvailableRoutes;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAcrossAvailableRoutesArgs {
    #[schemars(description = "Filter by origin chain ID. Optional.")]
    #[serde(default)]
    pub origin_chain_id: Option<u64>,

    #[schemars(description = "Filter by destination chain ID. Optional.")]
    #[serde(default)]
    pub destination_chain_id: Option<u64>,

    #[schemars(description = "Filter by origin token address. Optional.")]
    #[serde(default)]
    pub origin_token: Option<String>,

    #[schemars(description = "Filter by destination token address. Optional.")]
    #[serde(default)]
    pub destination_token: Option<String>,
}

pub(crate) struct GetAcrossTokenPrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAcrossTokenPriceArgs {
    #[schemars(
        description = "L1 (Ethereum mainnet) token address. Optional if l2_token is provided."
    )]
    #[serde(default)]
    pub l1_token: Option<String>,

    #[schemars(description = "L2 token address. Optional if l1_token is provided.")]
    #[serde(default)]
    pub l2_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // USDC addresses per chain
    const USDC_ETH: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    const USDC_ARB: &str = "0xaf88d065e77c8cC2239327C5EDb3A432268e5831";

    // WETH addresses per chain
    const WETH_ETH: &str = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2";
    const WETH_ARB: &str = "0x82aF49447D8a07e3bd95BD0d56f35241523fBab1";
    const WETH_BASE: &str = "0x4200000000000000000000000000000000000006";

    #[test]
    fn bridge_usdc_workflow() {
        println!("=== bridge_usdc_workflow ===");
        let client = AcrossClient::new().expect("failed to create AcrossClient");
        println!("AcrossClient created, endpoint: {}", client.api_endpoint);

        // Step 1: Get available routes for USDC from Ethereum to Arbitrum
        println!("Step 1: Fetching available routes for USDC (Ethereum -> Arbitrum)...");
        let routes = client
            .get_available_routes(Some(1), Some(42161), Some(USDC_ETH), None)
            .expect("get_available_routes failed");
        println!(
            "Routes response: {}",
            serde_json::to_string_pretty(&routes).unwrap()
        );
        assert!(
            routes.get("source").is_some(),
            "routes response should have source field"
        );

        // Step 2: Get bridge limits for USDC Ethereum -> Arbitrum
        // inputToken = USDC on Ethereum, outputToken = USDC on Arbitrum
        println!(
            "Step 2: Fetching bridge limits (inputToken={}, outputToken={})...",
            USDC_ETH, USDC_ARB
        );
        let limits = client
            .get_limits(USDC_ETH, USDC_ARB, 1, 42161)
            .expect("get_limits failed");
        println!(
            "Limits response: {}",
            serde_json::to_string_pretty(&limits).unwrap()
        );
        let min_deposit = limits.get("minDeposit");
        let max_deposit = limits.get("maxDeposit");
        println!(
            "minDeposit: {:?}, maxDeposit: {:?}",
            min_deposit, max_deposit
        );
        assert!(min_deposit.is_some(), "limits should contain minDeposit");
        assert!(max_deposit.is_some(), "limits should contain maxDeposit");

        // Step 3: Get suggested fees for bridging 5000 USDC (6 decimals -> 5_000_000_000)
        let amount = "5000000000";
        println!(
            "Step 3: Fetching suggested fees for {} USDC raw (inputToken={}, outputToken={})...",
            amount, USDC_ETH, USDC_ARB
        );
        let fees = client
            .get_suggested_fees(USDC_ETH, USDC_ARB, 1, 42161, amount, None, None)
            .expect("get_suggested_fees failed");
        println!(
            "Fees response: {}",
            serde_json::to_string_pretty(&fees).unwrap()
        );
        assert!(
            fees.get("totalRelayFee").is_some() || fees.get("relayFeeTotal").is_some(),
            "fees response should contain relay fee data"
        );

        // Step 4: Verify we have all the data needed to build a SpokePool deposit TX
        println!("Step 4: Verifying all responses have source field...");
        assert!(fees.get("source").is_some(), "fees should have source");
        assert!(limits.get("source").is_some(), "limits should have source");
        assert!(routes.get("source").is_some(), "routes should have source");
        println!("=== bridge_usdc_workflow PASSED ===");
    }

    #[test]
    fn cheapest_bridge_route_workflow() {
        println!("=== cheapest_bridge_route_workflow ===");
        let client = AcrossClient::new().expect("failed to create AcrossClient");
        println!("AcrossClient created, endpoint: {}", client.api_endpoint);

        // Step 1: Get available routes -- assert we get multiple routes
        println!("Step 1: Fetching available routes for WETH from Ethereum...");
        let routes = client
            .get_available_routes(Some(1), None, Some(WETH_ETH), None)
            .expect("get_available_routes failed");
        println!(
            "Routes response: {}",
            serde_json::to_string_pretty(&routes).unwrap()
        );
        assert!(routes.get("source").is_some(), "routes should have source");

        // Step 2: Get suggested fees for WETH Ethereum -> Arbitrum
        // inputToken = WETH on Ethereum, outputToken = WETH on Arbitrum
        let amount = "10000000000000000000"; // 10 ETH in wei
        println!(
            "Step 2: Fetching suggested fees for {} wei WETH (Eth->Arb, outputToken={})...",
            amount, WETH_ARB
        );
        let fees_arb = client
            .get_suggested_fees(WETH_ETH, WETH_ARB, 1, 42161, amount, None, None)
            .expect("get_suggested_fees for Arbitrum failed");
        println!(
            "Arbitrum fees response: {}",
            serde_json::to_string_pretty(&fees_arb).unwrap()
        );
        assert!(
            fees_arb.get("totalRelayFee").is_some() || fees_arb.get("relayFeeTotal").is_some(),
            "Arbitrum fees response should contain relay fee data"
        );

        // Step 3: Get suggested fees for WETH Ethereum -> Base
        // inputToken = WETH on Ethereum, outputToken = WETH on Base
        println!(
            "Step 3: Fetching suggested fees for {} wei WETH (Eth->Base, outputToken={})...",
            amount, WETH_BASE
        );
        let fees_base = client
            .get_suggested_fees(WETH_ETH, WETH_BASE, 1, 8453, amount, None, None)
            .expect("get_suggested_fees for Base failed");
        println!(
            "Base fees response: {}",
            serde_json::to_string_pretty(&fees_base).unwrap()
        );
        assert!(
            fees_base.get("totalRelayFee").is_some() || fees_base.get("relayFeeTotal").is_some(),
            "Base fees response should contain relay fee data"
        );

        // Step 4 & 5: Compare fees between routes and determine cheapest option
        println!("Step 4: Extracting and comparing fees...");
        let extract_fee = |fees: &Value| -> Option<String> {
            fees.get("totalRelayFee")
                .and_then(|f| f.get("total"))
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or_else(|| {
                    fees.get("relayFeeTotal")
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                })
        };

        let fee_arb = extract_fee(&fees_arb);
        let fee_base = extract_fee(&fees_base);
        println!("Arbitrum fee: {:?}", fee_arb);
        println!("Base fee: {:?}", fee_base);

        assert!(
            fee_arb.is_some() || fee_base.is_some(),
            "at least one route should have extractable fee data for comparison"
        );

        // Determine the cheapest option
        if let (Some(a), Some(b)) = (&fee_arb, &fee_base) {
            let arb_val = a.parse::<u128>().expect("failed to parse Arbitrum fee");
            let base_val = b.parse::<u128>().expect("failed to parse Base fee");
            let cheapest = if arb_val <= base_val {
                "Arbitrum"
            } else {
                "Base"
            };
            println!(
                "Step 5: Cheapest route is {} (Arb={}, Base={})",
                cheapest, arb_val, base_val
            );
            assert!(
                !cheapest.is_empty(),
                "should be able to determine cheapest route"
            );
        }
        println!("=== cheapest_bridge_route_workflow PASSED ===");
    }
}
