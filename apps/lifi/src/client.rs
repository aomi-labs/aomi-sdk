use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::time::Duration;

use crate::types::{
    BridgeQuoteResponse, ChainsQuery, ConnectionsQuery, GasSuggestionQuery, LifiQuoteResponse,
    PreparedOrder, PreparedTransaction, QuoteQuery, ReverseQuoteQuery, RouteOptions, RouteRequest,
    StatusQuery, TokenQuery, TokensQuery, ToolsQuery,
};

#[derive(Clone, Default)]
pub(crate) struct LifiApp;

// ============================================================================
// LI.FI HTTP Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_LIFI_ENDPOINT: &str = "https://li.quest";

#[derive(Clone)]
pub(crate) struct LifiClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) lifi_endpoint: String,
    pub(crate) lifi_api_key: Option<String>,
}

impl LifiClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[lifi] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            lifi_endpoint: std::env::var("LIFI_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_LIFI_ENDPOINT.to_string()),
            lifi_api_key: std::env::var("LIFI_API_KEY").ok(),
        })
    }

    fn send_json<T: DeserializeOwned>(
        request: reqwest::blocking::RequestBuilder,
        operation: &str,
    ) -> Result<T, String> {
        let response = request
            .send()
            .map_err(|e| format!("[lifi] {operation} request failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "[lifi] {operation} request failed: {status} {body}"
            ));
        }

        serde_json::from_str::<T>(&body)
            .map_err(|e| format!("[lifi] {operation} decode failed: {e}; body: {body}"))
    }

    fn authed(
        &self,
        mut request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        if let Some(api_key) = self.lifi_api_key.as_ref() {
            request = request.header("x-lifi-api-key", api_key);
        }
        request
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_quote(
        &self,
        from_chain: &str,
        to_chain: &str,
        from_token: &str,
        to_token: &str,
        from_amount: &str,
        from_address: &str,
        to_address: Option<&str>,
    ) -> Result<Value, String> {
        let from_chain_id = normalize_lifi_chain_id(from_chain)?;
        let to_chain_id = normalize_lifi_chain_id(to_chain)?;
        let query = QuoteQuery {
            from_chain: &from_chain_id,
            to_chain: &to_chain_id,
            from_token,
            to_token,
            from_amount,
            from_address,
            to_address,
            slippage: None,
        };

        let request = self.authed(
            self.http
                .get(format!("{}/v1/quote", self.lifi_endpoint))
                .query(&query),
        );
        Self::send_json(request, "quote")
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn place_order(
        &self,
        from_chain: &str,
        to_chain: &str,
        sell_token: &str,
        buy_token: &str,
        from_amount: &str,
        from_address: &str,
        receiver_address: Option<&str>,
        slippage: Option<f64>,
    ) -> Result<PreparedOrder, String> {
        let from_chain_id = normalize_lifi_chain_id(from_chain)?;
        let to_chain_id = normalize_lifi_chain_id(to_chain)?;
        let query = QuoteQuery {
            from_chain: &from_chain_id,
            to_chain: &to_chain_id,
            from_token: sell_token,
            to_token: buy_token,
            from_amount,
            from_address,
            to_address: receiver_address,
            slippage,
        };

        let request = self.authed(
            self.http
                .get(format!("{}/v1/quote", self.lifi_endpoint))
                .query(&query),
        );

        let quote: LifiQuoteResponse = Self::send_json(request, "place order")?;
        let main_tx = build_lifi_main_tx(&quote);
        let approval_tx = build_lifi_approval_tx(&quote, from_amount)?;
        Ok(PreparedOrder {
            raw_quote: serde_json::to_value(&quote)
                .map_err(|e| format!("[lifi] failed to serialize raw quote: {e}"))?,
            main_tx,
            approval_tx,
        })
    }

    pub(crate) fn get_transfer_status(
        &self,
        tx_hash: &str,
        from_chain: Option<&str>,
        to_chain: Option<&str>,
        bridge: Option<&str>,
    ) -> Result<Value, String> {
        let request = self.authed(
            self.http
                .get(format!("{}/v1/status", self.lifi_endpoint))
                .query(&StatusQuery {
                    tx_hash,
                    from_chain,
                    to_chain,
                    bridge,
                }),
        );
        Self::send_json(request, "transfer status")
    }

    pub(crate) fn get_chains(&self, chain_types: Option<&str>) -> Result<Value, String> {
        let request = self.authed(
            self.http
                .get(format!("{}/v1/chains", self.lifi_endpoint))
                .query(&ChainsQuery { chain_types }),
        );
        Self::send_json(request, "chains")
    }

    pub(crate) fn get_tokens(
        &self,
        chains: Option<&str>,
        chain_types: Option<&str>,
    ) -> Result<Value, String> {
        let request = self.authed(
            self.http
                .get(format!("{}/v1/tokens", self.lifi_endpoint))
                .query(&TokensQuery {
                    chains,
                    chain_types,
                }),
        );
        Self::send_json(request, "tokens")
    }

    pub(crate) fn get_token(&self, chain: &str, token: &str) -> Result<Value, String> {
        let request = self.authed(
            self.http
                .get(format!("{}/v1/token", self.lifi_endpoint))
                .query(&TokenQuery { chain, token }),
        );
        Self::send_json(request, "token")
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_routes(
        &self,
        from_chain: &str,
        to_chain: &str,
        from_token: &str,
        to_token: &str,
        amount: f64,
        from_address: &str,
        slippage: Option<f64>,
        order_preference: Option<&str>,
    ) -> Result<Value, String> {
        let (from_chain_name, from_chain_id) = get_chain_info(from_chain)?;
        let (to_chain_name, to_chain_id) = get_chain_info(to_chain)?;
        let from_token_addr = get_token_address(from_chain_name, from_token)?;
        let to_token_addr = get_token_address(to_chain_name, to_token)?;
        let from_decimals = get_token_decimals(from_chain_name, from_token);
        let from_amount = amount_to_base_units(amount, from_decimals)?;
        let body = RouteRequest {
            from_chain_id,
            to_chain_id,
            from_token_address: &from_token_addr,
            to_token_address: &to_token_addr,
            from_amount: &from_amount,
            from_address,
            options: RouteOptions {
                slippage,
                order: order_preference,
            },
        };

        let request = self.authed(
            self.http
                .post(format!("{}/v1/advanced/routes", self.lifi_endpoint))
                .json(&body),
        );
        Self::send_json(request, "routes")
    }

    pub(crate) fn get_step_transaction(&self, step: &Value) -> Result<Value, String> {
        let request = self.authed(
            self.http
                .post(format!(
                    "{}/v1/advanced/stepTransaction",
                    self.lifi_endpoint
                ))
                .json(step),
        );
        Self::send_json(request, "step transaction")
    }

    pub(crate) fn get_connections(
        &self,
        from_chain: Option<&str>,
        to_chain: Option<&str>,
        from_token: Option<&str>,
        to_token: Option<&str>,
    ) -> Result<Value, String> {
        let request = self.authed(
            self.http
                .get(format!("{}/v1/connections", self.lifi_endpoint))
                .query(&ConnectionsQuery {
                    from_chain,
                    to_chain,
                    from_token,
                    to_token,
                }),
        );
        Self::send_json(request, "connections")
    }

    pub(crate) fn get_tools(&self, chains: Option<&str>) -> Result<Value, String> {
        let request = self.authed(
            self.http
                .get(format!("{}/v1/tools", self.lifi_endpoint))
                .query(&ToolsQuery { chains }),
        );
        Self::send_json(request, "tools")
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_reverse_quote(
        &self,
        from_chain: &str,
        to_chain: Option<&str>,
        from_token: &str,
        to_token: &str,
        to_amount: &str,
        from_address: &str,
        to_address: Option<&str>,
    ) -> Result<Value, String> {
        let from_chain_id = normalize_lifi_chain_id(from_chain)?;
        let effective_to_chain = to_chain.unwrap_or(from_chain);
        let to_chain_id = normalize_lifi_chain_id(effective_to_chain)?;
        let query = ReverseQuoteQuery {
            from_chain: &from_chain_id,
            to_chain: &to_chain_id,
            from_token,
            to_token,
            to_amount,
            from_address,
            to_address,
        };

        let request = self.authed(
            self.http
                .get(format!("{}/v1/quote/toAmount", self.lifi_endpoint))
                .query(&query),
        );
        Self::send_json(request, "reverse quote")
    }

    pub(crate) fn get_gas_suggestion(
        &self,
        chain: &str,
        from_chain: Option<&str>,
        from_token: Option<&str>,
    ) -> Result<Value, String> {
        let request = self.authed(
            self.http
                .get(format!(
                    "{}/v1/gas/suggestion/{}",
                    self.lifi_endpoint, chain
                ))
                .query(&GasSuggestionQuery {
                    from_chain,
                    from_token,
                }),
        );
        Self::send_json(request, "gas suggestion")
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_bridge_quote(
        &self,
        from_chain: &str,
        to_chain: &str,
        from_token: &str,
        to_token: &str,
        amount: f64,
        from_address: Option<&str>,
        to_address: Option<&str>,
        slippage_bps: Option<u32>,
    ) -> Result<BridgeQuoteResponse, String> {
        let (from_chain_name, _) = get_chain_info(from_chain)?;
        let (to_chain_name, _) = get_chain_info(to_chain)?;
        let from_token_addr = get_token_address(from_chain_name, from_token)?;
        let to_token_addr = get_token_address(to_chain_name, to_token)?;
        let from_decimals = get_token_decimals(from_chain_name, from_token);
        let to_decimals = get_token_decimals(to_chain_name, to_token);
        let from_amount_wei = amount_to_base_units(amount, from_decimals)?;
        let slippage_bps = slippage_bps.unwrap_or(50);
        let slippage = (slippage_bps as f64) / 10_000.0;
        let from_chain_id = normalize_lifi_chain_id(from_chain)?;
        let to_chain_id = normalize_lifi_chain_id(to_chain)?;

        let from_label = format!(
            "{amount} {} on {}",
            from_token.to_uppercase(),
            from_chain.to_lowercase()
        );
        let to_label = format!("{} on {}", to_token.to_uppercase(), to_chain.to_lowercase());

        let from_addr = from_address.unwrap_or("");
        let to_addr = to_address.unwrap_or("");
        if !(is_hex_address(from_addr) && is_hex_address(to_addr)) {
            return Ok(BridgeQuoteResponse::planning_only(
                from_label,
                to_label,
                vec!["Source and destination wallet addresses are required".to_string()],
                Some(
                    "Provide source and destination wallet addresses to request an executable bridge route."
                        .to_string(),
                ),
            ));
        }

        let request = self.authed(
            self.http
                .get(format!("{}/v1/quote", self.lifi_endpoint))
                .query(&QuoteQuery {
                    from_chain: &from_chain_id,
                    to_chain: &to_chain_id,
                    from_token: &from_token_addr,
                    to_token: &to_token_addr,
                    from_amount: &from_amount_wei,
                    from_address: from_addr,
                    to_address: Some(to_addr),
                    slippage: Some(slippage),
                }),
        );

        if let Ok(quote) = Self::send_json::<LifiQuoteResponse>(request, "bridge quote") {
            return Ok(BridgeQuoteResponse::from_lifi_quote(
                &quote,
                from_label,
                to_label,
                to_decimals,
            ));
        }

        // Fallback: minimal price estimate using DeFiLlama
        let from_price = fetch_token_price(from_token).unwrap_or(1.0);
        let to_price = fetch_token_price(to_token).unwrap_or(1.0);
        let estimated_to_amount = (amount * from_price) / to_price * (1.0 - slippage.max(0.001));
        let min_received = estimated_to_amount * (1.0 - slippage);

        Ok(BridgeQuoteResponse {
            from: from_label,
            to: to_label,
            to_amount_estimate: Some(format!("{estimated_to_amount:.6}")),
            min_received: Some(format!("{min_received:.6}")),
            bridge: "planning-only".to_string(),
            estimated_duration_seconds: None,
            estimated_fee_usd: None,
            route_summary: vec!["No executable bridge payload available".to_string()],
            executable_tx: None,
            execution_supported: false,
            warning: Some(
                "Bridge quote is planning-only. Provide source and destination wallet addresses for executable routing."
                    .to_string(),
            ),
        })
    }
}

// ============================================================================
// Shared helpers
// ============================================================================

pub(crate) fn normalize_lifi_chain_id(chain: &str) -> Result<String, String> {
    let normalized = chain.to_lowercase();
    let chain_id = match normalized.as_str() {
        "ethereum" | "eth" | "mainnet" => "1",
        "polygon" | "matic" => "137",
        "arbitrum" | "arb" | "arbitrum_one" => "42161",
        "optimism" | "op" => "10",
        "base" => "8453",
        "bsc" | "bnb" | "binance" => "56",
        "avalanche" | "avax" => "43114",
        "gnosis" | "xdai" => "100",
        "fantom" | "ftm" => "250",
        "linea" => "59144",
        "scroll" => "534352",
        "zksync" | "zksync_era" => "324",
        _ => {
            if chain.chars().all(|c| c.is_ascii_digit()) {
                return Ok(chain.to_string());
            }
            return Err(format!(
                "[lifi] unsupported chain '{chain}'. Use a known chain name or numeric chain id"
            ));
        }
    };

    Ok(chain_id.to_string())
}

pub(crate) fn build_lifi_main_tx(quote: &LifiQuoteResponse) -> PreparedTransaction {
    quote
        .transaction_request
        .as_ref()
        .map(|tx| tx.to_prepared_transaction("LI.FI main transaction"))
        .unwrap_or(PreparedTransaction {
            to: Value::Null,
            data: Value::String("0x".to_string()),
            value: Value::String("0".to_string()),
            gas_limit: Value::Null,
            description: "LI.FI main transaction",
        })
}

pub(crate) fn build_lifi_approval_tx(
    quote: &LifiQuoteResponse,
    from_amount: &str,
) -> Result<Option<PreparedTransaction>, String> {
    let approval_address = quote.estimate.approval_address.as_deref();
    let from_token_address = quote
        .action
        .as_ref()
        .and_then(|action| action.from_token.as_ref())
        .and_then(|token| token.address.as_deref());

    if let (Some(spender), Some(token_address)) = (approval_address, from_token_address) {
        let is_native = token_address
            .eq_ignore_ascii_case("0x0000000000000000000000000000000000000000")
            || token_address.eq_ignore_ascii_case("0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee");
        if !is_native && is_hex_address(token_address) && is_hex_address(spender) {
            let approve_calldata = encode_approve_calldata(spender, from_amount)?;
            return Ok(Some(PreparedTransaction {
                to: Value::String(token_address.to_string()),
                data: Value::String(approve_calldata),
                value: Value::String("0".to_string()),
                gas_limit: Value::Null,
                description: "LI.FI token approval",
            }));
        }
    }

    Ok(None)
}

pub(crate) fn encode_approve_calldata(
    spender: &str,
    amount_decimal: &str,
) -> Result<String, String> {
    let selector = "095ea7b3"; // approve(address,uint256)
    let spender_clean = spender.trim_start_matches("0x").to_lowercase();
    if spender_clean.len() != 40 || !spender_clean.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("[lifi] invalid approval spender address".to_string());
    }
    let amount = amount_decimal
        .parse::<u128>()
        .map_err(|e| format!("[lifi] invalid approval amount {amount_decimal}: {e}"))?;
    let amount_hex = format!("{amount:x}");

    let spender_slot = format!("{spender_clean:0>64}");
    let amount_slot = format!("{amount_hex:0>64}");
    Ok(format!("0x{selector}{spender_slot}{amount_slot}"))
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
        _ => Err(format!("[lifi] unsupported chain: {chain}")),
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
        _ => Err(format!("[lifi] unknown token {token} on chain {chain}")),
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

/// Minimal price fetch from DeFiLlama for bridge quote fallback
fn fetch_token_price(token: &str) -> Option<f64> {
    let token_lower = token.to_lowercase();
    let coin_id = match token_lower.as_str() {
        "eth" | "ethereum" => "coingecko:ethereum",
        "btc" | "bitcoin" => "coingecko:bitcoin",
        "usdc" => "coingecko:usd-coin",
        "usdt" | "tether" => "coingecko:tether",
        "dai" => "coingecko:dai",
        "sol" | "solana" => "coingecko:solana",
        "bnb" => "coingecko:binancecoin",
        "avax" | "avalanche" => "coingecko:avalanche-2",
        "matic" | "polygon" => "coingecko:matic-network",
        _ => return None,
    };

    let url = format!("https://coins.llama.fi/prices/current/{coin_id}");
    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;
    let body: Value = http.get(&url).send().ok()?.json().ok()?;
    body.get("coins")
        .and_then(Value::as_object)
        .and_then(|coins| coins.values().next())
        .and_then(|coin| coin.get("price"))
        .and_then(Value::as_f64)
}

// ============================================================================
// Tool arg structs
// ============================================================================

pub(crate) struct GetLifiSwapQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiSwapQuoteArgs {
    /// Source chain
    pub(crate) chain: String,
    /// Destination chain (defaults to source chain for same-chain swaps)
    pub(crate) destination_chain: Option<String>,
    /// Sell token symbol or address
    pub(crate) sell_token: String,
    /// Buy token symbol or address
    pub(crate) buy_token: String,
    /// Amount to swap (human-readable units)
    pub(crate) amount: f64,
    /// Sender wallet address
    pub(crate) sender_address: String,
    /// Receiver wallet address (defaults to sender)
    pub(crate) receiver_address: Option<String>,
}

pub(crate) struct PlaceLifiOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceLifiOrderArgs {
    /// Source chain
    pub(crate) chain: String,
    /// Destination chain (defaults to source chain)
    pub(crate) destination_chain: Option<String>,
    /// Sell token symbol or address
    pub(crate) sell_token: String,
    /// Buy token symbol or address
    pub(crate) buy_token: String,
    /// Sell amount (human-readable units)
    pub(crate) amount: f64,
    /// Sender/taker wallet address
    pub(crate) sender_address: String,
    /// Receiver wallet address
    pub(crate) receiver_address: Option<String>,
    /// Slippage tolerance as decimal (0.005 = 0.5%)
    pub(crate) slippage: Option<f64>,
}

pub(crate) struct GetLifiBridgeQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiBridgeQuoteArgs {
    /// Source chain
    pub(crate) from_chain: String,
    /// Destination chain
    pub(crate) to_chain: String,
    /// Source token symbol/address
    pub(crate) from_token: String,
    /// Destination token symbol/address
    pub(crate) to_token: String,
    /// Amount to bridge
    pub(crate) amount: f64,
    /// Sender wallet address; needed for executable quote
    pub(crate) from_address: Option<String>,
    /// Receiver wallet address; needed for executable quote
    pub(crate) to_address: Option<String>,
    /// Slippage tolerance in basis points (default 50)
    pub(crate) slippage_bps: Option<u32>,
}

pub(crate) struct GetLifiTransferStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiTransferStatusArgs {
    /// Transaction hash to track
    pub(crate) tx_hash: String,
    /// Source chain name or ID (speeds up lookup)
    pub(crate) from_chain: Option<String>,
    /// Destination chain name or ID (speeds up lookup)
    pub(crate) to_chain: Option<String>,
    /// Bridge name (speeds up lookup)
    pub(crate) bridge: Option<String>,
}

pub(crate) struct GetLifiChains;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiChainsArgs {
    /// Filter by chain type, e.g. "EVM", "SVM"
    pub(crate) chain_types: Option<String>,
}

pub(crate) struct GetLifiTokens;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiTokensArgs {
    /// Comma-separated chain IDs to filter tokens
    pub(crate) chains: Option<String>,
    /// Filter by chain type, e.g. "EVM", "SVM"
    pub(crate) chain_types: Option<String>,
}

pub(crate) struct GetLifiToken;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiTokenArgs {
    /// Chain name or ID
    pub(crate) chain: String,
    /// Token address or symbol
    pub(crate) token: String,
}

pub(crate) struct GetLifiRoutes;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiRoutesArgs {
    /// Source chain name or ID
    pub(crate) from_chain: String,
    /// Destination chain name or ID
    pub(crate) to_chain: String,
    /// Source token symbol or address
    pub(crate) from_token: String,
    /// Destination token symbol or address
    pub(crate) to_token: String,
    /// Amount to swap/bridge (human-readable units)
    pub(crate) amount: f64,
    /// Sender wallet address
    pub(crate) from_address: String,
    /// Slippage tolerance as decimal (e.g. 0.005 = 0.5%)
    pub(crate) slippage: Option<f64>,
    /// Route ordering preference: "CHEAPEST", "FASTEST", "SAFEST", or "RECOMMENDED"
    pub(crate) order_preference: Option<String>,
}

pub(crate) struct GetLifiStepTransaction;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiStepTransactionArgs {
    /// A Step object as returned by /advanced/routes
    pub(crate) step: Value,
}

pub(crate) struct GetLifiConnections;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiConnectionsArgs {
    /// Source chain name or ID
    pub(crate) from_chain: Option<String>,
    /// Destination chain name or ID
    pub(crate) to_chain: Option<String>,
    /// Source token address
    pub(crate) from_token: Option<String>,
    /// Destination token address
    pub(crate) to_token: Option<String>,
}

pub(crate) struct GetLifiTools;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiToolsArgs {
    /// Comma-separated chain IDs to filter
    pub(crate) chains: Option<String>,
}

pub(crate) struct GetLifiReverseQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiReverseQuoteArgs {
    /// Source chain name or ID
    pub(crate) from_chain: String,
    /// Destination chain name or ID (defaults to source chain)
    pub(crate) to_chain: Option<String>,
    /// Source token symbol or address
    pub(crate) from_token: String,
    /// Destination token symbol or address
    pub(crate) to_token: String,
    /// Desired output amount in base units (e.g. wei)
    pub(crate) to_amount: String,
    /// Sender wallet address
    pub(crate) from_address: String,
    /// Receiver wallet address (defaults to sender)
    pub(crate) to_address: Option<String>,
}

pub(crate) struct GetLifiGasSuggestion;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetLifiGasSuggestionArgs {
    /// Destination chain name or ID
    pub(crate) chain: String,
    /// Source chain name or ID
    pub(crate) from_chain: Option<String>,
    /// Source token address
    pub(crate) from_token: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> LifiClient {
        LifiClient::new().expect("client should build")
    }

    #[test]
    fn chains_smoke() {
        let res = client().get_chains(None).expect("should get chains");
        assert!(res.get("chains").is_some(), "should have chains key");
    }

    #[test]
    fn chains_evm_filter_smoke() {
        let _ = client()
            .get_chains(Some("EVM"))
            .expect("should get EVM chains");
    }

    #[test]
    fn tokens_single_chain_smoke() {
        let res = client()
            .get_tokens(Some("1"), None)
            .expect("should get ethereum tokens");
        assert!(res.get("tokens").is_some(), "should have tokens key");
    }

    #[test]
    fn token_detail_smoke() {
        let _ = client()
            .get_token("1", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
            .expect("should get USDC token detail");
    }

    #[test]
    fn tools_smoke() {
        let res = client().get_tools(None).expect("should get tools");
        assert!(res.get("bridges").is_some() || res.get("exchanges").is_some());
    }

    #[test]
    fn connections_smoke() {
        let res = client()
            .get_connections(Some("1"), Some("137"), None, None)
            .expect("should get connections");
        assert!(
            res.get("connections").is_some(),
            "should have connections key"
        );
    }

    #[test]
    fn gas_suggestion_smoke() {
        let _ = client()
            .get_gas_suggestion("1", None, None)
            .expect("should get gas suggestion for ethereum");
    }

    #[test]
    fn normalize_chain_id_known() {
        assert_eq!(normalize_lifi_chain_id("ethereum").unwrap(), "1");
        assert_eq!(normalize_lifi_chain_id("polygon").unwrap(), "137");
        assert_eq!(normalize_lifi_chain_id("base").unwrap(), "8453");
        assert_eq!(normalize_lifi_chain_id("42161").unwrap(), "42161");
    }

    #[test]
    fn normalize_chain_id_unknown() {
        assert!(normalize_lifi_chain_id("foobar").is_err());
    }

    #[test]
    fn amount_to_base_units_usdc() {
        assert_eq!(amount_to_base_units(100.5, 6).unwrap(), "100500000");
    }

    #[test]
    fn amount_to_base_units_eth() {
        assert_eq!(
            amount_to_base_units(1.0, 18).unwrap(),
            "1000000000000000000"
        );
    }

    #[test]
    fn is_hex_address_valid() {
        assert!(is_hex_address("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"));
        assert!(!is_hex_address("usdc"));
        assert!(!is_hex_address("0xinvalid"));
    }
}
