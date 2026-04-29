use crate::types::{
    OneInchAllowanceResponse, OneInchLiquiditySourcesResponse, OneInchQuoteResponse,
    OneInchSwapResponse, OneInchTokensResponse, OneInchTransaction,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::time::Duration;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

#[derive(Clone, Default)]
pub(crate) struct OneInchApp;

// ============================================================================
// 1inch HTTP Client (blocking)
// ============================================================================

pub(crate) const BASE_URL: &str = "https://api.1inch.dev/swap/v6.0";

/// Supported chain IDs for 1inch Swap API v6.0.
pub(crate) const SUPPORTED_CHAINS: &[u64] = &[1, 10, 56, 100, 137, 8453, 42161, 43114];

#[derive(Clone)]
pub(crate) struct OneInchClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) base_url: String,
    pub(crate) api_key: String,
}

impl OneInchClient {
    pub(crate) fn new(api_key: Option<&str>) -> Result<Self, String> {
        let api_key = resolve_secret_value(
            api_key,
            "ONEINCH_API_KEY",
            "[1inch] missing api_key argument and ONEINCH_API_KEY environment variable",
        )?;
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[1inch] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            base_url: std::env::var("ONEINCH_API_ENDPOINT")
                .unwrap_or_else(|_| BASE_URL.to_string()),
            api_key,
        })
    }

    fn chain_url(&self, chain_id: u64) -> String {
        format!("{}/{chain_id}", self.base_url)
    }

    fn validate_chain(chain_id: u64) -> Result<(), String> {
        if SUPPORTED_CHAINS.contains(&chain_id) {
            Ok(())
        } else {
            Err(format!(
                "[1inch] unsupported chain_id {chain_id}. Supported: {SUPPORTED_CHAINS:?}"
            ))
        }
    }

    fn send_json<T: DeserializeOwned>(
        request: reqwest::blocking::RequestBuilder,
        operation: &str,
    ) -> Result<T, String> {
        let response = request
            .send()
            .map_err(|e| format!("[1inch] {operation} failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[1inch] {operation} failed: {status} {body}"));
        }

        serde_json::from_str::<T>(&body)
            .map_err(|e| format!("[1inch] {operation} decode failed: {e}; body: {body}"))
    }

    // ========================================================================
    // Endpoints
    // ========================================================================

    pub(crate) fn get_quote(
        &self,
        chain_id: u64,
        src: &str,
        dst: &str,
        amount: &str,
        protocols: Option<&str>,
    ) -> Result<OneInchQuoteResponse, String> {
        Self::validate_chain(chain_id)?;
        let mut request = self
            .http
            .get(format!("{}/quote", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key)
            .query(&[("src", src), ("dst", dst), ("amount", amount)]);
        if let Some(protocols) = protocols {
            request = request.query(&[("protocols", protocols)]);
        }
        Self::send_json(request, "quote")
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_swap(
        &self,
        chain_id: u64,
        src: &str,
        dst: &str,
        amount: &str,
        from: &str,
        slippage: f64,
        protocols: Option<&str>,
    ) -> Result<OneInchSwapResponse, String> {
        Self::validate_chain(chain_id)?;
        let mut request = self
            .http
            .get(format!("{}/swap", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key)
            .query(&[
                ("src", src),
                ("dst", dst),
                ("amount", amount),
                ("from", from),
                ("slippage", &slippage.to_string()),
            ]);
        if let Some(protocols) = protocols {
            request = request.query(&[("protocols", protocols)]);
        }
        Self::send_json(request, "swap")
    }

    pub(crate) fn get_approve_transaction(
        &self,
        chain_id: u64,
        token_address: &str,
        amount: Option<&str>,
    ) -> Result<OneInchTransaction, String> {
        Self::validate_chain(chain_id)?;
        let mut request = self
            .http
            .get(format!("{}/approve/transaction", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key)
            .query(&[("tokenAddress", token_address)]);
        if let Some(amount) = amount {
            request = request.query(&[("amount", amount)]);
        }
        Self::send_json(request, "approve/transaction")
    }

    pub(crate) fn get_allowance(
        &self,
        chain_id: u64,
        token_address: &str,
        wallet_address: &str,
    ) -> Result<OneInchAllowanceResponse, String> {
        Self::validate_chain(chain_id)?;
        let request = self
            .http
            .get(format!("{}/approve/allowance", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key)
            .query(&[
                ("tokenAddress", token_address),
                ("walletAddress", wallet_address),
            ]);
        Self::send_json(request, "approve/allowance")
    }

    pub(crate) fn get_liquidity_sources(
        &self,
        chain_id: u64,
    ) -> Result<OneInchLiquiditySourcesResponse, String> {
        Self::validate_chain(chain_id)?;
        let request = self
            .http
            .get(format!("{}/liquidity-sources", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key);
        Self::send_json(request, "liquidity-sources")
    }

    pub(crate) fn get_tokens(&self, chain_id: u64) -> Result<OneInchTokensResponse, String> {
        Self::validate_chain(chain_id)?;
        let request = self
            .http
            .get(format!("{}/tokens", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key);
        Self::send_json(request, "tokens")
    }
}

// ============================================================================
// Tool arg structs
// ============================================================================

pub(crate) struct GetOneInchQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOneInchQuoteArgs {
    /// Optional 1inch API key. Falls back to ONEINCH_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Chain ID (default: 1 for Ethereum). Supported: 1, 10, 56, 100, 137, 8453, 42161, 43114.
    pub(crate) chain_id: Option<u64>,
    /// Source token address (e.g. "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48" for USDC)
    pub(crate) src: String,
    /// Destination token address
    pub(crate) dst: String,
    /// Amount in minimal divisible units (wei for ETH, smallest unit for tokens)
    pub(crate) amount: String,
    /// Comma-separated list of protocols to use (optional, uses all if omitted)
    pub(crate) protocols: Option<String>,
}

pub(crate) struct GetOneInchSwap;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOneInchSwapArgs {
    /// Optional 1inch API key. Falls back to ONEINCH_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Chain ID (default: 1 for Ethereum). Supported: 1, 10, 56, 100, 137, 8453, 42161, 43114.
    pub(crate) chain_id: Option<u64>,
    /// Source token address
    pub(crate) src: String,
    /// Destination token address
    pub(crate) dst: String,
    /// Amount in minimal divisible units (wei for ETH, smallest unit for tokens)
    pub(crate) amount: String,
    /// Sender wallet address (the address that will execute the swap)
    pub(crate) from: String,
    /// Maximum acceptable slippage percentage (e.g. 1 for 1%)
    pub(crate) slippage: f64,
    /// Comma-separated list of protocols to use (optional, uses all if omitted)
    pub(crate) protocols: Option<String>,
}

pub(crate) struct GetOneInchApproveTransaction;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOneInchApproveTransactionArgs {
    /// Optional 1inch API key. Falls back to ONEINCH_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Chain ID (default: 1 for Ethereum). Supported: 1, 10, 56, 100, 137, 8453, 42161, 43114.
    pub(crate) chain_id: Option<u64>,
    /// Token contract address to approve
    pub(crate) token_address: String,
    /// Approval amount in minimal divisible units (optional; omit for unlimited approval)
    pub(crate) amount: Option<String>,
}

pub(crate) struct GetOneInchAllowance;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOneInchAllowanceArgs {
    /// Optional 1inch API key. Falls back to ONEINCH_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Chain ID (default: 1 for Ethereum). Supported: 1, 10, 56, 100, 137, 8453, 42161, 43114.
    pub(crate) chain_id: Option<u64>,
    /// Token contract address to check
    pub(crate) token_address: String,
    /// Wallet address to check allowance for
    pub(crate) wallet_address: String,
}

pub(crate) struct GetOneInchLiquiditySources;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOneInchLiquiditySourcesArgs {
    /// Optional 1inch API key. Falls back to ONEINCH_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Chain ID (default: 1 for Ethereum). Supported: 1, 10, 56, 100, 137, 8453, 42161, 43114.
    pub(crate) chain_id: Option<u64>,
}

pub(crate) struct GetOneInchTokens;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOneInchTokensArgs {
    /// Optional 1inch API key. Falls back to ONEINCH_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Chain ID (default: 1 for Ethereum). Supported: 1, 10, 56, 100, 137, 8453, 42161, 43114.
    pub(crate) chain_id: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    const WETH: &str = "0xc02aaa39b223fe8d0a0e5d3983027f52daa7aa3b";
    const DAI: &str = "0x6b175474e89094c44da98b954eedeac495271d0f";
    const CHAIN_ID: u64 = 1;
    const WALLET: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";

    fn client_or_skip() -> Option<OneInchClient> {
        std::env::var("ONEINCH_API_KEY")
            .ok()
            .map(|_| OneInchClient::new(None).expect("client should build"))
    }

    /// Swap 10k USDC for ETH at the best rate: quote -> allowance -> approve -> ready to swap.
    #[test]
    fn swap_usdc_to_eth_workflow() {
        let Some(c) = client_or_skip() else { return };

        let quote = c
            .get_quote(CHAIN_ID, USDC, WETH, "10000000000", None)
            .expect("quote should succeed");
        assert!(
            quote.dst_amount.as_deref().is_some_and(|s| !s.is_empty()),
            "dstAmount should not be empty"
        );

        let allowance = c
            .get_allowance(CHAIN_ID, USDC, WALLET)
            .expect("allowance check should succeed");
        assert!(
            allowance
                .allowance
                .as_deref()
                .is_some_and(|s| !s.is_empty()),
            "allowance must not be empty"
        );

        let approve_tx = c
            .get_approve_transaction(CHAIN_ID, USDC, None)
            .expect("approve transaction should succeed");
        assert!(
            approve_tx.data.is_some() || approve_tx.to.is_some(),
            "approve tx must contain transaction data"
        );
    }

    /// Discover tokens and liquidity sources, then quote a swap across multiple DEXs.
    #[test]
    fn check_liquidity_and_swap_workflow() {
        let Some(c) = client_or_skip() else { return };

        let tokens = c
            .get_tokens(CHAIN_ID)
            .expect("tokens endpoint should succeed");
        assert!(
            !tokens.tokens.is_empty(),
            "response must contain a tokens map"
        );

        let sources = c
            .get_liquidity_sources(CHAIN_ID)
            .expect("liquidity-sources should succeed");
        assert!(
            !sources.protocols.is_empty(),
            "response must contain protocols list"
        );

        let quote = c
            .get_quote(CHAIN_ID, DAI, WETH, "1000000000000000000000", None)
            .expect("quote should succeed");
        assert!(
            quote.dst_amount.as_deref().is_some_and(|s| !s.is_empty()),
            "dstAmount should not be empty"
        );
        assert!(
            quote.protocols.is_some(),
            "quote should contain routing protocols info"
        );
    }
}
