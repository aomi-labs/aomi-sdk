use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
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

    pub(crate) fn send_json(
        request: reqwest::blocking::RequestBuilder,
        operation: &str,
    ) -> Result<Value, String> {
        let response = request
            .send()
            .map_err(|e| format!("[1inch] {operation} failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[1inch] {operation} failed: {status} {body}"));
        }

        serde_json::from_str::<Value>(&body)
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
    ) -> Result<Value, String> {
        Self::validate_chain(chain_id)?;
        let mut request = self
            .http
            .get(format!("{}/quote", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key)
            .query(&[("src", src), ("dst", dst), ("amount", amount)]);
        if let Some(protocols) = protocols {
            request = request.query(&[("protocols", protocols)]);
        }
        let value = Self::send_json(request, "quote")?;
        Ok(with_source(value))
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
    ) -> Result<Value, String> {
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
        let value = Self::send_json(request, "swap")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_approve_transaction(
        &self,
        chain_id: u64,
        token_address: &str,
        amount: Option<&str>,
    ) -> Result<Value, String> {
        Self::validate_chain(chain_id)?;
        let mut request = self
            .http
            .get(format!("{}/approve/transaction", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key)
            .query(&[("tokenAddress", token_address)]);
        if let Some(amount) = amount {
            request = request.query(&[("amount", amount)]);
        }
        let value = Self::send_json(request, "approve/transaction")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_allowance(
        &self,
        chain_id: u64,
        token_address: &str,
        wallet_address: &str,
    ) -> Result<Value, String> {
        Self::validate_chain(chain_id)?;
        let request = self
            .http
            .get(format!("{}/approve/allowance", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key)
            .query(&[
                ("tokenAddress", token_address),
                ("walletAddress", wallet_address),
            ]);
        let value = Self::send_json(request, "approve/allowance")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_liquidity_sources(&self, chain_id: u64) -> Result<Value, String> {
        Self::validate_chain(chain_id)?;
        let request = self
            .http
            .get(format!("{}/liquidity-sources", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key);
        let value = Self::send_json(request, "liquidity-sources")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_tokens(&self, chain_id: u64) -> Result<Value, String> {
        Self::validate_chain(chain_id)?;
        let request = self
            .http
            .get(format!("{}/tokens", self.chain_url(chain_id)))
            .bearer_auth(&self.api_key);
        let value = Self::send_json(request, "tokens")?;
        Ok(with_source(value))
    }
}

// ============================================================================
// Shared helpers
// ============================================================================

pub(crate) fn with_source(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("1inch".to_string()));
            Value::Object(map)
        }
        other => json!({
            "source": "1inch",
            "data": other,
        }),
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
    /// Arbitrary wallet address for read-only allowance checks.
    const WALLET: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";

    fn has_api_key() -> bool {
        std::env::var("ONEINCH_API_KEY").is_ok()
    }

    fn client() -> OneInchClient {
        OneInchClient::new(None).expect("client should build (ONEINCH_API_KEY required)")
    }

    /// Swap 10k USDC for ETH at the best rate: quote -> allowance -> approve -> ready to swap.
    #[test]
    fn swap_usdc_to_eth_workflow() {
        if !has_api_key() {
            println!("ONEINCH_API_KEY not set — skipping 1inch swap_usdc_to_eth_workflow test");
            return;
        }
        let c = client();

        // 1. Get a quote for 10 000 USDC (6 decimals) -> WETH
        println!("[step 1] Requesting quote: 10000 USDC -> WETH on chain {CHAIN_ID}");
        let quote = c
            .get_quote(CHAIN_ID, USDC, WETH, "10000000000", None)
            .expect("quote should succeed");
        let dst_amount = quote
            .get("dstAmount")
            .expect("quote must contain dstAmount");
        println!("[step 1] Quote dstAmount (WETH wei): {dst_amount}");
        if let Some(protocols) = quote.get("protocols") {
            println!("[step 1] Routing protocols: {protocols}");
        }
        assert!(
            dst_amount.as_str().is_some() || dst_amount.is_number(),
            "dstAmount should be a string or number"
        );

        // 2. Check current allowance for USDC
        println!("[step 2] Checking USDC allowance for wallet {WALLET}");
        let allowance = c
            .get_allowance(CHAIN_ID, USDC, WALLET)
            .expect("allowance check should succeed");
        let allowance_value = allowance
            .get("allowance")
            .expect("response must contain allowance field");
        println!("[step 2] Current USDC allowance: {allowance_value}");
        assert!(
            allowance.get("allowance").is_some(),
            "response must contain allowance field"
        );

        // 3. Build an approve transaction for USDC (unlimited)
        println!("[step 3] Building unlimited approval TX for USDC");
        let approve_tx = c
            .get_approve_transaction(CHAIN_ID, USDC, None)
            .expect("approve transaction should succeed");
        if let Some(to) = approve_tx.get("to") {
            println!("[step 3] Approval TX to: {to}");
        }
        if let Some(data) = approve_tx.get("data") {
            let data_str = data.as_str().unwrap_or("");
            println!(
                "[step 3] Approval TX data (first 66 chars): {}",
                &data_str[..data_str.len().min(66)]
            );
        }
        if let Some(value) = approve_tx.get("value") {
            println!("[step 3] Approval TX value: {value}");
        }
        assert!(
            approve_tx.get("data").is_some() || approve_tx.get("to").is_some(),
            "approve tx must contain transaction data"
        );

        // 4. All three pieces are available — the full approval+swap flow can proceed.
        println!("[step 4] All workflow data collected — approval + swap flow is ready");
        assert!(
            quote.get("dstAmount").is_some()
                && allowance.get("allowance").is_some()
                && (approve_tx.get("data").is_some() || approve_tx.get("to").is_some()),
            "full approval + swap flow data must be available"
        );
    }

    /// Discover tokens and liquidity sources, then quote a swap across multiple DEXs.
    #[test]
    fn check_liquidity_and_swap_workflow() {
        if !has_api_key() {
            println!(
                "ONEINCH_API_KEY not set — skipping 1inch check_liquidity_and_swap_workflow test"
            );
            return;
        }
        let c = client();

        // 1. Fetch token list for Ethereum
        println!("[step 1] Fetching token list for chain {CHAIN_ID}");
        let tokens = c
            .get_tokens(CHAIN_ID)
            .expect("tokens endpoint should succeed");
        if let Some(tokens_map) = tokens.get("tokens").and_then(|t| t.as_object()) {
            println!("[step 1] Token count: {}", tokens_map.len());
            let sample: Vec<&String> = tokens_map.keys().take(5).collect();
            println!("[step 1] Sample token addresses: {sample:?}");
        }
        assert!(
            tokens.get("tokens").is_some(),
            "response must contain a tokens map"
        );

        // 2. Fetch liquidity sources (DEX list)
        println!("[step 2] Fetching liquidity sources for chain {CHAIN_ID}");
        let sources = c
            .get_liquidity_sources(CHAIN_ID)
            .expect("liquidity-sources should succeed");
        if let Some(protocols) = sources.get("protocols").and_then(|p| p.as_array()) {
            println!("[step 2] Liquidity source count: {}", protocols.len());
            let sample_names: Vec<&str> = protocols
                .iter()
                .take(5)
                .filter_map(|p| p.get("id").and_then(|id| id.as_str()))
                .collect();
            println!("[step 2] Sample liquidity sources: {sample_names:?}");
        }
        assert!(
            sources.get("protocols").is_some(),
            "response must contain protocols list"
        );

        // 3. Quote a DAI -> WETH swap (1000 DAI, 18 decimals)
        println!("[step 3] Requesting quote: 1000 DAI -> WETH on chain {CHAIN_ID}");
        let quote = c
            .get_quote(CHAIN_ID, DAI, WETH, "1000000000000000000000", None)
            .expect("quote should succeed");
        let dst_amount = quote
            .get("dstAmount")
            .expect("quote must contain dstAmount");
        println!("[step 3] Quote dstAmount (WETH wei): {dst_amount}");
        if let Some(src_amount) = quote.get("srcAmount") {
            println!("[step 3] Quote srcAmount (DAI wei): {src_amount}");
        }
        assert!(
            dst_amount.as_str().is_some() || dst_amount.is_number(),
            "dstAmount should be a string or number"
        );
        // The quote should include routing/protocol info
        if let Some(protocols) = quote.get("protocols") {
            println!("[step 3] Routing protocols used: {protocols}");
        }
        assert!(
            quote.get("protocols").is_some(),
            "quote should contain routing protocols info"
        );

        // 4. We now have tokens, liquidity sources, and a quote — enough to build the swap TX.
        println!("[step 4] All workflow data collected — ready to build swap TX");
        assert!(
            tokens.get("tokens").is_some()
                && sources.get("protocols").is_some()
                && quote.get("dstAmount").is_some(),
            "must have enough data to build the swap transaction"
        );
    }
}
