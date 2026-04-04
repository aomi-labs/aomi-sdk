use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct ZeroxApp;

// ============================================================================
// 0x HTTP Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_ZEROX_ENDPOINT: &str = "https://api.0x.org";

#[derive(Clone)]
pub(crate) struct ZeroxClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) zerox_endpoint: String,
    pub(crate) zerox_api_key: String,
}

impl ZeroxClient {
    pub(crate) fn new() -> Result<Self, String> {
        let api_key =
            std::env::var("ZEROX_API_KEY").map_err(|_| "[0x] missing ZEROX_API_KEY".to_string())?;
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[0x] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            zerox_endpoint: std::env::var("ZEROX_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_ZEROX_ENDPOINT.to_string()),
            zerox_api_key: api_key,
        })
    }

    pub(crate) fn send_json(
        request: reqwest::blocking::RequestBuilder,
        operation: &str,
    ) -> Result<Value, String> {
        let response = request
            .send()
            .map_err(|e| format!("[0x] {operation} request failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[0x] {operation} request failed: {status} {body}"));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[0x] {operation} decode failed: {e}; body: {body}"))
    }

    pub(crate) fn get_quote(
        &self,
        chain: &str,
        from_token: &str,
        to_token: &str,
        amount: f64,
        sender_address: Option<&str>,
        slippage: Option<f64>,
    ) -> Result<Value, String> {
        let (chain_name, chain_id) = get_chain_info(chain)?;
        let from_addr = get_token_address(chain_name, from_token)?;
        let to_addr = get_token_address(chain_name, to_token)?;
        let decimals = get_token_decimals(chain_name, from_token);
        let amount_wei = amount_to_base_units(amount, decimals)?;

        let mut request = self
            .http
            .get(format!("{}/swap/permit2/price", self.zerox_endpoint))
            .query(&[
                ("chainId", chain_id.to_string()),
                ("sellToken", from_addr),
                ("buyToken", to_addr),
                ("sellAmount", amount_wei),
                ("slippagePercentage", slippage.unwrap_or(0.01).to_string()),
            ])
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");
        if let Some(sender_address) = sender_address {
            request = request.query(&[("taker", sender_address)]);
        }

        let value = Self::send_json(request, "quote")?;
        Ok(with_source(value))
    }

    pub(crate) fn place_order(
        &self,
        chain: &str,
        sell_token: &str,
        buy_token: &str,
        amount: f64,
        sender_address: &str,
        slippage: Option<f64>,
    ) -> Result<Value, String> {
        let (chain_name, chain_id) = get_chain_info(chain)?;
        let sell_addr = get_token_address(chain_name, sell_token)?;
        let buy_addr = get_token_address(chain_name, buy_token)?;
        let decimals = get_token_decimals(chain_name, sell_token);
        let amount_wei = amount_to_base_units(amount, decimals)?;

        let response = self
            .http
            .get(format!(
                "{}/swap/allowance-holder/quote",
                self.zerox_endpoint
            ))
            .query(&[
                ("chainId", chain_id.to_string()),
                ("sellToken", sell_addr),
                ("buyToken", buy_addr),
                ("sellAmount", amount_wei),
                ("taker", sender_address.to_string()),
                ("slippagePercentage", slippage.unwrap_or(0.01).to_string()),
            ])
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");

        let value = Self::send_json(response, "place order")?;
        Ok(with_source(value))
    }

    // ========================================================================
    // High Priority endpoints
    // ========================================================================

    pub(crate) fn get_swap_chains(&self) -> Result<Value, String> {
        let request = self
            .http
            .get(format!("{}/swap/chains", self.zerox_endpoint))
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");
        let value = Self::send_json(request, "swap chains")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_allowance_holder_price(
        &self,
        chain: &str,
        sell_token: &str,
        buy_token: &str,
        amount: f64,
        sender_address: Option<&str>,
        slippage: Option<f64>,
    ) -> Result<Value, String> {
        let (chain_name, chain_id) = get_chain_info(chain)?;
        let sell_addr = get_token_address(chain_name, sell_token)?;
        let buy_addr = get_token_address(chain_name, buy_token)?;
        let decimals = get_token_decimals(chain_name, sell_token);
        let amount_wei = amount_to_base_units(amount, decimals)?;

        let mut request = self
            .http
            .get(format!(
                "{}/swap/allowance-holder/price",
                self.zerox_endpoint
            ))
            .query(&[
                ("chainId", chain_id.to_string()),
                ("sellToken", sell_addr),
                ("buyToken", buy_addr),
                ("sellAmount", amount_wei),
                ("slippagePercentage", slippage.unwrap_or(0.01).to_string()),
            ])
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");
        if let Some(sender) = sender_address {
            request = request.query(&[("taker", sender)]);
        }

        let value = Self::send_json(request, "allowance-holder price")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_liquidity_sources(&self, chain: &str) -> Result<Value, String> {
        let (_chain_name, chain_id) = get_chain_info(chain)?;

        let request = self
            .http
            .get(format!("{}/sources", self.zerox_endpoint))
            .query(&[("chainId", chain_id.to_string())])
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");
        let value = Self::send_json(request, "liquidity sources")?;
        Ok(with_source(value))
    }

    // ========================================================================
    // Gasless endpoints
    // ========================================================================

    pub(crate) fn get_gasless_price(
        &self,
        chain: &str,
        sell_token: &str,
        buy_token: &str,
        amount: f64,
        sender_address: Option<&str>,
        slippage: Option<f64>,
    ) -> Result<Value, String> {
        let (chain_name, chain_id) = get_chain_info(chain)?;
        let sell_addr = get_token_address(chain_name, sell_token)?;
        let buy_addr = get_token_address(chain_name, buy_token)?;
        let decimals = get_token_decimals(chain_name, sell_token);
        let amount_wei = amount_to_base_units(amount, decimals)?;

        let mut request = self
            .http
            .get(format!("{}/gasless/price", self.zerox_endpoint))
            .query(&[
                ("chainId", chain_id.to_string()),
                ("sellToken", sell_addr),
                ("buyToken", buy_addr),
                ("sellAmount", amount_wei),
                ("slippagePercentage", slippage.unwrap_or(0.01).to_string()),
            ])
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");
        if let Some(sender) = sender_address {
            request = request.query(&[("taker", sender)]);
        }

        let value = Self::send_json(request, "gasless price")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_gasless_quote(
        &self,
        chain: &str,
        sell_token: &str,
        buy_token: &str,
        amount: f64,
        sender_address: &str,
        slippage: Option<f64>,
    ) -> Result<Value, String> {
        let (chain_name, chain_id) = get_chain_info(chain)?;
        let sell_addr = get_token_address(chain_name, sell_token)?;
        let buy_addr = get_token_address(chain_name, buy_token)?;
        let decimals = get_token_decimals(chain_name, sell_token);
        let amount_wei = amount_to_base_units(amount, decimals)?;

        let request = self
            .http
            .get(format!("{}/gasless/quote", self.zerox_endpoint))
            .query(&[
                ("chainId", chain_id.to_string()),
                ("sellToken", sell_addr),
                ("buyToken", buy_addr),
                ("sellAmount", amount_wei),
                ("taker", sender_address.to_string()),
                ("slippagePercentage", slippage.unwrap_or(0.01).to_string()),
            ])
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");

        let value = Self::send_json(request, "gasless quote")?;
        Ok(with_source(value))
    }

    pub(crate) fn submit_gasless_swap(
        &self,
        chain_id: u64,
        trade: &Value,
        approval: Option<&Value>,
    ) -> Result<Value, String> {
        let mut body = json!({
            "chainId": chain_id,
            "trade": trade,
        });
        if let Some(approval) = approval {
            body["approval"] = approval.clone();
        }

        let request = self
            .http
            .post(format!("{}/gasless/submit", self.zerox_endpoint))
            .json(&body)
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");

        let value = Self::send_json(request, "gasless submit")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_gasless_status(
        &self,
        trade_hash: &str,
        chain_id: u64,
    ) -> Result<Value, String> {
        let request = self
            .http
            .get(format!(
                "{}/gasless/status/{}",
                self.zerox_endpoint, trade_hash
            ))
            .query(&[("chainId", chain_id.to_string())])
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");

        let value = Self::send_json(request, "gasless status")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_gasless_chains(&self) -> Result<Value, String> {
        let request = self
            .http
            .get(format!("{}/gasless/chains", self.zerox_endpoint))
            .header("0x-api-key", &self.zerox_api_key)
            .header("0x-version", "v2");
        let value = Self::send_json(request, "gasless chains")?;
        Ok(with_source(value))
    }
}

// ============================================================================
// Shared helpers
// ============================================================================

pub(crate) fn with_source(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("0x".to_string()));
            Value::Object(map)
        }
        other => json!({
            "source": "0x",
            "data": other,
        }),
    }
}

pub(crate) fn amount_to_base_units(amount: f64, decimals: u8) -> Result<String, String> {
    if !amount.is_finite() || amount < 0.0 {
        return Err("amount must be a finite non-negative number".to_string());
    }
    let scaled = amount * 10f64.powi(decimals as i32);
    if scaled > (u128::MAX as f64) {
        return Err("amount is too large to convert to base units".to_string());
    }
    Ok((scaled.round() as u128).to_string())
}

pub(crate) fn get_chain_info(chain: &str) -> Result<(&'static str, u64), String> {
    match chain.to_lowercase().as_str() {
        "ethereum" | "eth" | "mainnet" => Ok(("ethereum", 1)),
        "polygon" | "matic" => Ok(("polygon", 137)),
        "arbitrum" | "arb" => Ok(("arbitrum", 42161)),
        "optimism" | "op" => Ok(("optimism", 10)),
        "base" => Ok(("base", 8453)),
        "bsc" | "binance" => Ok(("bsc", 56)),
        "avalanche" | "avax" => Ok(("avalanche", 43114)),
        _ => Err(format!("[0x] unsupported chain: {chain}")),
    }
}

pub(crate) fn is_hex_address(token: &str) -> bool {
    token.len() == 42
        && token.starts_with("0x")
        && token[2..].chars().all(|c| c.is_ascii_hexdigit())
}

pub(crate) fn get_token_address(chain: &str, token: &str) -> Result<String, String> {
    let native = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";
    let token_lower = token.to_lowercase();

    if token_lower == native.to_lowercase() {
        return Ok(native.to_string());
    }
    if is_hex_address(token) {
        return Ok(token.to_string());
    }

    match (chain, token_lower.as_str()) {
        (_, "eth") | (_, "matic") | (_, "bnb") | (_, "avax") => Ok(native.to_string()),
        ("ethereum", "usdc") => Ok("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string()),
        ("ethereum", "usdt") => Ok("0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string()),
        ("ethereum", "dai") => Ok("0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string()),
        ("ethereum", "weth") => Ok("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
        ("ethereum", "wbtc") => Ok("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string()),
        ("ethereum", "uni") => Ok("0x1f9840a85d5aF5bf1D1762F925BDADdC4201F984".to_string()),
        ("ethereum", "aave") => Ok("0x7Fc66500c84A76Ad7e9c93437bFc5Ac33E2DdAE9".to_string()),
        ("ethereum", "link") => Ok("0x514910771AF9Ca656af840dff83E8264EcF986CA".to_string()),
        ("ethereum", "mkr") => Ok("0x9f8F72aA9304c8B593d555F12ef6589cC3A579A2".to_string()),
        ("ethereum", "crv") => Ok("0xD533a949740bb3306d119CC777fa900bA034cd52".to_string()),
        ("ethereum", "ldo") => Ok("0x5A98FcBEA516Cf06857215779Fd812CA3beF1B32".to_string()),
        ("arbitrum", "usdc") => Ok("0xaf88d065e77c8cC2239327C5EDb3A432268e5831".to_string()),
        ("arbitrum", "usdt") => Ok("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9".to_string()),
        ("arbitrum", "weth") => Ok("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string()),
        ("base", "usdc") => Ok("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()),
        ("base", "weth") => Ok("0x4200000000000000000000000000000000000006".to_string()),
        ("polygon", "usdc") => Ok("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359".to_string()),
        ("polygon", "usdt") => Ok("0xc2132D05D31c914a87C6611C10748AEb04B58e8F".to_string()),
        ("polygon", "weth") => Ok("0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".to_string()),
        _ => Err(format!("[0x] unknown token {token} on chain {chain}")),
    }
}

pub(crate) fn get_token_decimals(chain: &str, token: &str) -> u8 {
    let token_lower = token.to_lowercase();

    if is_hex_address(token) {
        return match (chain, token_lower.as_str()) {
            ("ethereum", "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48") => 6,
            ("ethereum", "0xdac17f958d2ee523a2206206994597c13d831ec7") => 6,
            ("arbitrum", "0xaf88d065e77c8cc2239327c5edb3a432268e5831") => 6,
            ("arbitrum", "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9") => 6,
            ("polygon", "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359") => 6,
            ("polygon", "0xc2132d05d31c914a87c6611c10748aeb04b58e8f") => 6,
            ("base", "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913") => 6,
            ("ethereum", "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599") => 8,
            _ => 18,
        };
    }

    match token_lower.as_str() {
        "usdc" | "usdt" => 6,
        "wbtc" => 8,
        _ => 18,
    }
}

// ============================================================================
// Tool arg structs
// ============================================================================

pub(crate) struct GetZeroxSwapQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetZeroxSwapQuoteArgs {
    /// Source chain
    pub(crate) chain: String,
    /// Sell token symbol or address
    pub(crate) sell_token: String,
    /// Buy token symbol or address
    pub(crate) buy_token: String,
    /// Amount to swap (human-readable units)
    pub(crate) amount: f64,
    /// Sender/taker address (optional for price quotes)
    pub(crate) sender_address: Option<String>,
    /// Slippage tolerance as decimal (0.005 = 0.5%)
    pub(crate) slippage: Option<f64>,
}

pub(crate) struct PlaceZeroxOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceZeroxOrderArgs {
    /// Source chain
    pub(crate) chain: String,
    /// Sell token symbol or address
    pub(crate) sell_token: String,
    /// Buy token symbol or address
    pub(crate) buy_token: String,
    /// Sell amount (human-readable units)
    pub(crate) amount: f64,
    /// Sender/taker wallet address (required)
    pub(crate) sender_address: String,
    /// Slippage tolerance as decimal (0.005 = 0.5%)
    pub(crate) slippage: Option<f64>,
}

// ---- High Priority tools ----

pub(crate) struct GetZeroxSwapChains;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetZeroxSwapChainsArgs {}

pub(crate) struct GetZeroxAllowanceHolderPrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetZeroxAllowanceHolderPriceArgs {
    /// Source chain (e.g. "ethereum", "polygon")
    pub(crate) chain: String,
    /// Sell token symbol or address
    pub(crate) sell_token: String,
    /// Buy token symbol or address
    pub(crate) buy_token: String,
    /// Amount to sell (human-readable units)
    pub(crate) amount: f64,
    /// Sender/taker address (optional for price discovery)
    pub(crate) sender_address: Option<String>,
    /// Slippage tolerance as decimal (0.005 = 0.5%)
    pub(crate) slippage: Option<f64>,
}

pub(crate) struct GetZeroxLiquiditySources;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetZeroxLiquiditySourcesArgs {
    /// Chain name (e.g. "ethereum", "polygon")
    pub(crate) chain: String,
}

// ---- Gasless tools ----

pub(crate) struct GetZeroxGaslessPrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetZeroxGaslessPriceArgs {
    /// Source chain (e.g. "ethereum", "polygon")
    pub(crate) chain: String,
    /// Sell token symbol or address (must be ERC-20, not native)
    pub(crate) sell_token: String,
    /// Buy token symbol or address
    pub(crate) buy_token: String,
    /// Amount to sell (human-readable units)
    pub(crate) amount: f64,
    /// Sender/taker address (optional)
    pub(crate) sender_address: Option<String>,
    /// Slippage tolerance as decimal (0.005 = 0.5%)
    pub(crate) slippage: Option<f64>,
}

pub(crate) struct GetZeroxGaslessQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetZeroxGaslessQuoteArgs {
    /// Source chain (e.g. "ethereum", "polygon")
    pub(crate) chain: String,
    /// Sell token symbol or address (must be ERC-20, not native)
    pub(crate) sell_token: String,
    /// Buy token symbol or address
    pub(crate) buy_token: String,
    /// Amount to sell (human-readable units)
    pub(crate) amount: f64,
    /// Sender/taker wallet address (required)
    pub(crate) sender_address: String,
    /// Slippage tolerance as decimal (0.005 = 0.5%)
    pub(crate) slippage: Option<f64>,
}

pub(crate) struct SubmitZeroxGaslessSwap;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SubmitZeroxGaslessSwapArgs {
    /// Chain ID (numeric, e.g. 1 for Ethereum)
    pub(crate) chain_id: u64,
    /// Signed trade object from gasless quote
    pub(crate) trade: Value,
    /// Signed approval object (if the gasless quote required one)
    pub(crate) approval: Option<Value>,
}

pub(crate) struct GetZeroxGaslessStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetZeroxGaslessStatusArgs {
    /// Trade hash returned by submit_zerox_gasless_swap
    pub(crate) trade_hash: String,
    /// Chain ID (numeric, e.g. 1 for Ethereum)
    pub(crate) chain_id: u64,
}

pub(crate) struct GetZeroxGaslessChains;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetZeroxGaslessChainsArgs {}

#[cfg(test)]
mod tests {
    use super::*;

    fn has_api_key() -> bool {
        std::env::var("ZEROX_API_KEY").is_ok()
    }

    fn client() -> ZeroxClient {
        ZeroxClient::new().expect("client should build (ZEROX_API_KEY required)")
    }

    #[test]
    fn skip_without_api_key() {
        if !has_api_key() {
            eprintln!("ZEROX_API_KEY not set — skipping 0x API tests");
            return;
        }
    }

    #[test]
    fn swap_chains_smoke() {
        if !has_api_key() {
            return;
        }
        let res = client().get_swap_chains().expect("should get swap chains");
        assert_eq!(res.get("source").and_then(Value::as_str), Some("0x"));
    }

    #[test]
    fn liquidity_sources_smoke() {
        if !has_api_key() {
            return;
        }
        let res = client()
            .get_liquidity_sources("ethereum")
            .expect("should get liquidity sources");
        assert_eq!(res.get("source").and_then(Value::as_str), Some("0x"));
    }

    #[test]
    fn gasless_chains_smoke() {
        if !has_api_key() {
            return;
        }
        let res = client()
            .get_gasless_chains()
            .expect("should get gasless chains");
        assert_eq!(res.get("source").and_then(Value::as_str), Some("0x"));
    }

    #[test]
    fn permit2_price_smoke() {
        if !has_api_key() {
            return;
        }
        let res = client()
            .get_quote("ethereum", "usdc", "weth", 1000.0, None, None)
            .expect("should get permit2 price for 1000 USDC -> WETH");
        assert_eq!(res.get("source").and_then(Value::as_str), Some("0x"));
    }

    #[test]
    fn allowance_holder_price_smoke() {
        if !has_api_key() {
            return;
        }
        let res = client()
            .get_allowance_holder_price("ethereum", "usdc", "weth", 1000.0, None, None)
            .expect("should get allowance-holder price for 1000 USDC -> WETH");
        assert_eq!(res.get("source").and_then(Value::as_str), Some("0x"));
    }

    // Unit tests for helpers (no API key needed)

    #[test]
    fn amount_to_base_units_smoke() {
        assert_eq!(
            amount_to_base_units(1.0, 18).unwrap(),
            "1000000000000000000"
        );
        assert_eq!(amount_to_base_units(100.0, 6).unwrap(), "100000000");
        assert!(amount_to_base_units(-1.0, 18).is_err());
    }

    #[test]
    fn get_chain_info_smoke() {
        assert_eq!(get_chain_info("ethereum").unwrap(), ("ethereum", 1));
        assert_eq!(get_chain_info("base").unwrap(), ("base", 8453));
        assert!(get_chain_info("foobar").is_err());
    }

    #[test]
    fn token_address_smoke() {
        assert_eq!(
            get_token_address("ethereum", "usdc").unwrap(),
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        );
        assert!(get_token_address("ethereum", "unknown_xyz").is_err());
    }

    #[test]
    fn token_decimals_smoke() {
        assert_eq!(get_token_decimals("ethereum", "usdc"), 6);
        assert_eq!(get_token_decimals("ethereum", "eth"), 18);
        assert_eq!(get_token_decimals("ethereum", "wbtc"), 8);
    }

    #[test]
    fn is_hex_address_smoke() {
        assert!(is_hex_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"));
        assert!(!is_hex_address("usdc"));
        assert!(!is_hex_address("0x123"));
    }
}
