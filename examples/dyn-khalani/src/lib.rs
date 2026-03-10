use aomi_dyn_sdk::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

const DEFAULT_KHALANI_API_ENDPOINT: &str = "https://api.hyperstream.dev";
const SYSTEM_NEXT_ACTION_KEY: &str = "SYSTEM_NEXT_ACTION";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NextAction {
    ToolCalls(Vec<NextActionTool>),
    Instructions(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextActionTool {
    pub name: String,
    pub reason: String,
    pub args: Value,
    pub condition: Option<String>,
}

#[derive(Clone)]
struct KhalaniClient {
    http: reqwest::blocking::Client,
    endpoint: String,
    api_key: Option<String>,
}

struct KhalaniQuoteRequest<'a> {
    from_chain: &'a str,
    to_chain: &'a str,
    from_token: &'a str,
    to_token: &'a str,
    from_amount: &'a str,
    from_address: &'a str,
    to_address: Option<&'a str>,
    slippage_bps: Option<u32>,
}

impl KhalaniClient {
    fn env_nonempty(name: &str) -> Option<String> {
        std::env::var(name)
            .ok()
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }

    fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        Ok(Self {
            http,
            endpoint: Self::env_nonempty("KHALANI_API_ENDPOINT")
                .unwrap_or_else(|| DEFAULT_KHALANI_API_ENDPOINT.to_string()),
            api_key: Self::env_nonempty("KHALANI_API_KEY"),
        })
    }

    fn with_source(value: Value, source: &str) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert("source".to_string(), Value::String(source.to_string()));
                Value::Object(map)
            }
            other => json!({
                "source": source,
                "data": other,
            }),
        }
    }

    fn send_json(
        &self,
        request: reqwest::blocking::RequestBuilder,
        operation: &str,
    ) -> Result<Value, String> {
        let response = request
            .send()
            .map_err(|e| format!("[khalani] {operation} request failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "[khalani] {operation} request failed: {status} {body}"
            ));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[khalani] {operation} decode failed: {e}; body: {body}"))
    }

    fn amount_to_base_units(amount: f64, decimals: u8) -> Result<String, String> {
        if !amount.is_finite() || amount < 0.0 {
            return Err("amount must be a finite non-negative number".to_string());
        }
        let scaled = amount * 10f64.powi(decimals as i32);
        if scaled > (u128::MAX as f64) {
            return Err("amount is too large to convert to base units".to_string());
        }
        Ok((scaled.round() as u128).to_string())
    }

    fn get_chain_info(chain: &str) -> Result<(&'static str, u64), String> {
        match chain.to_lowercase().as_str() {
            "ethereum" | "eth" | "mainnet" => Ok(("ethereum", 1)),
            "polygon" | "matic" => Ok(("polygon", 137)),
            "arbitrum" | "arb" => Ok(("arbitrum", 42161)),
            "optimism" | "op" => Ok(("optimism", 10)),
            "base" => Ok(("base", 8453)),
            "bsc" | "binance" => Ok(("bsc", 56)),
            "avalanche" | "avax" => Ok(("avalanche", 43114)),
            _ => Err(format!("unsupported chain: {chain}")),
        }
    }

    fn is_hex_address(token: &str) -> bool {
        token.len() == 42
            && token.starts_with("0x")
            && token[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    fn get_token_address(chain: &str, token: &str) -> Result<String, String> {
        let native = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";
        let token_lower = token.to_lowercase();

        if token_lower == native.to_lowercase() {
            return Ok(native.to_string());
        }
        if Self::is_hex_address(token) {
            return Ok(token.to_string());
        }

        match (chain, token_lower.as_str()) {
            (_, "eth") | (_, "matic") | (_, "bnb") | (_, "avax") => Ok(native.to_string()),
            ("ethereum", "usdc") => Ok("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string()),
            ("ethereum", "usdt") => Ok("0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string()),
            ("ethereum", "dai") => Ok("0x6B175474E89094C44Da98b954EedeAC495271d0F".to_string()),
            ("ethereum", "weth") => Ok("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string()),
            ("ethereum", "wbtc") => Ok("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599".to_string()),
            ("arbitrum", "usdc") => Ok("0xaf88d065e77c8cC2239327C5EDb3A432268e5831".to_string()),
            ("arbitrum", "usdt") => Ok("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9".to_string()),
            ("arbitrum", "weth") => Ok("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1".to_string()),
            ("base", "usdc") => Ok("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()),
            ("base", "weth") => Ok("0x4200000000000000000000000000000000000006".to_string()),
            ("polygon", "usdc") => Ok("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359".to_string()),
            ("polygon", "usdt") => Ok("0xc2132D05D31c914a87C6611C10748AEb04B58e8F".to_string()),
            ("polygon", "weth") => Ok("0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619".to_string()),
            _ => Err(format!("unknown token {token} on chain {chain}")),
        }
    }

    fn get_token_decimals(chain: &str, token: &str) -> u8 {
        let token_lower = token.to_lowercase();

        if Self::is_hex_address(token) {
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

    fn quote_amount_base_units(
        &self,
        chain: &str,
        token: &str,
        amount: f64,
    ) -> Result<String, String> {
        let (chain_name, _) = Self::get_chain_info(chain)?;
        let decimals = Self::get_token_decimals(chain_name, token);
        Self::amount_to_base_units(amount, decimals)
    }

    fn get_quote_khalani(&self, req: KhalaniQuoteRequest<'_>) -> Result<Value, String> {
        let (from_chain_name, from_chain_id) = Self::get_chain_info(req.from_chain)?;
        let (to_chain_name, to_chain_id) = Self::get_chain_info(req.to_chain)?;
        let from_addr = Self::get_token_address(from_chain_name, req.from_token)?;
        let to_addr = Self::get_token_address(to_chain_name, req.to_token)?;
        let receiver = req.to_address.unwrap_or(req.from_address);

        let base_url = format!("{}/v1/quotes", self.endpoint.trim_end_matches('/'));
        let mut payloads = vec![
            json!({
                "tradeType": "EXACT_INPUT",
                "fromChainId": from_chain_id,
                "toChainId": to_chain_id,
                "fromToken": from_addr,
                "toToken": to_addr,
                "amount": req.from_amount,
                "fromAddress": req.from_address,
                "recipient": receiver,
                "refundTo": req.from_address,
            }),
            json!({
                "tradeType": "EXACT_INPUT",
                "fromChainId": from_chain_id,
                "toChainId": to_chain_id,
                "fromToken": from_addr,
                "toToken": to_addr,
                "amount": req.from_amount,
                "fromAddress": req.from_address,
                "toAddress": receiver,
            }),
            json!({
                "tradeType": "EXACT_INPUT",
                "fromChainId": from_chain_id,
                "toChainId": to_chain_id,
                "fromToken": from_addr,
                "toToken": to_addr,
                "amount": req.from_amount,
                "userAddress": req.from_address,
                "recipient": receiver,
            }),
            json!({
                "fromChainId": from_chain_id,
                "toChainId": to_chain_id,
                "fromAddress": req.from_address,
                "toAddress": receiver,
                "fromToken": from_addr,
                "toToken": to_addr,
                "fromAmount": req.from_amount,
                "orderType": "TOKENS_TO_TOKENS",
                "tradeType": "EXACT_INPUT",
            }),
        ];

        let mut errors = Vec::new();
        for payload in &mut payloads {
            if let Some(bps) = req.slippage_bps {
                payload["slippageInBps"] = json!(bps);
            }

            let mut request = self.http.post(base_url.clone()).json(payload);
            if let Some(api_key) = self.api_key.as_deref() {
                request = request.header("x-api-key", api_key);
            }

            match self.send_json(request, "quote") {
                Ok(value) => return Ok(Self::with_source(value, "khalani")),
                Err(err) => {
                    errors.push(format!(
                        "payload={} error={}",
                        payload,
                        err.replace('\n', " ")
                    ));
                }
            }
        }

        if errors.is_empty() {
            Err("[khalani] quote request failed".to_string())
        } else {
            Err(format!(
                "[khalani] quote request failed for all payload variants: {}",
                errors.join(" | ")
            ))
        }
    }

    fn build_deposit_khalani(&self, payload: Value) -> Result<Value, String> {
        let mut request = self
            .http
            .post(format!(
                "{}/v1/deposit/build",
                self.endpoint.trim_end_matches('/')
            ))
            .json(&payload);
        if let Some(api_key) = self.api_key.as_deref() {
            request = request.header("x-api-key", api_key);
        }

        let value = self.send_json(request, "deposit build")?;
        Ok(Self::with_source(value, "khalani"))
    }

    fn submit_deposit_khalani(&self, payload: Value) -> Result<Value, String> {
        let mut request = self
            .http
            .put(format!(
                "{}/v1/deposit/submit",
                self.endpoint.trim_end_matches('/')
            ))
            .json(&payload);
        if let Some(api_key) = self.api_key.as_deref() {
            request = request.header("x-api-key", api_key);
        }

        let value = self.send_json(request, "deposit submit")?;
        Ok(Self::with_source(value, "khalani"))
    }

    fn get_order_khalani(&self, order_id: &str) -> Result<Value, String> {
        let mut request = self.http.get(format!(
            "{}/v1/orders/id/{}",
            self.endpoint.trim_end_matches('/'),
            order_id
        ));
        if let Some(api_key) = self.api_key.as_deref() {
            request = request.header("x-api-key", api_key);
        }

        let value = self.send_json(request, "get order")?;
        Ok(Self::with_source(value, "khalani"))
    }

    fn get_orders_by_address_khalani(
        &self,
        address: &str,
        status: Option<&str>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Value, String> {
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(status) = status {
            query.push(("status", status.to_string()));
        }
        if let Some(limit) = limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(offset) = offset {
            query.push(("offset", offset.to_string()));
        }

        let mut request = self
            .http
            .get(format!(
                "{}/v1/orders/{}",
                self.endpoint.trim_end_matches('/'),
                address
            ))
            .query(&query);
        if let Some(api_key) = self.api_key.as_deref() {
            request = request.header("x-api-key", api_key);
        }

        let value = self.send_json(request, "get orders by address")?;
        Ok(Self::with_source(value, "khalani"))
    }

    fn get_tokens_khalani(
        &self,
        chain_id: Option<u64>,
        limit: Option<u32>,
        offset: Option<u32>,
        query: Option<&str>,
    ) -> Result<Value, String> {
        let mut query_params: Vec<(&str, String)> = Vec::new();
        if let Some(chain_id) = chain_id {
            query_params.push(("chainId", chain_id.to_string()));
        }
        if let Some(limit) = limit {
            query_params.push(("limit", limit.to_string()));
        }
        if let Some(offset) = offset {
            query_params.push(("offset", offset.to_string()));
        }
        if let Some(query) = query {
            query_params.push(("q", query.to_string()));
        }

        let mut request = self
            .http
            .get(format!("{}/v1/tokens", self.endpoint.trim_end_matches('/')))
            .query(&query_params);
        if let Some(api_key) = self.api_key.as_deref() {
            request = request.header("x-api-key", api_key);
        }

        let value = self.send_json(request, "get tokens")?;
        Ok(Self::with_source(value, "khalani"))
    }

    fn search_tokens_khalani(
        &self,
        query: &str,
        chain_id: Option<u64>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Value, String> {
        let mut query_params: Vec<(&str, String)> = vec![("q", query.to_string())];
        if let Some(chain_id) = chain_id {
            query_params.push(("chainId", chain_id.to_string()));
        }
        if let Some(limit) = limit {
            query_params.push(("limit", limit.to_string()));
        }
        if let Some(offset) = offset {
            query_params.push(("offset", offset.to_string()));
        }

        let mut request = self
            .http
            .get(format!(
                "{}/v1/tokens/search",
                self.endpoint.trim_end_matches('/')
            ))
            .query(&query_params);
        if let Some(api_key) = self.api_key.as_deref() {
            request = request.header("x-api-key", api_key);
        }

        let value = self.send_json(request, "search tokens")?;
        Ok(Self::with_source(value, "khalani"))
    }

    fn get_chains_khalani(&self) -> Result<Value, String> {
        let mut request = self
            .http
            .get(format!("{}/v1/chains", self.endpoint.trim_end_matches('/')));
        if let Some(api_key) = self.api_key.as_deref() {
            request = request.header("x-api-key", api_key);
        }

        let value = self.send_json(request, "get chains")?;
        Ok(Self::with_source(value, "khalani"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetKhalaniQuoteArgs {
    pub chain: String,
    pub destination_chain: Option<String>,
    pub sell_token: String,
    pub buy_token: String,
    pub amount: f64,
    pub sender_address: Option<String>,
    pub receiver_address: Option<String>,
    pub slippage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BuildKhalaniOrderArgs {
    pub chain: String,
    pub destination_chain: Option<String>,
    pub sell_token: String,
    pub buy_token: String,
    pub amount: f64,
    pub sender_address: Option<String>,
    pub receiver_address: Option<String>,
    pub slippage: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubmitKhalaniOrderArgs {
    pub quote_id: String,
    pub route_id: Option<String>,
    pub submit_type: String,
    pub transaction_hash: Option<String>,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetKhalaniOrderStatusArgs {
    pub order_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetKhalaniOrdersByAddressArgs {
    pub address: String,
    pub status: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetKhalaniTokensArgs {
    pub chain_id: Option<u64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SearchKhalaniTokensArgs {
    pub query: String,
    pub chain_id: Option<u64>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GetKhalaniChainsArgs {}

fn parse_args<T: DeserializeOwned>(args_json: &str) -> Result<T, String> {
    serde_json::from_str(args_json).map_err(|e| format!("invalid args: {e}"))
}

fn parse_ctx(ctx_json: &str) -> Result<DynCtx, String> {
    serde_json::from_str(ctx_json).map_err(|e| format!("invalid ctx: {e}"))
}

fn slippage_to_bps(slippage: Option<f64>) -> Option<u32> {
    slippage.map(|s| (s * 10_000.0).round()).and_then(|bps| {
        if bps.is_finite() && bps >= 0.0 {
            Some(bps as u32)
        } else {
            None
        }
    })
}

fn resolve_sender_address(
    user_address: Option<String>,
    provided: Option<&str>,
) -> Result<String, String> {
    user_address
        .or_else(|| provided.map(ToString::to_string))
        .ok_or_else(|| "No connected wallet address found in context".to_string())
}

fn normalize_khalani_quote_response(value: &Value) -> Option<Value> {
    if let Some(array) = value.as_array() {
        return array.first().cloned();
    }
    if let Some(data_array) = value.get("data").and_then(Value::as_array) {
        return data_array.first().cloned();
    }
    if value.is_object() {
        return Some(value.clone());
    }
    None
}

fn extract_khalani_quote_id(value: &Value) -> Option<String> {
    [
        value.get("quoteId").and_then(Value::as_str),
        value.get("quote_id").and_then(Value::as_str),
        value.get("id").and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .find(|v| !v.trim().is_empty())
    .map(ToString::to_string)
}

fn extract_khalani_allowance_target(value: &Value) -> Option<String> {
    [
        value.get("allowanceTarget").and_then(Value::as_str),
        value
            .get("allowance")
            .and_then(|v| v.get("target"))
            .and_then(Value::as_str),
        value
            .get("route")
            .and_then(|v| v.get("allowanceTarget"))
            .and_then(Value::as_str),
    ]
    .into_iter()
    .flatten()
    .find(|v| !v.trim().is_empty())
    .map(ToString::to_string)
}

fn extract_khalani_route_id(value: &Value) -> Option<String> {
    let route_from_array = value
        .get("routes")
        .and_then(Value::as_array)
        .and_then(|routes| routes.first())
        .and_then(|route| {
            route
                .get("routeId")
                .and_then(Value::as_str)
                .or_else(|| route.get("id").and_then(Value::as_str))
        });

    [
        value.get("routeId").and_then(Value::as_str),
        value.get("route_id").and_then(Value::as_str),
        value.get("selectedRouteId").and_then(Value::as_str),
        route_from_array,
    ]
    .into_iter()
    .flatten()
    .find(|v| !v.trim().is_empty())
    .map(ToString::to_string)
}

fn hex_to_decimal_wei(s: &str) -> String {
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    match hex {
        Some(h) => u128::from_str_radix(h, 16)
            .map(|v| v.to_string())
            .unwrap_or_else(|_| s.to_string()),
        None => s.to_string(),
    }
}

fn normalize_tx_fields(tx: &Value) -> Option<Value> {
    let to = tx.get("to").and_then(Value::as_str)?;
    let raw_value = tx.get("value").and_then(Value::as_str).unwrap_or("0");
    Some(json!({
        "to": to,
        "data": tx.get("data").and_then(Value::as_str).unwrap_or("0x"),
        "value": hex_to_decimal_wei(raw_value),
        "gas_limit": tx.get("gasLimit").or_else(|| tx.get("gas")).cloned().unwrap_or(Value::Null),
    }))
}

fn extract_khalani_eth_send_tx(approval: &Value) -> Option<Value> {
    let request = approval.get("request")?;
    if request.get("method").and_then(Value::as_str)? != "eth_sendTransaction" {
        return None;
    }
    let tx = request
        .get("params")
        .and_then(Value::as_array)
        .and_then(|params| params.first())?;
    normalize_tx_fields(tx)
}

fn extract_khalani_transaction_type(build: &Value) -> String {
    build
        .get("transaction")
        .and_then(|v| v.get("type"))
        .and_then(Value::as_str)
        .or_else(|| build.get("type").and_then(Value::as_str))
        .unwrap_or("CONTRACT_CALL")
        .to_string()
}

fn extract_khalani_contract_call_tx(build: &Value) -> Option<Value> {
    let tx = build.get("transaction").unwrap_or(build);
    normalize_tx_fields(tx).or_else(|| tx.get("tx").and_then(normalize_tx_fields))
}

fn extract_khalani_typed_data(build: &Value) -> Option<Value> {
    let tx = build.get("transaction").unwrap_or(build);
    tx.get("typedData")
        .cloned()
        .or_else(|| tx.get("eip712").cloned())
        .or_else(|| tx.get("payload").cloned())
        .or_else(|| build.get("typedData").cloned())
        .filter(Value::is_object)
}

fn build_transaction_preflight(tx: &Value) -> Option<Value> {
    let to = tx.get("to").and_then(Value::as_str)?;
    let data = tx.get("data").and_then(Value::as_str)?;
    let payload = data.strip_prefix("0x")?;

    if payload.len() >= 8 + 64 + 64 && payload[..8].eq_ignore_ascii_case("095ea7b3") {
        let spender_word = &payload[8..72];
        let amount_word = &payload[72..136];
        let spender = format!("0x{}", &spender_word[24..64]);
        let amount_hex = format!("0x{amount_word}");

        return Some(json!({
            "tool": "encode_and_simulate",
            "args": {
                "function_signature": "approve(address,uint256)",
                "arguments": [spender, amount_hex],
                "to": to,
                "value": "0"
            }
        }));
    }

    None
}

fn extract_quote_summary(quote_entry: &Value) -> Value {
    let route = quote_entry
        .get("routes")
        .and_then(Value::as_array)
        .and_then(|r| r.first());
    let route_quote = route.and_then(|r| r.get("quote"));
    json!({
        "route": route
            .and_then(|r| r.get("routeId").or_else(|| r.get("id")))
            .and_then(Value::as_str)
            .unwrap_or("unknown"),
        "amount_in": route_quote.and_then(|q| q.get("amountIn")).cloned().unwrap_or(Value::Null),
        "amount_out": route_quote.and_then(|q| q.get("amountOut")).cloned().unwrap_or(Value::Null),
        "estimated_duration_seconds": route_quote
            .and_then(|q| q.get("expectedDurationSeconds"))
            .cloned()
            .unwrap_or(Value::Null),
        "tags": route_quote
            .and_then(|q| q.get("tags"))
            .cloned()
            .unwrap_or_else(|| json!([])),
    })
}

fn build_wallet_tx_request(tx: &Value, description: String) -> Value {
    json!({
        "to": tx.get("to").cloned().unwrap_or(Value::Null),
        "value": tx.get("value").cloned().unwrap_or_else(|| Value::String("0".to_string())),
        "data": tx.get("data").cloned().unwrap_or_else(|| Value::String("0x".to_string())),
        "gas_limit": tx.get("gas_limit").cloned().unwrap_or(Value::Null),
        "description": description,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_khalani_result(
    quote_id: &str,
    route_id: &Option<String>,
    transaction_type: &str,
    summary: Value,
    wallet_tool: &str,
    wallet_request: Value,
    preflight: Option<Value>,
    follow_up: Value,
) -> Result<Value, String> {
    let mut tool_calls = Vec::new();

    if let Some(ref pf) = preflight {
        if let Some(name) = pf.get("tool").and_then(Value::as_str) {
            tool_calls.push(NextActionTool {
                name: name.to_string(),
                reason: "Run preflight checks before wallet interaction.".to_string(),
                args: pf.get("args").cloned().unwrap_or_default(),
                condition: None,
            });
        }
    }

    tool_calls.push(NextActionTool {
        name: wallet_tool.to_string(),
        reason: "REQUIRED: Call this tool with these exact args. Do NOT skip or assume it was already sent.".to_string(),
        args: wallet_request.clone(),
        condition: if preflight.is_some() {
            Some("After preflight succeeds.".to_string())
        } else {
            None
        },
    });

    if let Some(step) = follow_up.get("step").and_then(Value::as_str) {
        let condition = match step {
            "build_khalani_order" => {
                Some("After wallet callback reports approval success.".to_string())
            }
            "submit_khalani_order" if wallet_tool == "send_eip712_to_wallet" => Some(
                "After wallet callback reports signature success; include signature from callback."
                    .to_string(),
            ),
            "submit_khalani_order" => Some(
                "After wallet callback reports transaction success; include transaction_hash from callback."
                    .to_string(),
            ),
            _ => None,
        };
        tool_calls.push(NextActionTool {
            name: step.to_string(),
            reason: follow_up
                .get("reason")
                .and_then(Value::as_str)
                .unwrap_or("Run the follow-up step for this workflow.")
                .to_string(),
            args: follow_up.get("args_template").cloned().unwrap_or_default(),
            condition,
        });
    }

    let action_value = serde_json::to_value(NextAction::ToolCalls(tool_calls))
        .map_err(|e| format!("failed to serialize SYSTEM_NEXT_ACTION: {e}"))?;

    let mut result = json!({
        "source": "khalani",
        "quote_id": quote_id,
        "route_id": route_id,
        "transaction_type": transaction_type,
        "summary": summary,
        "wallet_request": wallet_request,
    });

    let obj = result
        .as_object_mut()
        .ok_or_else(|| "result is not an object".to_string())?;
    obj.insert(SYSTEM_NEXT_ACTION_KEY.to_string(), action_value);

    Ok(result)
}

fn quote_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chain": { "type": "string", "description": "Source chain: ethereum, arbitrum, polygon, base, etc." },
            "destination_chain": { "type": "string", "description": "Destination chain for cross-chain routes. Defaults to source." },
            "sell_token": { "type": "string", "description": "Token to swap from." },
            "buy_token": { "type": "string", "description": "Token to swap to." },
            "amount": { "type": "number", "description": "Human-readable sell amount." },
            "sender_address": { "type": "string", "description": "Sender wallet address. Defaults to connected wallet in context." },
            "receiver_address": { "type": "string", "description": "Recipient wallet address. Defaults to sender." },
            "slippage": { "type": "number", "description": "Slippage decimal (0.005 = 0.5%)." }
        },
        "required": ["chain", "sell_token", "buy_token", "amount"]
    })
}

fn submit_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "quote_id": { "type": "string", "description": "Khalani quote ID to submit." },
            "route_id": { "type": "string", "description": "Khalani route ID when provided by build output." },
            "submit_type": { "type": "string", "description": "SIGNED_TRANSACTION or SIGNED_EIP712." },
            "transaction_hash": { "type": "string", "description": "Wallet transaction hash for SIGNED_TRANSACTION." },
            "signature": { "type": "string", "description": "Wallet signature for SIGNED_EIP712." }
        },
        "required": ["quote_id", "submit_type"]
    })
}

fn order_status_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "order_id": { "type": "string", "description": "Khalani order ID." }
        },
        "required": ["order_id"]
    })
}

fn orders_by_address_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "address": { "type": "string", "description": "Wallet address to query." },
            "status": { "type": "string", "description": "Optional status filter." },
            "limit": { "type": "integer", "description": "Optional page size." },
            "offset": { "type": "integer", "description": "Optional pagination offset." }
        },
        "required": ["address"]
    })
}

fn tokens_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "chain_id": { "type": "integer", "description": "Optional chain id filter." },
            "limit": { "type": "integer", "description": "Optional page size." },
            "offset": { "type": "integer", "description": "Optional pagination offset." }
        }
    })
}

fn search_tokens_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "query": { "type": "string", "description": "Token symbol/name/address query." },
            "chain_id": { "type": "integer", "description": "Optional chain id filter." },
            "limit": { "type": "integer", "description": "Optional page size." },
            "offset": { "type": "integer", "description": "Optional pagination offset." }
        },
        "required": ["query"]
    })
}

#[derive(Clone)]
pub struct KhalaniRuntime {
    client: KhalaniClient,
}

impl Default for KhalaniRuntime {
    fn default() -> Self {
        let client = KhalaniClient::new().unwrap_or_else(|_| {
            let http = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build fallback HTTP client");
            KhalaniClient {
                http,
                endpoint: DEFAULT_KHALANI_API_ENDPOINT.to_string(),
                api_key: None,
            }
        });
        Self { client }
    }
}

type ToolHandler = fn(&KhalaniRuntime, &str, &str) -> Result<Value, String>;

impl KhalaniRuntime {
    fn route_tool(name: &str) -> Option<ToolHandler> {
        match name {
            "get_khalani_quote" => Some(Self::handle_get_khalani_quote),
            "build_khalani_order" => Some(Self::handle_build_khalani_order),
            "submit_khalani_order" => Some(Self::handle_submit_khalani_order),
            "get_khalani_order_status" => Some(Self::handle_get_khalani_order_status),
            "get_khalani_orders_by_address" => Some(Self::handle_get_khalani_orders_by_address),
            "get_khalani_tokens" => Some(Self::handle_get_khalani_tokens),
            "search_khalani_tokens" => Some(Self::handle_search_khalani_tokens),
            "get_khalani_chains" => Some(Self::handle_get_khalani_chains),
            _ => None,
        }
    }

    fn handle_get_khalani_quote(&self, args_json: &str, ctx_json: &str) -> Result<Value, String> {
        let args: GetKhalaniQuoteArgs = parse_args(args_json)?;
        let ctx = parse_ctx(ctx_json)?;
        let sender_address =
            resolve_sender_address(ctx.user_address, args.sender_address.as_deref())?;
        let to_chain = args
            .destination_chain
            .clone()
            .unwrap_or_else(|| args.chain.clone());
        let amount_base_units =
            self.client
                .quote_amount_base_units(&args.chain, &args.sell_token, args.amount)?;

        self.client.get_quote_khalani(KhalaniQuoteRequest {
            from_chain: &args.chain,
            to_chain: &to_chain,
            from_token: &args.sell_token,
            to_token: &args.buy_token,
            from_amount: &amount_base_units,
            from_address: &sender_address,
            to_address: args.receiver_address.as_deref(),
            slippage_bps: slippage_to_bps(args.slippage),
        })
    }

    fn handle_build_khalani_order(&self, args_json: &str, ctx_json: &str) -> Result<Value, String> {
        let args: BuildKhalaniOrderArgs = parse_args(args_json)?;
        let ctx = parse_ctx(ctx_json)?;
        let sender_address =
            resolve_sender_address(ctx.user_address, args.sender_address.as_deref())?;
        let to_chain = args
            .destination_chain
            .clone()
            .unwrap_or_else(|| args.chain.clone());
        let amount_base_units =
            self.client
                .quote_amount_base_units(&args.chain, &args.sell_token, args.amount)?;

        let quote = self.client.get_quote_khalani(KhalaniQuoteRequest {
            from_chain: &args.chain,
            to_chain: &to_chain,
            from_token: &args.sell_token,
            to_token: &args.buy_token,
            from_amount: &amount_base_units,
            from_address: &sender_address,
            to_address: args.receiver_address.as_deref(),
            slippage_bps: slippage_to_bps(args.slippage),
        })?;

        let quote_entry = normalize_khalani_quote_response(&quote)
            .ok_or_else(|| "Khalani quote response is empty".to_string())?;
        let quote_id = extract_khalani_quote_id(&quote_entry)
            .ok_or_else(|| "Khalani quote missing quoteId".to_string())?;
        let route_id = extract_khalani_route_id(&quote_entry);
        let summary = extract_quote_summary(&quote_entry);

        let slippage_bps = slippage_to_bps(args.slippage);
        let mut build_payloads: Vec<Value> = Vec::new();

        if let Some(ref rid) = route_id {
            for addr_key in ["from", "fromAddress", "userAddress"] {
                let mut payload = json!({
                    addr_key: &sender_address,
                    "quoteId": &quote_id,
                    "routeId": rid,
                    "depositMethod": "CONTRACT_CALL",
                });
                if let Some(bps) = slippage_bps {
                    payload["slippageInBps"] = json!(bps);
                }
                build_payloads.push(payload);
            }
        }

        let mut legacy_payload = json!({
            "quoteId": &quote_id,
            "user": &sender_address,
        });
        if let Some(target) = extract_khalani_allowance_target(&quote_entry) {
            legacy_payload["allowanceTarget"] = Value::String(target);
        }
        if let Some(bps) = slippage_bps {
            legacy_payload["slippageInBps"] = json!(bps);
        }
        build_payloads.push(legacy_payload);

        let mut build: Option<Value> = None;
        let mut last_build_error: Option<String> = None;
        for payload in build_payloads {
            match self.client.build_deposit_khalani(payload) {
                Ok(value) => {
                    build = Some(value);
                    break;
                }
                Err(err) => last_build_error = Some(err),
            }
        }
        let build = build.ok_or_else(|| {
            last_build_error.unwrap_or_else(|| "Khalani deposit build request failed".to_string())
        })?;

        if let Some(approvals) = build.get("approvals").and_then(Value::as_array) {
            let (tx, is_deposit) = approvals
                .iter()
                .find_map(|a| {
                    let tx = extract_khalani_eth_send_tx(a)?;
                    let deposit = a.get("deposit").and_then(Value::as_bool) == Some(true);
                    Some((tx, deposit))
                })
                .ok_or_else(|| {
                    "Khalani approvals present but no executable transaction found".to_string()
                })?;

            let (description, follow_up) = if is_deposit {
                (
                    format!(
                        "Khalani deposit tx for {} {} -> {}",
                        args.amount, args.sell_token, args.buy_token
                    ),
                    json!({
                        "step": "submit_khalani_order",
                        "args_template": {
                            "quote_id": quote_id,
                            "route_id": route_id,
                            "submit_type": "SIGNED_TRANSACTION"
                        }
                    }),
                )
            } else {
                (
                    format!(
                        "Khalani approval tx for {} on {}",
                        args.sell_token, args.chain
                    ),
                    json!({
                        "step": "build_khalani_order",
                        "args_template": {
                            "chain": args.chain,
                            "destination_chain": args.destination_chain,
                            "sell_token": args.sell_token,
                            "buy_token": args.buy_token,
                            "amount": args.amount,
                            "sender_address": sender_address,
                            "receiver_address": args.receiver_address,
                            "slippage": args.slippage,
                        },
                        "reason": "Approval required before deposit."
                    }),
                )
            };

            return build_khalani_result(
                &quote_id,
                &route_id,
                "APPROVAL_FLOW",
                summary,
                "send_transaction_to_wallet",
                build_wallet_tx_request(&tx, description),
                build_transaction_preflight(&tx),
                follow_up,
            );
        }

        let tx_type = extract_khalani_transaction_type(&build);
        if tx_type.eq_ignore_ascii_case("PERMIT2") {
            let typed_data = extract_khalani_typed_data(&build)
                .ok_or_else(|| "Khalani build missing typed data".to_string())?;
            return build_khalani_result(
                &quote_id,
                &route_id,
                &tx_type,
                summary,
                "send_eip712_to_wallet",
                json!({
                    "typed_data": typed_data,
                    "description": format!(
                        "Khalani Permit2 signature for {} {} -> {}",
                        args.amount, args.sell_token, args.buy_token
                    )
                }),
                None,
                json!({
                    "step": "submit_khalani_order",
                    "args_template": {
                        "quote_id": quote_id,
                        "route_id": route_id,
                        "submit_type": "SIGNED_EIP712"
                    }
                }),
            );
        }

        let tx = extract_khalani_contract_call_tx(&build)
            .ok_or_else(|| "Khalani build missing executable transaction".to_string())?;
        build_khalani_result(
            &quote_id,
            &route_id,
            &tx_type,
            summary,
            "send_transaction_to_wallet",
            build_wallet_tx_request(
                &tx,
                format!(
                    "Khalani swap {} {} to {} on {}",
                    args.amount, args.sell_token, args.buy_token, args.chain
                ),
            ),
            build_transaction_preflight(&tx),
            json!({
                "step": "submit_khalani_order",
                "args_template": {
                    "quote_id": quote_id,
                    "route_id": route_id,
                    "submit_type": "SIGNED_TRANSACTION"
                }
            }),
        )
    }

    fn handle_submit_khalani_order(
        &self,
        args_json: &str,
        _ctx_json: &str,
    ) -> Result<Value, String> {
        let args: SubmitKhalaniOrderArgs = parse_args(args_json)?;
        let submit_type = args.submit_type.to_uppercase();
        let payload = match submit_type.as_str() {
            "SIGNED_EIP712" => {
                let signature = args.signature.clone().ok_or_else(|| {
                    "submit_khalani_order requires signature for SIGNED_EIP712".to_string()
                })?;
                if let Some(route_id) = args.route_id.clone() {
                    json!({
                        "quoteId": args.quote_id,
                        "routeId": route_id,
                        "signature": signature,
                    })
                } else {
                    json!({
                        "quoteId": args.quote_id,
                        "submittedData": {
                            "type": "SIGNED_EIP712",
                            "signature": signature,
                        }
                    })
                }
            }
            "SIGNED_TRANSACTION" => {
                let tx_hash = args.transaction_hash.clone().ok_or_else(|| {
                    "submit_khalani_order requires transaction_hash for SIGNED_TRANSACTION"
                        .to_string()
                })?;
                if let Some(route_id) = args.route_id.clone() {
                    json!({
                        "quoteId": args.quote_id,
                        "routeId": route_id,
                        "txHash": tx_hash,
                        "transactionHash": tx_hash,
                    })
                } else {
                    json!({
                        "quoteId": args.quote_id,
                        "submittedData": {
                            "type": "SIGNED_TRANSACTION",
                            "transactionHash": tx_hash,
                        }
                    })
                }
            }
            other => {
                return Err(format!(
                    "submit_khalani_order unsupported submit_type '{other}'"
                ));
            }
        };

        self.client.submit_deposit_khalani(payload)
    }

    fn handle_get_khalani_order_status(
        &self,
        args_json: &str,
        _ctx_json: &str,
    ) -> Result<Value, String> {
        let args: GetKhalaniOrderStatusArgs = parse_args(args_json)?;
        self.client.get_order_khalani(&args.order_id)
    }

    fn handle_get_khalani_orders_by_address(
        &self,
        args_json: &str,
        _ctx_json: &str,
    ) -> Result<Value, String> {
        let args: GetKhalaniOrdersByAddressArgs = parse_args(args_json)?;
        self.client.get_orders_by_address_khalani(
            &args.address,
            args.status.as_deref(),
            args.limit,
            args.offset,
        )
    }

    fn handle_get_khalani_tokens(&self, args_json: &str, _ctx_json: &str) -> Result<Value, String> {
        let args: GetKhalaniTokensArgs = parse_args(args_json)?;
        self.client
            .get_tokens_khalani(args.chain_id, args.limit, args.offset, None)
    }

    fn handle_search_khalani_tokens(
        &self,
        args_json: &str,
        _ctx_json: &str,
    ) -> Result<Value, String> {
        let args: SearchKhalaniTokensArgs = parse_args(args_json)?;
        self.client
            .search_tokens_khalani(&args.query, args.chain_id, args.limit, args.offset)
    }

    fn handle_get_khalani_chains(&self, args_json: &str, _ctx_json: &str) -> Result<Value, String> {
        let _: GetKhalaniChainsArgs = parse_args(args_json)?;
        self.client.get_chains_khalani()
    }
}

impl DynRuntime for KhalaniRuntime {
    fn manifest(&self) -> DynManifest {
        DynManifest {
            abi_version: DYN_ABI_VERSION,
            name: "khalani_dyn".into(),
            version: "0.1.0".into(),
            preamble: "You are the Khalani Agent. Use Khalani tools for quotes, build, submit, status, and metadata. Respect SYSTEM_NEXT_ACTION sequencing when present.".into(),
            model_preference: DynModelPreference::default(),
            tools: vec![
                DynToolDescriptor {
                    name: "get_khalani_quote".into(),
                    namespace: "khalani_dyn".into(),
                    description: "Fetch a Khalani quote for same-chain or cross-chain swap routes.".into(),
                    parameters_schema: quote_schema(),
                    is_async: false,
                },
                DynToolDescriptor {
                    name: "build_khalani_order".into(),
                    namespace: "khalani_dyn".into(),
                    description: "Build a Khalani execution step and return explicit next wallet actions.".into(),
                    parameters_schema: quote_schema(),
                    is_async: false,
                },
                DynToolDescriptor {
                    name: "submit_khalani_order".into(),
                    namespace: "khalani_dyn".into(),
                    description: "Submit a wallet-completed Khalani order using tx hash or EIP-712 signature.".into(),
                    parameters_schema: submit_schema(),
                    is_async: false,
                },
                DynToolDescriptor {
                    name: "get_khalani_order_status".into(),
                    namespace: "khalani_dyn".into(),
                    description: "Fetch a Khalani order by order_id.".into(),
                    parameters_schema: order_status_schema(),
                    is_async: false,
                },
                DynToolDescriptor {
                    name: "get_khalani_orders_by_address".into(),
                    namespace: "khalani_dyn".into(),
                    description: "List Khalani orders by wallet address.".into(),
                    parameters_schema: orders_by_address_schema(),
                    is_async: false,
                },
                DynToolDescriptor {
                    name: "get_khalani_tokens".into(),
                    namespace: "khalani_dyn".into(),
                    description: "List Khalani supported tokens.".into(),
                    parameters_schema: tokens_schema(),
                    is_async: false,
                },
                DynToolDescriptor {
                    name: "search_khalani_tokens".into(),
                    namespace: "khalani_dyn".into(),
                    description: "Search Khalani tokens by query.".into(),
                    parameters_schema: search_tokens_schema(),
                    is_async: false,
                },
                DynToolDescriptor {
                    name: "get_khalani_chains".into(),
                    namespace: "khalani_dyn".into(),
                    description: "List Khalani supported chains.".into(),
                    parameters_schema: json!({ "type": "object", "properties": {} }),
                    is_async: false,
                },
            ],
        }
    }

    fn execute_tool(&self, name: &str, args_json: &str, ctx_json: &str) -> DynResult {
        let result = match Self::route_tool(name) {
            Some(handler) => handler(self, args_json, ctx_json),
            None => Err(format!("unknown tool: {name}")),
        };

        match result {
            Ok(value) => DynResult::ok(value),
            Err(err) => DynResult::err(err),
        }
    }
}

declare_dyn!(KhalaniRuntime);
