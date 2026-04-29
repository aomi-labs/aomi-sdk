use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::time::Duration;

pub(crate) use crate::types::*;

#[derive(Clone, Default)]
pub(crate) struct CowApp;

// ============================================================================
// CoW HTTP Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_COW_ENDPOINT: &str = "https://api.cow.fi/mainnet";

#[derive(Clone)]
pub(crate) struct CowClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) cow_endpoint: String,
    pub(crate) cow_api_key: Option<String>,
}

impl CowClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[cow] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            cow_endpoint: std::env::var("COW_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_COW_ENDPOINT.to_string()),
            cow_api_key: std::env::var("COW_API_KEY").ok(),
        })
    }

    fn send_json<T: DeserializeOwned>(
        request: reqwest::blocking::RequestBuilder,
        operation: &str,
    ) -> Result<T, String> {
        let response = request
            .send()
            .map_err(|e| format!("[cow] {operation} request failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[cow] {operation} request failed: {status} {body}"));
        }

        serde_json::from_str::<T>(&body)
            .map_err(|e| format!("[cow] {operation} decode failed: {e}; body: {body}"))
    }

    fn authed(&self, mut request: reqwest::blocking::RequestBuilder) -> reqwest::blocking::RequestBuilder {
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        request
    }

    pub(crate) fn cow_api_base_for_chain(&self, chain: &str) -> Result<String, String> {
        let path = match chain.to_lowercase().as_str() {
            "ethereum" | "eth" | "mainnet" => "mainnet",
            "gnosis" | "xdai" => "xdai",
            "arbitrum" | "arb" | "arbitrum_one" => "arbitrum_one",
            "base" => "base",
            "polygon" | "matic" => "polygon",
            "avalanche" | "avax" => "avalanche",
            "bnb" | "bsc" => "bsc",
            "sepolia" => "sepolia",
            other => return Err(format!("[cow] unsupported chain for orderbook: {other}")),
        };

        let endpoint = self.cow_endpoint.trim_end_matches('/');
        if let Some((prefix, _)) = endpoint.rsplit_once('/') {
            return Ok(format!("{prefix}/{path}/api/v1"));
        }
        Ok(format!("{endpoint}/{path}/api/v1"))
    }

    pub(crate) fn get_quote<B: Serialize>(
        &self,
        chain: &str,
        payload: &B,
    ) -> Result<CowQuote, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let request = self.authed(self.http.post(format!("{base}/quote")).json(&payload));
        Self::send_json(request, "quote")
    }

    pub(crate) fn place_order(&self, chain: &str, payload: Value) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let request = self.authed(self.http.post(format!("{base}/orders")).json(&payload));
        Self::send_json(request, "post order")
    }

    pub(crate) fn get_order(&self, chain: &str, uid: &str) -> Result<CowOrder, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let request = self.authed(self.http.get(format!("{base}/orders/{uid}")));
        Self::send_json(request, "get order")
    }

    pub(crate) fn get_order_status(
        &self,
        chain: &str,
        uid: &str,
    ) -> Result<CowOrderStatus, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let request = self.authed(self.http.get(format!("{base}/orders/{uid}/status")));
        Self::send_json(request, "get order status")
    }

    pub(crate) fn get_user_orders(
        &self,
        chain: &str,
        owner: &str,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<CowOrder>, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self.http.get(format!("{base}/account/{owner}/orders"));
        if let Some(offset) = offset {
            request = request.query(&[("offset", offset)]);
        }
        if let Some(limit) = limit {
            request = request.query(&[("limit", limit)]);
        }
        Self::send_json(self.authed(request), "get user orders")
    }

    pub(crate) fn cancel_orders<B: Serialize>(
        &self,
        chain: &str,
        payload: &B,
    ) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let request = self.authed(self.http.delete(format!("{base}/orders")).json(&payload));
        Self::send_json(request, "cancel orders")
    }

    pub(crate) fn get_trades(
        &self,
        chain: &str,
        owner: Option<&str>,
        order_uid: Option<&str>,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<CowTrade>, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        // Trades endpoint is v2 — replace /v1 suffix with /v2
        let base_v2 = base
            .strip_suffix("/v1")
            .map(|p| format!("{p}/v2"))
            .unwrap_or(base);
        let mut request = self.http.get(format!("{base_v2}/trades"));
        if let Some(owner) = owner {
            request = request.query(&[("owner", owner)]);
        }
        if let Some(order_uid) = order_uid {
            request = request.query(&[("orderUid", order_uid)]);
        }
        if let Some(offset) = offset {
            request = request.query(&[("offset", offset)]);
        }
        if let Some(limit) = limit {
            request = request.query(&[("limit", limit)]);
        }
        Self::send_json(self.authed(request), "get trades")
    }

    pub(crate) fn get_native_price(
        &self,
        chain: &str,
        token_address: &str,
    ) -> Result<CowNativePrice, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let request =
            self.authed(self.http.get(format!("{base}/token/{token_address}/native_price")));
        Self::send_json(request, "get native price")
    }

    pub(crate) fn get_orders_by_tx(
        &self,
        chain: &str,
        tx_hash: &str,
    ) -> Result<Vec<CowOrder>, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let request = self.authed(self.http.get(format!("{base}/transactions/{tx_hash}/orders")));
        Self::send_json(request, "get orders by tx")
    }

    pub(crate) fn debug_order(&self, chain: &str, uid: &str) -> Result<CowOrder, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let request = self.authed(self.http.get(format!("{base}/debug/order/{uid}")));
        Self::send_json(request, "debug order")
    }
}

// ============================================================================
// Shared helpers
// ============================================================================

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
        "gnosis" | "xdai" => Ok(("gnosis", 100)),
        _ => Err(format!("[cow] unsupported chain: {chain}")),
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
        ("gnosis", "usdc") => Ok("0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83".to_string()),
        ("gnosis", "wxdai") => Ok("0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d".to_string()),
        _ => Err(format!("[cow] unknown token {token} on chain {chain}")),
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
            ("gnosis", "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83") => 6,
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

pub(crate) struct GetCowSwapQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowSwapQuoteArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Sell token symbol or address
    pub(crate) sell_token: String,
    /// Buy token symbol or address
    pub(crate) buy_token: String,
    /// Amount to swap (human-readable units)
    pub(crate) amount: f64,
    /// Sender/from address
    pub(crate) sender_address: String,
    /// Receiver address (optional, defaults to sender)
    pub(crate) receiver_address: Option<String>,
    /// Order side: "sell" or "buy" (default: "sell")
    pub(crate) order_side: Option<String>,
    /// Quote validity timestamp (optional)
    pub(crate) valid_to: Option<u64>,
    /// Allow partial fills (optional)
    pub(crate) partially_fillable: Option<bool>,
    /// Signing scheme: eip712, ethsign (optional)
    pub(crate) signing_scheme: Option<String>,
    /// Slippage tolerance as decimal (0.005 = 0.5%)
    pub(crate) slippage: Option<f64>,
}

pub(crate) struct PlaceCowOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceCowOrderArgs {
    /// CoW chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Signed order payload to submit to CoW /orders endpoint
    pub(crate) signed_order: Value,
}

pub(crate) struct GetCowOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowOrderArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Order UID returned when the order was placed
    pub(crate) order_uid: String,
}

pub(crate) struct GetCowOrderStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowOrderStatusArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Order UID returned when the order was placed
    pub(crate) order_uid: String,
}

pub(crate) struct GetCowUserOrders;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowUserOrdersArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Owner (wallet) address
    pub(crate) owner_address: String,
    /// Pagination offset
    pub(crate) offset: Option<u32>,
    /// Maximum number of results to return
    pub(crate) limit: Option<u32>,
}

pub(crate) struct CancelCowOrders;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CancelCowOrdersArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// List of order UIDs to cancel
    pub(crate) order_uids: Vec<String>,
    /// Cancellation signature from the order owner
    pub(crate) signature: String,
    /// Signing scheme used: "eip712" or "ethsign"
    pub(crate) signing_scheme: String,
}

pub(crate) struct GetCowTrades;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowTradesArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Owner address (provide exactly one of owner or order_uid)
    pub(crate) owner: Option<String>,
    /// Order UID (provide exactly one of owner or order_uid)
    pub(crate) order_uid: Option<String>,
    /// Pagination offset
    pub(crate) offset: Option<u32>,
    /// Maximum number of results to return
    pub(crate) limit: Option<u32>,
}

pub(crate) struct GetCowNativePrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowNativePriceArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Token contract address (0x...)
    pub(crate) token_address: String,
}

pub(crate) struct GetCowOrdersByTx;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowOrdersByTxArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Transaction hash (0x...)
    pub(crate) tx_hash: String,
}

pub(crate) struct DebugCowOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct DebugCowOrderArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Order UID to debug
    pub(crate) order_uid: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> CowClient {
        CowClient::new().expect("client should build")
    }

    #[test]
    fn cow_api_base_for_chain_smoke() {
        let c = client();
        assert!(
            c.cow_api_base_for_chain("ethereum")
                .unwrap()
                .contains("mainnet")
        );
        assert!(c.cow_api_base_for_chain("gnosis").unwrap().contains("xdai"));
        assert!(
            c.cow_api_base_for_chain("arbitrum")
                .unwrap()
                .contains("arbitrum_one")
        );
        assert!(c.cow_api_base_for_chain("base").unwrap().contains("base"));
        assert!(
            c.cow_api_base_for_chain("polygon")
                .unwrap()
                .contains("polygon")
        );
        assert!(c.cow_api_base_for_chain("foobar").is_err());
    }

    #[test]
    fn native_price_weth_smoke() {
        let res = client()
            .get_native_price("ethereum", "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
            .expect("should get WETH native price");
        assert!(res.price.is_some(), "should have price key");
    }

    #[test]
    fn native_price_usdc_smoke() {
        let res = client()
            .get_native_price("ethereum", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
            .expect("should get USDC native price");
        assert!(res.price.is_some(), "should have numeric price");
    }

    #[test]
    fn user_orders_no_orders_smoke() {
        // Random address that likely has no orders — empty array or error are both acceptable
        let _ = client().get_user_orders(
            "ethereum",
            "0x0000000000000000000000000000000000000001",
            None,
            Some(1),
        );
    }

    #[test]
    fn get_trades_v2_requires_filter() {
        // v2 trades may require a filter — either works or errors out
        let _ = client().get_trades("ethereum", None, None, None, Some(1));
    }

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
        assert_eq!(get_chain_info("gnosis").unwrap(), ("gnosis", 100));
        assert!(get_chain_info("unknown_chain").is_err());
    }

    #[test]
    fn token_address_known() {
        assert_eq!(
            get_token_address("ethereum", "usdc").unwrap(),
            "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"
        );
        assert_eq!(
            get_token_address("ethereum", "eth").unwrap(),
            "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE"
        );
        assert!(get_token_address("ethereum", "unknown_token_xyz").is_err());
    }

    #[test]
    fn token_decimals_known() {
        assert_eq!(get_token_decimals("ethereum", "usdc"), 6);
        assert_eq!(get_token_decimals("ethereum", "wbtc"), 8);
        assert_eq!(get_token_decimals("ethereum", "eth"), 18);
    }
}
