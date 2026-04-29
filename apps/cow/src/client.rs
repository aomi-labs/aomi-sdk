use alloy_dyn_abi::eip712::TypedData;
use alloy_primitives::{Signature, U256};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct CowApp;

#[derive(Clone, Debug)]
pub(crate) struct PendingCowQuote {
    pub(crate) session_id: String,
    pub(crate) chain: String,
    pub(crate) signed_order: Value,
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedCowOrderSubmission {
    pub(crate) chain: String,
    pub(crate) signed_order: Value,
}

static PENDING_COW_QUOTES: OnceLock<Mutex<HashMap<String, PendingCowQuote>>> = OnceLock::new();

fn pending_cow_quotes() -> &'static Mutex<HashMap<String, PendingCowQuote>> {
    PENDING_COW_QUOTES.get_or_init(|| Mutex::new(HashMap::new()))
}

// ============================================================================
// CoW HTTP Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_COW_ENDPOINT: &str = "https://api.cow.fi/mainnet";
pub(crate) const COW_SETTLEMENT_CONTRACT: &str = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41";

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

    pub(crate) fn send_json(
        request: reqwest::blocking::RequestBuilder,
        operation: &str,
    ) -> Result<Value, String> {
        let response = request
            .send()
            .map_err(|e| format!("[cow] {operation} request failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[cow] {operation} request failed: {status} {body}"));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[cow] {operation} decode failed: {e}; body: {body}"))
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

    pub(crate) fn get_quote(&self, chain: &str, payload: Value) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self.http.post(format!("{base}/quote")).json(&payload);
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }

        let value = Self::send_json(request, "quote")?;
        Ok(with_source(value))
    }

    pub(crate) fn place_order(&self, chain: &str, payload: Value) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self.http.post(format!("{base}/orders")).json(&payload);
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }

        let value = Self::send_json(request, "post order")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_order(&self, chain: &str, uid: &str) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self.http.get(format!("{base}/orders/{uid}"));
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let value = Self::send_json(request, "get order")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_order_status(&self, chain: &str, uid: &str) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self.http.get(format!("{base}/orders/{uid}/status"));
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let value = Self::send_json(request, "get order status")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_user_orders(
        &self,
        chain: &str,
        owner: &str,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut url = format!("{base}/account/{owner}/orders");
        let mut params = Vec::new();
        if let Some(offset) = offset {
            params.push(format!("offset={offset}"));
        }
        if let Some(limit) = limit {
            params.push(format!("limit={limit}"));
        }
        if !params.is_empty() {
            url = format!("{url}?{}", params.join("&"));
        }
        let mut request = self.http.get(url);
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let value = Self::send_json(request, "get user orders")?;
        Ok(with_source(value))
    }

    pub(crate) fn cancel_orders(&self, chain: &str, payload: Value) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self.http.delete(format!("{base}/orders")).json(&payload);
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let value = Self::send_json(request, "cancel orders")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_trades(
        &self,
        chain: &str,
        owner: Option<&str>,
        order_uid: Option<&str>,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        // Trades endpoint is v2 — replace /v1 suffix with /v2
        let base_v2 = if let Some(prefix) = base.strip_suffix("/v1") {
            format!("{prefix}/v2")
        } else {
            base
        };
        let mut params = Vec::new();
        if let Some(owner) = owner {
            params.push(format!("owner={owner}"));
        }
        if let Some(order_uid) = order_uid {
            params.push(format!("orderUid={order_uid}"));
        }
        if let Some(offset) = offset {
            params.push(format!("offset={offset}"));
        }
        if let Some(limit) = limit {
            params.push(format!("limit={limit}"));
        }
        let url = if params.is_empty() {
            format!("{base_v2}/trades")
        } else {
            format!("{base_v2}/trades?{}", params.join("&"))
        };
        let mut request = self.http.get(url);
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let value = Self::send_json(request, "get trades")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_native_price(
        &self,
        chain: &str,
        token_address: &str,
    ) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self
            .http
            .get(format!("{base}/token/{token_address}/native_price"));
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let value = Self::send_json(request, "get native price")?;
        Ok(with_source(value))
    }

    pub(crate) fn get_orders_by_tx(&self, chain: &str, tx_hash: &str) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self
            .http
            .get(format!("{base}/transactions/{tx_hash}/orders"));
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let value = Self::send_json(request, "get orders by tx")?;
        Ok(with_source(value))
    }

    pub(crate) fn debug_order(&self, chain: &str, uid: &str) -> Result<Value, String> {
        let base = self.cow_api_base_for_chain(chain)?;
        let mut request = self.http.get(format!("{base}/debug/order/{uid}"));
        if let Some(api_key) = self.cow_api_key.as_ref() {
            request = request.header("Authorization", format!("Bearer {api_key}"));
        }
        let value = Self::send_json(request, "debug order")?;
        Ok(with_source(value))
    }
}

// ============================================================================
// Shared helpers
// ============================================================================

pub(crate) fn with_source(value: Value) -> Value {
    match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("cow".to_string()));
            Value::Object(map)
        }
        other => json!({
            "source": "cow",
            "data": other,
        }),
    }
}

pub(crate) fn canonicalize_quote_order(quote_response: &Value) -> Result<Value, String> {
    let quote = quote_response
        .get("quote")
        .and_then(Value::as_object)
        .ok_or_else(|| "[cow] quote response missing quote object".to_string())?;

    let mut order = serde_json::Map::new();
    for field in [
        "sellToken",
        "buyToken",
        "receiver",
        "sellAmount",
        "buyAmount",
        "validTo",
        "appData",
        "feeAmount",
        "kind",
        "partiallyFillable",
        "sellTokenBalance",
        "buyTokenBalance",
    ] {
        let value = quote
            .get(field)
            .cloned()
            .ok_or_else(|| format!("[cow] quote missing `{field}`"))?;
        order.insert(field.to_string(), value);
    }

    if order
        .get("receiver")
        .map(|value| value.is_null())
        .unwrap_or(true)
    {
        return Err(
            "[cow] quote receiver is null; request the quote with receiver set to the wallet address"
                .to_string(),
        );
    }

    let quoted_sell_amount = order
        .get("sellAmount")
        .cloned()
        .ok_or_else(|| "[cow] quote missing `sellAmount`".to_string())?;
    let quoted_fee_amount = order
        .get("feeAmount")
        .cloned()
        .ok_or_else(|| "[cow] quote missing `feeAmount`".to_string())?;
    let total_sell_amount = parse_cow_token_amount(&quoted_sell_amount, "sellAmount")?
        .checked_add(parse_cow_token_amount(&quoted_fee_amount, "feeAmount")?)
        .ok_or_else(|| "[cow] quote sellAmount + feeAmount overflowed U256".to_string())?;
    order.insert(
        "sellAmount".to_string(),
        Value::String(total_sell_amount.to_string()),
    );
    order.insert("feeAmount".to_string(), Value::String("0".to_string()));

    Ok(Value::Object(order))
}

fn parse_cow_token_amount(value: &Value, field: &str) -> Result<U256, String> {
    let raw = value
        .as_str()
        .ok_or_else(|| format!("[cow] {field} must be a decimal string"))?;
    U256::from_str(raw).map_err(|e| format!("[cow] invalid {field}: {e}"))
}

pub(crate) fn build_cow_order_typed_data(chain_id: u64, order_to_sign: Value) -> Value {
    json!({
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "version", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" },
            ],
            "Order": [
                { "name": "sellToken", "type": "address" },
                { "name": "buyToken", "type": "address" },
                { "name": "receiver", "type": "address" },
                { "name": "sellAmount", "type": "uint256" },
                { "name": "buyAmount", "type": "uint256" },
                { "name": "validTo", "type": "uint32" },
                { "name": "appData", "type": "bytes32" },
                { "name": "feeAmount", "type": "uint256" },
                { "name": "kind", "type": "string" },
                { "name": "partiallyFillable", "type": "bool" },
                { "name": "sellTokenBalance", "type": "string" },
                { "name": "buyTokenBalance", "type": "string" },
            ],
        },
        "primaryType": "Order",
        "domain": {
            "name": "Gnosis Protocol",
            "version": "v2",
            "chainId": chain_id,
            "verifyingContract": COW_SETTLEMENT_CONTRACT,
        },
        "message": order_to_sign,
    })
}

pub(crate) fn build_cow_wallet_signature_request(
    typed_data: &Value,
    description: &str,
) -> Result<Value, String> {
    let typed_data_json = serde_json::to_string(typed_data)
        .map_err(|e| format!("[cow] failed to serialize typed_data for wallet signing: {e}"))?;
    Ok(json!({
        "typed_data": typed_data_json,
        "description": description,
    }))
}

pub(crate) fn build_cow_signed_order_payload(
    order_to_sign: &Value,
    from: &str,
    signing_scheme: &str,
    orderbook_quote_id: Option<i64>,
) -> Result<Value, String> {
    let mut signed_order = order_to_sign
        .as_object()
        .cloned()
        .ok_or_else(|| "[cow] order_to_sign must be an object".to_string())?;
    signed_order.insert("from".to_string(), Value::String(from.to_string()));
    signed_order.insert(
        "signingScheme".to_string(),
        Value::String(signing_scheme.to_string()),
    );
    if let Some(orderbook_quote_id) = orderbook_quote_id {
        signed_order.insert(
            "quoteId".to_string(),
            Value::Number(orderbook_quote_id.into()),
        );
    }

    Ok(Value::Object(signed_order))
}

pub(crate) fn build_cow_submit_args_template(chain: &str, quote_id: &str) -> Value {
    json!({
        "chain": chain,
        "quote_id": quote_id,
        "signature": Value::Null,
        "signature_source": "wallet callback payload.signature",
        "handoff_note": "After wallet signature success, call place_cow_order immediately with this object and the callback signature. Do not claim success until the tool returns success.",
    })
}

pub(crate) fn build_cow_quote_id(session_id: &str, call_id: &str) -> String {
    format!("cowq:{session_id}:{call_id}")
}

pub(crate) fn store_pending_cow_quote(
    quote_id: &str,
    session_id: &str,
    chain: &str,
    signed_order: &Value,
) -> Result<(), String> {
    let mut pending = pending_cow_quotes()
        .lock()
        .map_err(|_| "[cow] pending quote cache lock poisoned".to_string())?;
    pending.insert(
        quote_id.to_string(),
        PendingCowQuote {
            session_id: session_id.to_string(),
            chain: chain.to_string(),
            signed_order: signed_order.clone(),
        },
    );
    Ok(())
}

fn extract_signature_candidate(signed_order: &Value) -> Option<String> {
    signed_order
        .get("signature")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn resolve_cow_order_submission(
    session_id: &str,
    chain: &str,
    quote_id: Option<&str>,
    signed_order: Value,
    signature: Option<&str>,
) -> Result<ResolvedCowOrderSubmission, String> {
    let callback_signature = signature
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| extract_signature_candidate(&signed_order));

    if let Some(quote_id) = quote_id.map(str::trim).filter(|value| !value.is_empty()) {
        let pending = pending_cow_quotes()
            .lock()
            .map_err(|_| "[cow] pending quote cache lock poisoned".to_string())?
            .get(quote_id)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "[cow] quote_id `{quote_id}` was not found in the pending quote cache; get a fresh quote before signing"
                )
            })?;
        if pending.session_id != session_id {
            return Err(format!(
                "[cow] quote_id `{quote_id}` belongs to a different session; get a fresh quote in the current chat before signing"
            ));
        }
        let cached_signed_order = pending.signed_order;
        let hydrated =
            hydrate_cow_signed_order_payload(cached_signed_order, callback_signature.as_deref())?;
        return Ok(ResolvedCowOrderSubmission {
            chain: pending.chain,
            signed_order: hydrated,
        });
    }

    Ok(ResolvedCowOrderSubmission {
        chain: chain.to_string(),
        signed_order: hydrate_cow_signed_order_payload(
            signed_order,
            callback_signature.as_deref(),
        )?,
    })
}

pub(crate) fn hydrate_cow_signed_order_payload(
    mut signed_order: Value,
    signature: Option<&str>,
) -> Result<Value, String> {
    let signed_order_obj = signed_order
        .as_object_mut()
        .ok_or_else(|| "[cow] signed_order must be an object".to_string())?;

    let nested_signature = signed_order_obj
        .get("signature")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let callback_signature = signature.map(str::trim).filter(|value| !value.is_empty());

    match (nested_signature, callback_signature) {
        (Some(existing), Some(injected)) if !existing.eq_ignore_ascii_case(injected) => {
            return Err(
                "[cow] signature mismatch between signed_order.signature and callback signature"
                    .to_string(),
            );
        }
        (Some(_), _) => {}
        (None, Some(injected)) => {
            signed_order_obj.insert("signature".to_string(), Value::String(injected.to_string()));
        }
        (None, None) => {
            return Err(
                "[cow] signed order is missing signature; use the wallet callback signature from the matching quote"
                    .to_string(),
            );
        }
    }

    Ok(signed_order)
}

pub(crate) fn canonicalize_signed_order_for_signature(
    signed_order: &Value,
) -> Result<Value, String> {
    let signed_order_obj = signed_order
        .as_object()
        .ok_or_else(|| "[cow] signed_order must be an object".to_string())?;

    let mut order = serde_json::Map::new();
    for field in [
        "sellToken",
        "buyToken",
        "receiver",
        "sellAmount",
        "buyAmount",
        "validTo",
        "appData",
        "feeAmount",
        "kind",
        "partiallyFillable",
        "sellTokenBalance",
        "buyTokenBalance",
    ] {
        let value = signed_order_obj
            .get(field)
            .cloned()
            .ok_or_else(|| format!("[cow] signed_order missing `{field}`"))?;
        order.insert(field.to_string(), value);
    }

    Ok(Value::Object(order))
}

pub(crate) fn verify_cow_order_signature(chain: &str, signed_order: &Value) -> Result<(), String> {
    let signed_order_obj = signed_order
        .as_object()
        .ok_or_else(|| "[cow] signed_order must be an object".to_string())?;
    let signing_scheme = signed_order_obj
        .get("signingScheme")
        .and_then(Value::as_str)
        .unwrap_or("eip712");
    if !signing_scheme.eq_ignore_ascii_case("eip712") {
        return Ok(());
    }

    let owner = signed_order_obj
        .get("from")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "[cow] signed_order is missing `from` owner address".to_string())?;
    let signature = signed_order_obj
        .get("signature")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "[cow] signed_order is missing signature".to_string())?;

    let (_, chain_id) = get_chain_info(chain)?;
    let order_to_sign = canonicalize_signed_order_for_signature(signed_order)?;
    let typed_data = build_cow_order_typed_data(chain_id, order_to_sign);
    let typed: TypedData = serde_json::from_value(typed_data)
        .map_err(|e| format!("[cow] invalid typed data reconstructed for verification: {e}"))?;
    let hash = typed
        .eip712_signing_hash()
        .map_err(|e| format!("[cow] failed to hash typed data for verification: {e}"))?;
    let signature = Signature::from_str(signature)
        .map_err(|e| format!("[cow] invalid wallet signature: {e}"))?;
    let recovered = signature
        .recover_address_from_prehash(&hash)
        .map_err(|e| format!("[cow] failed to recover signer from signature: {e}"))?;

    if !format!("{recovered:#x}").eq_ignore_ascii_case(owner) {
        return Err(format!(
            "[cow] signature does not match signed_order.from; recovered {recovered:#x} but expected {owner}. Use the exact submit_args_template and matching wallet callback signature from the same quote."
        ));
    }

    Ok(())
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
        "gnosis" | "xdai" => Ok(("gnosis", 100)),
        "sepolia" => Ok(("ethereum", 11155111)),
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
    /// Opaque quote ID returned by get_cow_swap_quote. When provided, the app
    /// resolves the exact cached CoW order template for this session.
    pub(crate) quote_id: Option<String>,
    /// Signed order payload to submit to CoW /orders endpoint. Optional when quote_id is provided.
    #[serde(default)]
    pub(crate) signed_order: Value,
    /// Wallet callback signature. When provided, fills signed_order.signature if absent.
    pub(crate) signature: Option<String>,
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
    use serde_json::json;

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
        assert_eq!(res.get("source").and_then(Value::as_str), Some("cow"));
        assert!(res.get("price").is_some(), "should have price key");
    }

    #[test]
    fn native_price_usdc_smoke() {
        let res = client()
            .get_native_price("ethereum", "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
            .expect("should get USDC native price");
        assert_eq!(res.get("source").and_then(Value::as_str), Some("cow"));
        let price = res.get("price").and_then(Value::as_f64);
        assert!(price.is_some(), "should have numeric price");
    }

    #[test]
    fn user_orders_no_orders_smoke() {
        // Using a random address that likely has no orders — should return empty array
        let res = client().get_user_orders(
            "ethereum",
            "0x0000000000000000000000000000000000000001",
            None,
            Some(1),
        );
        // Either empty array or an error is acceptable
        if let Ok(val) = res {
            assert_eq!(val.get("source").and_then(Value::as_str), Some("cow"));
        }
    }

    #[test]
    fn get_trades_v2_requires_filter() {
        // Calling without owner or order_uid should fail at the tool layer (not here),
        // but the API itself might return an error or empty result
        let res = client().get_trades("ethereum", None, None, None, Some(1));
        // v2 trades endpoint might require a filter — either works or returns error
        if let Ok(val) = res {
            assert_eq!(val.get("source").and_then(Value::as_str), Some("cow"));
        }
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

    #[test]
    fn canonicalize_quote_order_requires_receiver() {
        let quote = json!({
            "quote": {
                "sellToken": "0x1",
                "buyToken": "0x2",
                "receiver": null,
                "sellAmount": "1",
                "buyAmount": "2",
                "validTo": 1,
                "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "feeAmount": "0",
                "kind": "sell",
                "partiallyFillable": false,
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20"
            }
        });

        let err = canonicalize_quote_order(&quote).expect_err("null receiver should be rejected");
        assert!(err.contains("receiver is null"));
    }

    #[test]
    fn canonicalize_quote_order_rolls_fee_into_sell_amount() {
        let quote = json!({
            "quote": {
                "sellToken": "0x1",
                "buyToken": "0x2",
                "receiver": "0x0000000000000000000000000000000000000003",
                "sellAmount": "100",
                "buyAmount": "2",
                "validTo": 1,
                "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "feeAmount": "7",
                "kind": "sell",
                "partiallyFillable": false,
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20"
            }
        });

        let order = canonicalize_quote_order(&quote).expect("quote should canonicalize");
        assert_eq!(order["sellAmount"].as_str(), Some("107"));
        assert_eq!(order["feeAmount"].as_str(), Some("0"));
    }

    #[test]
    fn build_cow_order_typed_data_uses_canonical_domain() {
        let typed_data = build_cow_order_typed_data(
            137,
            json!({
                "sellToken": "0x1",
                "buyToken": "0x2",
                "receiver": "0x0000000000000000000000000000000000000003",
                "sellAmount": "1",
                "buyAmount": "2",
                "validTo": 1,
                "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "feeAmount": "0",
                "kind": "sell",
                "partiallyFillable": false,
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20"
            }),
        );

        assert_eq!(
            typed_data["domain"]["verifyingContract"].as_str(),
            Some(COW_SETTLEMENT_CONTRACT)
        );
        assert_eq!(typed_data["domain"]["chainId"].as_u64(), Some(137));
    }

    #[test]
    fn build_signed_order_payload_includes_quote_id_and_signing_scheme() {
        let signed_order = build_cow_signed_order_payload(
            &json!({
                "sellToken": "0x1",
                "buyToken": "0x2",
                "receiver": "0x0000000000000000000000000000000000000003",
                "sellAmount": "1",
                "buyAmount": "2",
                "validTo": 1,
                "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "feeAmount": "0",
                "kind": "sell",
                "partiallyFillable": false,
                "sellTokenBalance": "erc20",
                "buyTokenBalance": "erc20"
            }),
            "0x0000000000000000000000000000000000000004",
            "eip712",
            Some(31),
        )
        .expect("signed order should build");

        assert!(signed_order.get("signature").is_none());
        assert_eq!(signed_order["quoteId"].as_u64(), Some(31));
        assert_eq!(
            signed_order["signingScheme"].as_str(),
            Some("eip712")
        );
    }

    #[test]
    fn build_submit_args_template_is_compact() {
        let template = build_cow_submit_args_template("polygon", "cowq:test:1");

        assert_eq!(template["chain"].as_str(), Some("polygon"));
        assert_eq!(template["quote_id"].as_str(), Some("cowq:test:1"));
        assert!(template["signature"].is_null());
        assert_eq!(
            template["signature_source"].as_str(),
            Some("wallet callback payload.signature")
        );
        assert!(
            template["handoff_note"]
                .as_str()
                .expect("handoff note should be present")
                .contains("Do not claim success until the tool returns success.")
        );
        assert!(template.get("signed_order").is_none());
    }

    #[test]
    fn build_wallet_signature_request_uses_stringified_typed_data() {
        let typed_data = json!({
            "domain": {"name": "Gnosis Protocol", "version": "v2", "chainId": 137, "verifyingContract": COW_SETTLEMENT_CONTRACT},
            "types": {"EIP712Domain": [], "Order": []},
            "primaryType": "Order",
            "message": {"sellToken": "0x1"}
        });

        let request = build_cow_wallet_signature_request(&typed_data, "Sign CoW order")
            .expect("wallet signature request should build");
        let typed_data_json = request["typed_data"]
            .as_str()
            .expect("typed_data should be a string");
        let reparsed: Value =
            serde_json::from_str(typed_data_json).expect("typed_data string should be valid json");
        assert_eq!(reparsed, typed_data);
    }

    #[test]
    fn hydrate_signed_order_payload_uses_callback_signature_when_nested_is_missing() {
        let signed_order = json!({
            "sellToken": "0x1",
            "signature": null
        });
        let hydrated = hydrate_cow_signed_order_payload(signed_order, Some("0xabc"))
            .expect("callback signature should hydrate payload");
        assert_eq!(hydrated["signature"].as_str(), Some("0xabc"));
    }

    #[test]
    fn hydrate_signed_order_payload_rejects_conflicting_signatures() {
        let signed_order = json!({
            "sellToken": "0x1",
            "signature": "0xabc"
        });
        let err = hydrate_cow_signed_order_payload(signed_order, Some("0xdef"))
            .expect_err("conflicting signatures should fail");
        assert!(err.contains("signature mismatch"));
    }

    #[test]
    fn verify_cow_order_signature_accepts_matching_owner() {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};

        let signer = PrivateKeySigner::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000000001",
        )
        .expect("fixture private key should parse");
        let owner = format!("{:#x}", signer.address());
        let order_to_sign = json!({
            "sellToken": "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
            "buyToken": "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619",
            "receiver": owner,
            "sellAmount": "100000",
            "buyAmount": "1",
            "validTo": 1777397782u64,
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount": "1",
            "kind": "sell",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20"
        });
        let typed_data = build_cow_order_typed_data(137, order_to_sign.clone());
        let typed: TypedData =
            serde_json::from_value(typed_data).expect("typed data should deserialize");
        let hash = typed
            .eip712_signing_hash()
            .expect("typed data hash should build");
        let signature = signer
            .sign_hash_sync(&hash)
            .expect("typed data should sign")
            .to_string();
        let signed_order = json!({
            "sellToken": order_to_sign["sellToken"],
            "buyToken": order_to_sign["buyToken"],
            "receiver": order_to_sign["receiver"],
            "sellAmount": order_to_sign["sellAmount"],
            "buyAmount": order_to_sign["buyAmount"],
            "validTo": order_to_sign["validTo"],
            "appData": order_to_sign["appData"],
            "feeAmount": order_to_sign["feeAmount"],
            "kind": order_to_sign["kind"],
            "partiallyFillable": order_to_sign["partiallyFillable"],
            "sellTokenBalance": order_to_sign["sellTokenBalance"],
            "buyTokenBalance": order_to_sign["buyTokenBalance"],
            "from": format!("{:#x}", signer.address()),
            "signingScheme": "eip712",
            "signature": signature,
        });

        verify_cow_order_signature("polygon", &signed_order)
            .expect("matching signature should verify");
    }

    #[test]
    fn verify_cow_order_signature_rejects_wrong_owner() {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};

        let signer = PrivateKeySigner::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000000001",
        )
        .expect("fixture private key should parse");
        let order_to_sign = json!({
            "sellToken": "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
            "buyToken": "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619",
            "receiver": "0x0000000000000000000000000000000000000002",
            "sellAmount": "100000",
            "buyAmount": "1",
            "validTo": 1777397782u64,
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount": "1",
            "kind": "sell",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20"
        });
        let typed_data = build_cow_order_typed_data(137, order_to_sign.clone());
        let typed: TypedData =
            serde_json::from_value(typed_data).expect("typed data should deserialize");
        let hash = typed
            .eip712_signing_hash()
            .expect("typed data hash should build");
        let signature = signer
            .sign_hash_sync(&hash)
            .expect("typed data should sign")
            .to_string();
        let signed_order = json!({
            "sellToken": order_to_sign["sellToken"],
            "buyToken": order_to_sign["buyToken"],
            "receiver": order_to_sign["receiver"],
            "sellAmount": order_to_sign["sellAmount"],
            "buyAmount": order_to_sign["buyAmount"],
            "validTo": order_to_sign["validTo"],
            "appData": order_to_sign["appData"],
            "feeAmount": order_to_sign["feeAmount"],
            "kind": order_to_sign["kind"],
            "partiallyFillable": order_to_sign["partiallyFillable"],
            "sellTokenBalance": order_to_sign["sellTokenBalance"],
            "buyTokenBalance": order_to_sign["buyTokenBalance"],
            "from": "0x0000000000000000000000000000000000000003",
            "signingScheme": "eip712",
            "signature": signature,
        });

        let err = verify_cow_order_signature("polygon", &signed_order)
            .expect_err("wrong owner should fail local verification");
        assert!(err.contains("signature does not match signed_order.from"));
    }

    #[test]
    fn resolve_submission_uses_cached_quote_template_for_quote_id() {
        use alloy::signers::{SignerSync, local::PrivateKeySigner};

        let signer = PrivateKeySigner::from_str(
            "0x0000000000000000000000000000000000000000000000000000000000000001",
        )
        .expect("fixture private key should parse");
        let owner = format!("{:#x}", signer.address());
        let order_to_sign = json!({
            "sellToken": "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
            "buyToken": "0x7ceB23fD6bC0adD59E62ac25578270cFf1b9f619",
            "receiver": owner,
            "sellAmount": "100000",
            "buyAmount": "1",
            "validTo": 1777397782u64,
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "feeAmount": "1",
            "kind": "sell",
            "partiallyFillable": false,
            "sellTokenBalance": "erc20",
            "buyTokenBalance": "erc20"
        });
        let quote_id = "cowq:test:cached";
        let signed_order = build_cow_signed_order_payload(
            &order_to_sign,
            &owner,
            "eip712",
            Some(31),
        )
        .expect("signed order should build");
        store_pending_cow_quote(quote_id, "session-a", "polygon", &signed_order)
            .expect("pending quote should store");

        let typed_data = build_cow_order_typed_data(137, order_to_sign.clone());
        let typed: TypedData =
            serde_json::from_value(typed_data).expect("typed data should deserialize");
        let hash = typed
            .eip712_signing_hash()
            .expect("typed data hash should build");
        let signature = signer
            .sign_hash_sync(&hash)
            .expect("typed data should sign")
            .to_string();

        let mutated_signed_order = json!({
            "sellToken": order_to_sign["sellToken"],
            "buyToken": order_to_sign["buyToken"],
            "receiver": "0x0000000000000000000000000000000000000002",
            "sellAmount": "999999",
            "buyAmount": order_to_sign["buyAmount"],
            "validTo": order_to_sign["validTo"],
            "appData": order_to_sign["appData"],
            "feeAmount": order_to_sign["feeAmount"],
            "kind": order_to_sign["kind"],
            "partiallyFillable": order_to_sign["partiallyFillable"],
            "sellTokenBalance": order_to_sign["sellTokenBalance"],
            "buyTokenBalance": order_to_sign["buyTokenBalance"],
            "from": "0x0000000000000000000000000000000000000002",
            "signingScheme": "eip712"
        });

        let resolved = resolve_cow_order_submission(
            "session-a",
            "wrong-chain",
            Some(quote_id),
            mutated_signed_order,
            Some(&signature),
        )
        .expect("cached quote should resolve");

        assert_eq!(resolved.chain, "polygon");
        assert_eq!(resolved.signed_order["from"].as_str(), Some(owner.as_str()));
        assert_eq!(resolved.signed_order["sellAmount"].as_str(), Some("100000"));
        assert_eq!(resolved.signed_order["quoteId"].as_u64(), Some(31));
        verify_cow_order_signature("polygon", &resolved.signed_order)
            .expect("resolved cached order should verify");
    }
}
