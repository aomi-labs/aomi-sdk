use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct KhalaniApp;

pub(crate) use crate::tool::*;

// ============================================================================
// Khalani HTTP Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_KHALANI_API: &str = "https://api.hyperstream.dev";
pub(crate) const SYSTEM_NEXT_ACTION_KEY: &str = "SYSTEM_NEXT_ACTION";

#[derive(Debug, Clone)]
pub(crate) struct ResolvedKhalaniToken {
    pub(crate) address: String,
    pub(crate) decimals: u8,
}

#[derive(Clone)]
pub(crate) struct KhalaniClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_endpoint: String,
}

impl KhalaniClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[khalani] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_endpoint: std::env::var("KHALANI_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_KHALANI_API.to_string()),
        })
    }

    pub(crate) fn send_json(
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

    pub(crate) fn with_source(value: Value) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert("source".to_string(), Value::String("khalani".to_string()));
                Value::Object(map)
            }
            other => json!({
                "source": "khalani",
                "data": other,
            }),
        }
    }

    // ---- Quote ----
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn get_quote(
        &self,
        from_chain_id: u64,
        to_chain_id: u64,
        from_token: &str,
        to_token: &str,
        amount: &str,
        sender_address: &str,
        receiver_address: Option<&str>,
        slippage_bps: Option<u32>,
    ) -> Result<Value, String> {
        let mut payload = json!({
            "tradeType": "EXACT_INPUT",
            "fromChainId": from_chain_id,
            "toChainId": to_chain_id,
            "fromToken": from_token,
            "toToken": to_token,
            "amount": amount,
            "fromAddress": sender_address,
        });
        if let Some(receiver) = receiver_address {
            payload["receiver"] = Value::String(receiver.to_string());
        }
        if let Some(bps) = slippage_bps {
            payload["slippageInBps"] = json!(bps);
        }

        let request = self
            .http
            .post(format!("{}/v1/quotes", self.api_endpoint))
            .json(&payload);
        let value = Self::send_json(request, "get quote")?;
        Ok(Self::with_source(value))
    }

    // ---- Build Deposit ----
    pub(crate) fn build_deposit(&self, payload: Value) -> Result<Value, String> {
        let request = self
            .http
            .post(format!("{}/v1/deposit/build", self.api_endpoint))
            .json(&payload);
        Self::send_json(request, "build deposit")
    }

    // ---- Submit Deposit ----
    pub(crate) fn submit_deposit(&self, payload: Value) -> Result<Value, String> {
        let request = self
            .http
            .put(format!("{}/v1/deposit/submit", self.api_endpoint))
            .json(&payload);
        let value = Self::send_json(request, "submit deposit")?;
        Ok(Self::with_source(value))
    }

    // ---- Orders by Address ----
    pub(crate) fn get_orders_by_address(
        &self,
        address: &str,
        status: Option<&str>,
        limit: Option<u32>,
        offset: Option<u32>,
        order_ids: Option<&str>,
    ) -> Result<Value, String> {
        let mut request = self
            .http
            .get(format!("{}/v1/orders/{}", self.api_endpoint, address));

        let mut query_params: Vec<(&str, String)> = Vec::new();
        if let Some(s) = status {
            query_params.push(("status", s.to_string()));
        }
        if let Some(l) = limit {
            query_params.push(("limit", l.to_string()));
        }
        if let Some(o) = offset {
            query_params.push(("offset", o.to_string()));
        }
        if let Some(ids) = order_ids {
            query_params.push(("orderIds", ids.to_string()));
        }
        if !query_params.is_empty() {
            request = request.query(&query_params);
        }

        let value = Self::send_json(request, "get orders by address")?;
        Ok(Self::with_source(value))
    }

    // ---- Tokens ----
    pub(crate) fn get_tokens(
        &self,
        chain_id: Option<u64>,
        limit: Option<u32>,
        offset: Option<u32>,
        query: Option<&str>,
    ) -> Result<Value, String> {
        let mut request = self.http.get(format!("{}/v1/tokens", self.api_endpoint));

        let mut query_params: Vec<(&str, String)> = Vec::new();
        if let Some(cid) = chain_id {
            query_params.push(("chainIds", cid.to_string()));
        }
        if let Some(l) = limit {
            query_params.push(("limit", l.to_string()));
        }
        if let Some(o) = offset {
            query_params.push(("offset", o.to_string()));
        }
        if let Some(q) = query {
            query_params.push(("q", q.to_string()));
        }
        if !query_params.is_empty() {
            request = request.query(&query_params);
        }

        let value = Self::send_json(request, "get tokens")?;
        Ok(Self::with_source(value))
    }

    // ---- Search Tokens ----
    pub(crate) fn search_tokens(
        &self,
        query: &str,
        chain_id: Option<u64>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Value, String> {
        let mut request = self
            .http
            .get(format!("{}/v1/tokens/search", self.api_endpoint))
            .query(&[("q", query)]);

        let mut query_params: Vec<(&str, String)> = Vec::new();
        if let Some(cid) = chain_id {
            query_params.push(("chainIds", cid.to_string()));
        }
        if let Some(l) = limit {
            query_params.push(("limit", l.to_string()));
        }
        if let Some(o) = offset {
            query_params.push(("offset", o.to_string()));
        }
        if !query_params.is_empty() {
            request = request.query(&query_params);
        }

        let value = Self::send_json(request, "search tokens")?;
        Ok(Self::with_source(value))
    }

    // ---- Chains ----
    pub(crate) fn get_chains(&self) -> Result<Value, String> {
        let request = self.http.get(format!("{}/v1/chains", self.api_endpoint));
        let value = Self::send_json(request, "get chains")?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn resolve_token(
        &self,
        token: &str,
        chain_id: u64,
    ) -> Result<ResolvedKhalaniToken, String> {
        let token = token.trim();
        if token.is_empty() {
            return Err("token cannot be empty".to_string());
        }

        let results = self.search_tokens(token, Some(chain_id), Some(50), Some(0))?;
        let data = results
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("Khalani token search returned no token list for '{token}'"))?;

        let token_lower = token.to_ascii_lowercase();
        let mut best: Option<(i32, ResolvedKhalaniToken)> = None;
        for candidate in data {
            let Some(candidate_chain_id) = candidate.get("chainId").and_then(Value::as_u64) else {
                continue;
            };
            if candidate_chain_id != chain_id {
                continue;
            }

            let Some(address) = candidate.get("address").and_then(Value::as_str) else {
                continue;
            };
            let decimals = candidate
                .get("decimals")
                .and_then(Value::as_u64)
                .and_then(|v| u8::try_from(v).ok())
                .unwrap_or(18);
            let symbol = candidate
                .get("symbol")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let name = candidate
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();
            let address_lower = address.to_ascii_lowercase();

            let score = if is_hex_address(token) && address_lower == token_lower {
                100
            } else if symbol == token_lower {
                95
            } else if name == token_lower {
                85
            } else if symbol.starts_with(&token_lower) {
                60
            } else if name.contains(&token_lower) {
                40
            } else {
                0
            };

            if score == 0 {
                continue;
            }

            let resolved = ResolvedKhalaniToken {
                address: address_lower,
                decimals,
            };

            if best
                .as_ref()
                .is_none_or(|(best_score, _)| score > *best_score)
            {
                best = Some((score, resolved));
            }
        }

        best.map(|(_, resolved)| resolved).ok_or_else(|| {
            format!(
                "Unable to resolve token '{token}' on chain {chain_id} via Khalani token search"
            )
        })
    }
}

// ============================================================================
// Shared Helpers
// ============================================================================

pub(crate) fn resolve_sender_address(
    ctx: &DynToolCallCtx,
    provided: Option<&str>,
) -> Result<String, String> {
    provided
        .map(ToString::to_string)
        .or_else(|| ctx.attribute_string(&["user_address"]))
        .ok_or_else(|| "No connected wallet address found in context".to_string())
}

pub(crate) fn slippage_to_bps(slippage: Option<f64>) -> Option<u32> {
    slippage.map(|s| (s * 10_000.0).round()).and_then(|bps| {
        if bps.is_finite() && bps >= 0.0 {
            Some(bps as u32)
        } else {
            None
        }
    })
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

pub(crate) fn resolve_chain_id(chain: &str) -> Result<u64, String> {
    let raw = chain.trim();
    if raw.is_empty() {
        return Err("chain cannot be empty".to_string());
    }
    if let Ok(value) = raw.parse::<u64>() {
        return Ok(value);
    }

    let normalized: String = raw
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect();

    match normalized.as_str() {
        "ethereum" | "mainnet" => Ok(1),
        "base" => Ok(8453),
        "polygon" | "matic" | "polygonpos" => Ok(137),
        "arbitrum" | "arbitrumone" => Ok(42161),
        "optimism" | "op" | "opmainnet" => Ok(10),
        "bsc" | "binance" | "binancesmartchain" => Ok(56),
        "avalanche" | "avax" | "avalanchecchain" => Ok(43114),
        _ => Err(format!(
            "Unknown chain '{chain}'. Provide a numeric chain ID or a supported alias like ethereum, base, polygon, arbitrum, optimism, bsc, or avalanche."
        )),
    }
}

pub(crate) fn is_hex_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && value[2..].chars().all(|ch| ch.is_ascii_hexdigit())
}

// ============================================================================
// Quote normalization helpers
// ============================================================================

pub(crate) fn normalize_khalani_quote_response(value: &Value) -> Option<Value> {
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

pub(crate) fn extract_khalani_quote_id(value: &Value) -> Option<String> {
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

pub(crate) fn extract_khalani_allowance_target(value: &Value) -> Option<String> {
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

pub(crate) fn extract_khalani_route_id(value: &Value) -> Option<String> {
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

pub(crate) fn hex_to_decimal_wei(s: &str) -> String {
    let hex = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X"));
    match hex {
        Some(h) => u128::from_str_radix(h, 16)
            .map(|v| v.to_string())
            .unwrap_or_else(|_| s.to_string()),
        None => s.to_string(),
    }
}

pub(crate) fn normalize_tx_fields(tx: &Value) -> Option<Value> {
    let to = tx.get("to").and_then(Value::as_str)?;
    let raw_value = tx.get("value").and_then(Value::as_str).unwrap_or("0");
    Some(json!({
        "to": to,
        "data": tx.get("data").and_then(Value::as_str).unwrap_or("0x"),
        "value": hex_to_decimal_wei(raw_value),
        "gas_limit": tx.get("gasLimit").or_else(|| tx.get("gas")).cloned().unwrap_or(Value::Null),
    }))
}

pub(crate) fn extract_khalani_eth_send_tx(approval: &Value) -> Option<Value> {
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

pub(crate) fn extract_khalani_transaction_type(build: &Value) -> String {
    build
        .get("transaction")
        .and_then(|v| v.get("type"))
        .and_then(Value::as_str)
        .or_else(|| build.get("type").and_then(Value::as_str))
        .unwrap_or("CONTRACT_CALL")
        .to_string()
}

pub(crate) fn extract_khalani_contract_call_tx(build: &Value) -> Option<Value> {
    let tx = build.get("transaction").unwrap_or(build);
    normalize_tx_fields(tx).or_else(|| tx.get("tx").and_then(normalize_tx_fields))
}

pub(crate) fn extract_khalani_typed_data(build: &Value) -> Option<Value> {
    let tx = build.get("transaction").unwrap_or(build);
    tx.get("typedData")
        .cloned()
        .or_else(|| tx.get("eip712").cloned())
        .or_else(|| tx.get("payload").cloned())
        .or_else(|| build.get("typedData").cloned())
        .filter(Value::is_object)
}

pub(crate) fn build_transaction_preflight(tx: &Value) -> Option<Value> {
    let to = tx.get("to").and_then(Value::as_str)?;
    let data = tx.get("data").and_then(Value::as_str)?;
    let payload = data.strip_prefix("0x")?;

    // Emit preflight only for approve(address,uint256) calldata
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

pub(crate) fn extract_quote_summary(quote_entry: &Value) -> Value {
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

pub(crate) fn build_wallet_tx_request(tx: &Value, description: String) -> Value {
    json!({
        "to": tx.get("to").cloned().unwrap_or(Value::Null),
        "value": tx.get("value").cloned().unwrap_or_else(|| Value::String("0".to_string())),
        "data": tx.get("data").cloned().unwrap_or_else(|| Value::String("0x".to_string())),
        "gas_limit": tx.get("gas_limit").cloned().unwrap_or(Value::Null),
        "description": description,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_common_chain_aliases() {
        assert_eq!(resolve_chain_id("ethereum").unwrap(), 1);
        assert_eq!(resolve_chain_id("base").unwrap(), 8453);
        assert_eq!(resolve_chain_id("polygon").unwrap(), 137);
        assert_eq!(resolve_chain_id("42161").unwrap(), 42161);
    }

    #[test]
    fn detects_hex_addresses() {
        assert!(is_hex_address("0x5d907bea404e6f821d467314a9ca07663cf64c9b"));
        assert!(!is_hex_address("ETH"));
        assert!(!is_hex_address("0x1234"));
    }
}

// ============================================================================
// SYSTEM_NEXT_ACTION types and builder
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum NextAction {
    ToolCalls(Vec<NextActionTool>),
    #[allow(dead_code)]
    Instructions(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct NextActionTool {
    pub(crate) name: String,
    pub(crate) reason: String,
    pub(crate) args: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) condition: Option<String>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_khalani_result(
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

    if let Some(ref pf) = preflight
        && let Some(name) = pf.get("tool").and_then(Value::as_str)
    {
        tool_calls.push(NextActionTool {
            name: name.to_string(),
            reason: "Run preflight checks before wallet interaction.".to_string(),
            args: pf.get("args").cloned().unwrap_or_default(),
            condition: None,
        });
    }

    tool_calls.push(NextActionTool {
        name: wallet_tool.to_string(),
        reason: "REQUIRED: Call this tool with these exact args. \
                 Do NOT skip or assume it was already sent."
            .to_string(),
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
                "After wallet callback reports signature success; \
                 include signature from callback."
                    .to_string(),
            ),
            "submit_khalani_order" => Some(
                "After wallet callback reports transaction success; \
                 include transaction_hash from callback."
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
        .map_err(|e| format!("Failed to serialize SYSTEM_NEXT_ACTION: {e}"))?;

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

// ============================================================================
// Tool 1: get_khalani_quote
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetKhalaniQuoteArgs {
    /// Source chain: ethereum, arbitrum, polygon, base, etc.
    pub(crate) chain: String,
    /// Destination chain for cross-chain routes. Defaults to the source chain.
    pub(crate) destination_chain: Option<String>,
    /// Token to swap from.
    pub(crate) sell_token: String,
    /// Token to swap to.
    pub(crate) buy_token: String,
    /// Human-readable sell amount.
    pub(crate) amount: f64,
    /// Sender/taker wallet address. Defaults to the connected wallet.
    pub(crate) sender_address: Option<String>,
    /// Recipient wallet address. Defaults to sender.
    pub(crate) receiver_address: Option<String>,
    /// Slippage decimal (0.005 = 0.5%).
    pub(crate) slippage: Option<f64>,
}

pub(crate) struct GetKhalaniQuote;
