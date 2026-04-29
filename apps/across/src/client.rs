use crate::types::{
    AcrossDepositStatus, AcrossLimits, AcrossRoute, AcrossSuggestedFees, AcrossTokenPrice,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde::de::DeserializeOwned;
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

    fn get_json<T: DeserializeOwned>(
        &self,
        request: reqwest::blocking::RequestBuilder,
        op: &str,
    ) -> Result<T, String> {
        let response = request
            .send()
            .map_err(|e| format!("[across] {op} failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[across] {op} failed: {status} {body}"));
        }

        serde_json::from_str::<T>(&body)
            .map_err(|e| format!("[across] {op} decode failed: {e}; body: {body}"))
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
    ) -> Result<AcrossSuggestedFees, String> {
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

        self.get_json(request, "suggested-fees")
    }

    pub(crate) fn get_limits(
        &self,
        input_token: &str,
        output_token: &str,
        origin_chain_id: u64,
        destination_chain_id: u64,
    ) -> Result<AcrossLimits, String> {
        let request = self
            .http
            .get(format!("{}/limits", self.api_endpoint))
            .query(&[("inputToken", input_token), ("outputToken", output_token)])
            .query(&[
                ("originChainId", origin_chain_id),
                ("destinationChainId", destination_chain_id),
            ]);

        self.get_json(request, "limits")
    }

    pub(crate) fn get_deposit_status(
        &self,
        origin_chain_id: u64,
        deposit_id: u64,
    ) -> Result<AcrossDepositStatus, String> {
        let request = self
            .http
            .get(format!("{}/deposit/status", self.api_endpoint))
            .query(&[
                ("originChainId", origin_chain_id),
                ("depositId", deposit_id),
            ]);

        self.get_json(request, "deposit status")
    }

    pub(crate) fn get_available_routes(
        &self,
        origin_chain_id: Option<u64>,
        destination_chain_id: Option<u64>,
        origin_token: Option<&str>,
        destination_token: Option<&str>,
    ) -> Result<Vec<AcrossRoute>, String> {
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

        self.get_json(request, "available routes")
    }

    pub(crate) fn get_coingecko_price(
        &self,
        l1_token: Option<&str>,
        l2_token: Option<&str>,
    ) -> Result<AcrossTokenPrice, String> {
        let mut request = self.http.get(format!("{}/coingecko", self.api_endpoint));

        if let Some(t) = l1_token {
            request = request.query(&[("l1Token", t)]);
        }
        if let Some(t) = l2_token {
            request = request.query(&[("l2Token", t)]);
        }

        self.get_json(request, "token price")
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
        let client = AcrossClient::new().expect("failed to create AcrossClient");

        let routes = client
            .get_available_routes(Some(1), Some(42161), Some(USDC_ETH), None)
            .expect("get_available_routes failed");
        assert!(!routes.is_empty(), "routes response should not be empty");

        let limits = client
            .get_limits(USDC_ETH, USDC_ARB, 1, 42161)
            .expect("get_limits failed");
        assert!(!limits.min_deposit.is_empty());
        assert!(!limits.max_deposit.is_empty());

        let amount = "5000000000";
        let fees = client
            .get_suggested_fees(USDC_ETH, USDC_ARB, 1, 42161, amount, None, None)
            .expect("get_suggested_fees failed");
        assert!(!fees.total_relay_fee.total.is_empty());
    }

    #[test]
    fn cheapest_bridge_route_workflow() {
        let client = AcrossClient::new().expect("failed to create AcrossClient");

        let routes = client
            .get_available_routes(Some(1), None, Some(WETH_ETH), None)
            .expect("get_available_routes failed");
        assert!(!routes.is_empty(), "routes should not be empty");

        let amount = "10000000000000000000"; // 10 ETH in wei
        let fees_arb = client
            .get_suggested_fees(WETH_ETH, WETH_ARB, 1, 42161, amount, None, None)
            .expect("get_suggested_fees for Arbitrum failed");

        let fees_base = client
            .get_suggested_fees(WETH_ETH, WETH_BASE, 1, 8453, amount, None, None)
            .expect("get_suggested_fees for Base failed");

        let arb_val = fees_arb
            .total_relay_fee
            .total
            .parse::<u128>()
            .expect("failed to parse Arbitrum fee");
        let base_val = fees_base
            .total_relay_fee
            .total
            .parse::<u128>()
            .expect("failed to parse Base fee");
        let cheapest = if arb_val <= base_val {
            "Arbitrum"
        } else {
            "Base"
        };
        assert!(!cheapest.is_empty());
    }
}
