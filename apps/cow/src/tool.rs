//! Curated tool layer for CoW Protocol's Order Book API. Wraps the generated
//! progenitor client in `aomi_ext::cow` (see `ext/specs/cow.yaml`).
//!
//! Designed for the user story: "price a swap on CoW, sign it off-chain, place
//! it on the order book, then track it."
//!
//! Tool surface (8 user-centric tools):
//!   * `get_cow_swap_quote`     — price + fee for a swap (POST /quote)
//!   * `place_cow_order`        — submit signed order JSON (POST /orders)
//!   * `get_cow_order`          — full order detail by UID (GET /orders/{uid})
//!   * `get_cow_order_status`   — lifecycle state poll (GET /orders/{uid}/status)
//!   * `get_cow_user_orders`    — order history (GET /account/{owner}/orders)
//!   * `cancel_cow_orders`      — cancel open orders (DELETE /orders)
//!   * `get_cow_trades`         — settlement records (GET /trades)
//!   * `get_cow_native_price`   — token native price (GET /token/{token}/native_price)

use aomi_ext::cow::Client as CowClient;
use aomi_ext::cow::types::{
    Address, OrderCancellations, OrderCreation, OrderParameters, OrderQuoteRequest, Uid,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct CowApp;

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[cow] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("cow".to_string()));
            Value::Object(map)
        }
        other => json!({ "source": "cow", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[cow] runtime: {e}"))
}

/// CoW exposes one orderbook host per chain; we pick the right baseurl here.
fn base_url_for_chain(chain: &str) -> Result<String, String> {
    let path = match chain.to_lowercase().as_str() {
        "ethereum" | "eth" | "mainnet" => "mainnet",
        "gnosis" | "xdai" => "xdai",
        "arbitrum" | "arb" | "arbitrum_one" => "arbitrum_one",
        "base" => "base",
        "polygon" | "matic" => "polygon",
        "avalanche" | "avax" => "avalanche",
        "bnb" | "bsc" => "bsc",
        "sepolia" => "sepolia",
        other => return Err(format!("[cow] unsupported chain: {other}")),
    };
    let endpoint =
        std::env::var("COW_API_ENDPOINT").unwrap_or_else(|_| "https://api.cow.fi".to_string());
    let endpoint = endpoint.trim_end_matches('/');
    // The generated client paths are `/api/v1/...`, so the baseurl should be
    // the chain-rooted host without `/api/v1`.
    Ok(format!("{endpoint}/{path}"))
}

/// Build a generated client with optional Bearer auth. CoW's public API is
/// accessible without an API key, but `COW_API_KEY` is honoured if set.
fn make_client(chain: &str) -> Result<CowClient, String> {
    let baseurl = base_url_for_chain(chain)?;
    let mut builder = reqwest::ClientBuilder::new().timeout(Duration::from_secs(30));
    if let Ok(api_key) = std::env::var("COW_API_KEY") {
        let mut headers = reqwest::header::HeaderMap::new();
        let mut bearer = reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|e| format!("[cow] invalid COW_API_KEY: {e}"))?;
        bearer.set_sensitive(true);
        headers.insert(reqwest::header::AUTHORIZATION, bearer);
        builder = builder.default_headers(headers);
    }
    let http = builder
        .build()
        .map_err(|e| format!("[cow] failed to build HTTP client: {e}"))?;
    Ok(CowClient::new_with_client(&baseurl, http))
}

fn from_value<T: serde::de::DeserializeOwned>(name: &str, value: Value) -> Result<T, String> {
    serde_json::from_value::<T>(value).map_err(|e| format!("[cow] failed to decode {name}: {e}"))
}

// ============================================================================
// EIP-712 helpers — wallet handoff for the swap flow
// ============================================================================

/// GPv2Settlement contract is deployed at the same address on every chain
/// where CoW operates. The order's EIP-712 `verifyingContract` is this value.
const GPV2_SETTLEMENT: &str = "0x9008D19f58AAbD9eD0D60971565AA8510560ab41";

/// Well-known stub `appData`: the empty JSON object `{}` and its keccak256
/// hash. CoW orders carry an `appData` bytes32 field that the orderbook
/// validates against `keccak256(appData_json_submitted)`. Computing keccak
/// in this crate would require a new dep; we sidestep it by always signing
/// and submitting the empty stub. Tradeoff: orders won't carry CoW Swap UI
/// metadata (`appCode`, slippage hint, …), which only affects analytics —
/// pricing and execution are unaffected.
const STUB_APPDATA_JSON: &str = "{}";
const STUB_APPDATA_HASH: &str =
    "0xb48d38f93eaa084033fc5970bf96e559c33c4cdc07d889ab00b4d63f9590739d";

/// Chain identifier used in the EIP-712 domain. CoW supports the same set as
/// the orderbook routing in `base_url_for_chain`.
fn chain_id_for_cow(chain: &str) -> Result<u64, String> {
    Ok(match chain.to_lowercase().as_str() {
        "ethereum" | "eth" | "mainnet" => 1,
        "gnosis" | "xdai" => 100,
        "arbitrum" | "arb" | "arbitrum_one" => 42161,
        "base" => 8453,
        "polygon" | "matic" => 137,
        "avalanche" | "avax" => 43114,
        "bnb" | "bsc" => 56,
        "sepolia" => 11155111,
        other => return Err(format!("[cow] unsupported chain for EIP-712: {other}")),
    })
}

/// Choose the bytes32 `appDataHash` for the EIP-712 message.
///
/// We deliberately ignore whatever appData CoW echoed back in the quote
/// response and always sign the stub. The orderbook only checks
/// `keccak256(appData_json_submitted) == bytes32_signed`; pricing is
/// already locked in by the quote's sell/buy/fee numbers and is unaffected
/// by appData. See `STUB_APPDATA_*` above for the tradeoff.
fn appdata_hash_for_signing(_quote: &OrderParameters) -> String {
    STUB_APPDATA_HASH.to_string()
}

/// What the host wallet's `evm_commit_message` step needs to render and sign.
/// Matches CoW's GPv2Order EIP-712 schema exactly.
fn build_cow_order_typed_data(chain: &str, prepared: &PreparedCowOrder) -> Result<Value, String> {
    let chain_id = chain_id_for_cow(chain)?;
    Ok(json!({
        "types": {
            "EIP712Domain": [
                { "name": "name",              "type": "string" },
                { "name": "version",           "type": "string" },
                { "name": "chainId",           "type": "uint256" },
                { "name": "verifyingContract", "type": "address" },
            ],
            "Order": [
                { "name": "sellToken",         "type": "address" },
                { "name": "buyToken",          "type": "address" },
                { "name": "receiver",          "type": "address" },
                { "name": "sellAmount",        "type": "uint256" },
                { "name": "buyAmount",         "type": "uint256" },
                { "name": "validTo",           "type": "uint32"  },
                { "name": "appData",           "type": "bytes32" },
                { "name": "feeAmount",         "type": "uint256" },
                { "name": "kind",              "type": "string"  },
                { "name": "partiallyFillable", "type": "bool"    },
                { "name": "sellTokenBalance",  "type": "string"  },
                { "name": "buyTokenBalance",   "type": "string"  },
            ],
        },
        "primaryType": "Order",
        "domain": {
            "name": "Gnosis Protocol",
            "version": "v2",
            "chainId": chain_id,
            "verifyingContract": GPV2_SETTLEMENT,
        },
        "message": {
            "sellToken":         prepared.sell_token,
            "buyToken":          prepared.buy_token,
            // The on-chain contract requires `receiver` as `address(0)` when
            // unset; the API mirrors that — pass zero-address rather than omit.
            "receiver":          prepared.receiver.clone().unwrap_or_else(|| "0x0000000000000000000000000000000000000000".to_string()),
            "sellAmount":        prepared.sell_amount,
            "buyAmount":         prepared.buy_amount,
            "validTo":           prepared.valid_to,
            "appData":           prepared.app_data_hash,
            "feeAmount":         prepared.fee_amount,
            "kind":              prepared.kind,
            "partiallyFillable": prepared.partially_fillable,
            "sellTokenBalance":  prepared.sell_token_balance,
            "buyTokenBalance":   prepared.buy_token_balance,
        },
    }))
}

fn build_cow_order_description(prepared: &PreparedCowOrder) -> String {
    let dir = if prepared.kind == "sell" {
        "Sell"
    } else {
        "Buy"
    };
    format!(
        "CoW {dir} order — {} of {} for {} of {}",
        prepared.sell_amount, prepared.sell_token, prepared.buy_amount, prepared.buy_token
    )
}

/// Distilled order fields the wallet step signs and `place_cow_order`
/// submits. Mirrors the on-chain `GPv2Order` struct; nothing here is mutable
/// between sign and submit — that's the whole point.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct PreparedCowOrder {
    pub(crate) sell_token: String,
    pub(crate) buy_token: String,
    pub(crate) receiver: Option<String>,
    pub(crate) sell_amount: String,
    pub(crate) buy_amount: String,
    pub(crate) valid_to: i64,
    /// keccak256 hash of the appData JSON, hex-encoded as `0x…32 bytes`.
    pub(crate) app_data_hash: String,
    /// Full appData JSON string when the user wants CoW to record it; can be
    /// `"{}"` and the contract still verifies against `app_data_hash`. Carried
    /// here so `place_cow_order` can echo it back to the API.
    pub(crate) app_data_full: Option<String>,
    pub(crate) fee_amount: String,
    /// `"sell"` or `"buy"`.
    pub(crate) kind: String,
    pub(crate) partially_fillable: bool,
    /// `"erc20"` (default), `"external"`, or `"internal"`.
    pub(crate) sell_token_balance: String,
    /// `"erc20"` (default) or `"internal"`.
    pub(crate) buy_token_balance: String,
    /// Signer of the order; required by CoW so the API can pre-validate.
    pub(crate) from: Option<String>,
    /// Quote ID returned by `/quote`; lets CoW link order ↔ quote for slippage analytics.
    pub(crate) quote_id: Option<i64>,
}

/// Build a `PreparedCowOrder` from a fresh `/quote` response.
///
/// Two non-obvious pinnings:
///
///   * `app_data_hash` / `app_data_full` are always the stub (`"{}"` and
///     its keccak). See `STUB_APPDATA_*` above for the tradeoff.
///
///   * `fee_amount` is always `"0"`, not the quote's `feeAmount`. CoW's
///     `OrderCreation` docs say: *"When creating an order, this should be
///     set to zero as fees are now computed dynamically by solvers."*
///     The quote's `feeAmount` is for human-readable display only; signing
///     it produces an order the solver will reject. The quoted fee still
///     shows up in the preview JSON the LLM presents to the user.
fn prepared_order_from_quote(
    quote: &OrderParameters,
    quote_id: Option<i64>,
    from: Option<String>,
) -> PreparedCowOrder {
    PreparedCowOrder {
        sell_token: quote.sell_token.to_string(),
        buy_token: quote.buy_token.to_string(),
        receiver: quote.receiver.as_ref().map(|a| a.to_string()),
        sell_amount: quote.sell_amount.to_string(),
        buy_amount: quote.buy_amount.to_string(),
        valid_to: quote.valid_to,
        app_data_hash: appdata_hash_for_signing(quote),
        app_data_full: Some(STUB_APPDATA_JSON.to_string()),
        fee_amount: "0".to_string(),
        kind: quote.kind.to_string(),
        partially_fillable: quote.partially_fillable,
        sell_token_balance: quote.sell_token_balance.to_string(),
        buy_token_balance: quote.buy_token_balance.to_string(),
        from,
        quote_id,
    }
}

// ============================================================================
// Token / chain helpers (shorthand resolution)
// ============================================================================

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
        _ => Err(format!("[cow] unknown token {token} on chain {chain}")),
    }
}

fn get_token_decimals(chain: &str, token: &str) -> u8 {
    let token_lower = token.to_lowercase();
    if is_hex_address(token) {
        return 18; // unknown — caller should pass amount in base units if needed.
    }
    match (chain, token_lower.as_str()) {
        (_, "usdc") | (_, "usdt") => 6,
        (_, "wbtc") => 8,
        _ => 18,
    }
}

fn chain_for_token(chain: &str) -> String {
    match chain.to_lowercase().as_str() {
        "ethereum" | "eth" | "mainnet" => "ethereum".to_string(),
        "polygon" | "matic" => "polygon".to_string(),
        "arbitrum" | "arb" | "arbitrum_one" => "arbitrum".to_string(),
        "base" => "base".to_string(),
        "bsc" | "binance" | "bnb" => "bsc".to_string(),
        "avalanche" | "avax" => "avalanche".to_string(),
        "gnosis" | "xdai" => "gnosis".to_string(),
        "sepolia" => "sepolia".to_string(),
        other => other.to_string(),
    }
}

fn amount_to_base_units(amount: f64, decimals: u8) -> Result<String, String> {
    if !amount.is_finite() || amount < 0.0 {
        return Err("[cow] amount must be a finite non-negative number".to_string());
    }
    let scaled = amount * 10f64.powi(decimals as i32);
    if scaled > (u128::MAX as f64) {
        return Err("[cow] amount is too large to convert to base units".to_string());
    }
    Ok((scaled.round() as u128).to_string())
}

// ============================================================================
// GetCowSwapQuote
// ============================================================================

pub(crate) struct GetCowSwapQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowSwapQuoteArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia.
    pub(crate) chain: String,
    /// Sell token symbol (eth, usdc, weth, ...) or 0x address.
    pub(crate) sell_token: String,
    /// Buy token symbol or 0x address.
    pub(crate) buy_token: String,
    /// Amount in human units. Treated as sell amount when order_side="sell" (default), buy amount when "buy".
    pub(crate) amount: f64,
    /// User-facing wallet address that owns the funds. Often the AA smart-account
    /// address shown in the UI. Used for the quote's `from` parameter and as the
    /// default receiver. Distinct from `signer_address` — see below.
    pub(crate) sender_address: String,
    /// Optional alternate receiver. Defaults to sender_address.
    pub(crate) receiver_address: Option<String>,
    /// "sell" (exact-in, default) or "buy" (exact-out).
    pub(crate) order_side: Option<String>,
    /// Slippage tolerance as decimal (0.005 = 0.5%). Defaults to CoW's recommendation when omitted.
    pub(crate) slippage: Option<f64>,
    /// EOA that will actually produce the EIP-712 signature, and therefore the
    /// address CoW recovers from the signature and matches against the order's
    /// `from` field. For AA flows this is the smart-account's signing EOA,
    /// NOT the smart-account address itself. Leave unset to fall back to the
    /// session's connected EVM address (`domain.evm.address` context attribute),
    /// and as a last resort to `sender_address`.
    pub(crate) signer_address: Option<String>,
}

impl DynAomiTool for GetCowSwapQuote {
    type App = CowApp;
    type Args = GetCowSwapQuoteArgs;
    const NAME: &'static str = "get_cow_swap_quote";
    const DESCRIPTION: &'static str = "Use when the user wants to swap on CoW Protocol (\"swap 100 USDC for WETH on base\"). Prices the swap, then automatically routes through the host wallet's EIP-712 signature step and submits the signed order — the LLM does NOT call place_cow_order separately. Returns the quote (sellAmount, buyAmount, fees) for the user to confirm; once they do, the wallet signature and final orderUid land via the routed continuation. For account-abstraction setups, set `signer_address` to the AA wallet's signing EOA (the value the runtime injects under `domain.evm.address`); CoW recovers that EOA from the EIP-712 signature.";

    fn run_with_routes(
        _app: &CowApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let chain = args.chain.clone();
        let token_chain = chain_for_token(&chain);
        let sell_addr = get_token_address(&token_chain, &args.sell_token)?;
        let buy_addr = get_token_address(&token_chain, &args.buy_token)?;
        let decimals = get_token_decimals(&token_chain, &args.sell_token);
        let amount_base = amount_to_base_units(args.amount, decimals)?;
        let kind = args.order_side.as_deref().unwrap_or("sell").to_string();

        // Build a JSON payload matching OrderQuoteRequestVariant0::Variant0
        // (sell-side ECDSA flow) or Variant1 / Variant2 (buy-side). We let
        // serde decide which oneOf variant matches.
        let mut body = json!({
            "sellToken": sell_addr,
            "buyToken":  buy_addr,
            "from":      args.sender_address,
            "kind":      kind,
        });
        if kind == "sell" {
            body["sellAmountBeforeFee"] = json!(amount_base);
        } else {
            body["buyAmount"] = json!(amount_base);
        }
        if let Some(r) = &args.receiver_address {
            body["receiver"] = json!(r);
        }
        if args.slippage.is_some() {
            body["priceQuality"] = json!("optimal");
        }

        let typed: OrderQuoteRequest = from_value("quote request", body)?;
        let client = make_client(&chain)?;
        let runtime = rt()?;
        let quote = runtime
            .block_on(async move { client.quote(&typed).await })
            .map_err(|e| format!("[cow] quote: {e}"))?
            .into_inner();

        // Resolve the signer address (CoW's `from` field). For AA flows the
        // smart-account address (`sender_address`) and the EOA that produces
        // the signature are different — CoW recovers the EOA from the
        // signature and compares it against `from`. Prefer the explicit
        // arg, then the session-injected connected wallet, then fall back
        // to `sender_address` for non-AA setups.
        let signer = args
            .signer_address
            .clone()
            .or_else(|| ctx.attribute_string(&["domain", "evm", "address"]))
            .unwrap_or_else(|| args.sender_address.clone());

        // Distil the quote into the exact set of fields the wallet will sign
        // and `place_cow_order` will resubmit. Once the signer commits to
        // these, nothing in this struct can change without invalidating the
        // signature.
        let prepared = prepared_order_from_quote(&quote.quote, quote.id, Some(signer));
        let typed_data = build_cow_order_typed_data(&chain, &prepared)?;
        let description = build_cow_order_description(&prepared);

        // Surface the quote + the prepared order to the LLM so the user has
        // something to confirm.
        let mut preview = ok(&quote)?;
        if let Value::Object(map) = &mut preview {
            map.insert(
                "prepared_order".to_string(),
                serde_json::to_value(&prepared)
                    .map_err(|e| format!("[cow] serialize prepared_order: {e}"))?,
            );
        }

        // Template `place_cow_order` will receive after the wallet binds the
        // signature. Everything except `signature` is filled in here; the
        // route fills in `signature` from the host wallet's commit step.
        let submit_template = serde_json::to_value(&PlaceCowOrderArgs {
            chain,
            order: prepared,
            signature: None,
            signing_scheme: Some("eip712".to_string()),
        })
        .map_err(|e| format!("[cow] serialize submit template: {e}"))?;

        let wallet_request = json!({
            "typed_data":  typed_data,
            "description": description,
        });

        Ok(ToolReturn::route(preview)
            .next(|next| {
                next.add::<host::EvmCommitMessage>(wallet_request)
                    .bind_as("signature")
                    .note(
                        "Sign the CoW order EIP-712 payload byte-for-byte. \
                         Do not modify any field — the signature is over the \
                         exact `message` shown.",
                    );
            })
            .after::<PlaceCowOrder>(submit_template)
            .awaits("signature")
            .note("Wallet signed — submitting the CoW order to the orderbook.")
            .build())
    }
}

// ============================================================================
// PlaceCowOrder
// ============================================================================

pub(crate) struct PlaceCowOrder;

/// Continuation step invoked by `get_cow_swap_quote`'s routed flow. The
/// `signature` field is bound by the host wallet via `host::EvmCommitMessage`;
/// the rest is pre-filled from the quote so the LLM never re-types order
/// fields between sign and submit. Calling this tool directly without a
/// `signature` returns an error — it's not a standalone primitive.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct PlaceCowOrderArgs {
    /// CoW chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Canonical order fields the wallet signed. Echoed back unchanged from
    /// the quote — mutating any field here invalidates the signature.
    pub(crate) order: PreparedCowOrder,
    /// Wallet signature hex (`0x…`). Filled by the routed `evm_commit_message`
    /// step; must be set when this tool runs.
    pub(crate) signature: Option<String>,
    /// `"eip712"` (default), `"ethsign"`, `"presign"`, or `"eip1271"`.
    pub(crate) signing_scheme: Option<String>,
}

impl DynAomiTool for PlaceCowOrder {
    type App = CowApp;
    type Args = PlaceCowOrderArgs;
    const NAME: &'static str = "place_cow_order";
    const DESCRIPTION: &'static str = "Routed continuation of get_cow_swap_quote — the host wallet auto-invokes it after binding the EIP-712 signature. Submits the signed order to CoW's orderbook and returns the orderUid you can poll with get_cow_order_status. Do not invoke directly unless you already have a fresh wallet signature for the exact prepared order.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let signature = args.signature.ok_or_else(|| {
            "[cow] place_cow_order requires a signature; this tool is invoked automatically \
             after get_cow_swap_quote's evm_commit_message step — don't call it manually"
                .to_string()
        })?;
        let signing_scheme = args
            .signing_scheme
            .as_deref()
            .unwrap_or("eip712")
            .to_string();

        // Reconstruct OrderCreation from the prepared order + bound signature.
        // Field-for-field with no transformation other than re-typing strings
        // into the generated client's newtypes (Address, TokenAmount, …).
        let app_data_value = args
            .order
            .app_data_full
            .as_deref()
            .unwrap_or(args.order.app_data_hash.as_str());
        let body_json = json!({
            "sellToken":         args.order.sell_token,
            "buyToken":          args.order.buy_token,
            "receiver":          args.order.receiver,
            "sellAmount":        args.order.sell_amount,
            "buyAmount":         args.order.buy_amount,
            "validTo":           args.order.valid_to,
            "appData":           app_data_value,
            "appDataHash":       args.order.app_data_hash,
            "feeAmount":         args.order.fee_amount,
            "kind":              args.order.kind,
            "partiallyFillable": args.order.partially_fillable,
            "sellTokenBalance":  args.order.sell_token_balance,
            "buyTokenBalance":   args.order.buy_token_balance,
            "from":              args.order.from,
            "quoteId":           args.order.quote_id,
            "signature":         signature,
            "signingScheme":     signing_scheme,
        });
        let body: OrderCreation = from_value("order creation", body_json)?;
        let client = make_client(&args.chain)?;
        let runtime = rt()?;
        runtime.block_on(async move {
            let resp = client
                .create_order(&body)
                .await
                .map_err(|e| format!("[cow] create_order: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// GetCowOrder
// ============================================================================

pub(crate) struct GetCowOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowOrderArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Order UID returned when the order was placed
    pub(crate) order_uid: String,
}

impl DynAomiTool for GetCowOrder {
    type App = CowApp;
    type Args = GetCowOrderArgs;
    const NAME: &'static str = "get_cow_order";
    const DESCRIPTION: &'static str = "Use when the user wants full detail on a CoW order they previously placed (executed amounts, fees, signature, status). Provide the orderUid returned from place_cow_order. For just the lifecycle state, prefer get_cow_order_status.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = make_client(&args.chain)?;
        let runtime = rt()?;
        let uid = Uid(args.order_uid);
        runtime.block_on(async move {
            let resp = client
                .get_order(&uid)
                .await
                .map_err(|e| format!("[cow] get_order: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// GetCowOrderStatus
// ============================================================================

pub(crate) struct GetCowOrderStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowOrderStatusArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Order UID returned when the order was placed
    pub(crate) order_uid: String,
}

impl DynAomiTool for GetCowOrderStatus {
    type App = CowApp;
    type Args = GetCowOrderStatusArgs;
    const NAME: &'static str = "get_cow_order_status";
    const DESCRIPTION: &'static str = "Use to poll a CoW order's lifecycle state (open / scheduled / active / solved / executing / traded / cancelled). Lighter than get_cow_order. Don't poll faster than every ~3s; CoW solver auctions clear in ~30s.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = make_client(&args.chain)?;
        let runtime = rt()?;
        let uid = Uid(args.order_uid);
        runtime.block_on(async move {
            let resp = client
                .get_order_status(&uid)
                .await
                .map_err(|e| format!("[cow] get_order_status: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// GetCowUserOrders
// ============================================================================

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

impl DynAomiTool for GetCowUserOrders {
    type App = CowApp;
    type Args = GetCowUserOrdersArgs;
    const NAME: &'static str = "get_cow_user_orders";
    const DESCRIPTION: &'static str = "Use when the user asks about their CoW order history on a chain (\"my recent swaps on base\"). Paginated, newest first. Default limit if omitted is CoW's default (~10).";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = make_client(&args.chain)?;
        let runtime = rt()?;
        let owner = Address(args.owner_address);
        let limit = args.limit.map(|v| v as i64);
        let offset = args.offset.map(|v| v as i64);
        runtime.block_on(async move {
            let resp = client
                .get_user_orders_paginated(&owner, limit, offset)
                .await
                .map_err(|e| format!("[cow] get_user_orders: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// CancelCowOrders
// ============================================================================

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

impl DynAomiTool for CancelCowOrders {
    type App = CowApp;
    type Args = CancelCowOrdersArgs;
    const NAME: &'static str = "cancel_cow_orders";
    const DESCRIPTION: &'static str = "Use when the user wants to cancel open CoW orders (only orders not yet executed can be cancelled). Requires a cancellation signature from the order owner — the host wallet must sign the cancellation message before calling this. Pass `signing_scheme=\"eip712\"` when in doubt.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let body_json = json!({
            "orderUids": args.order_uids,
            "signature": args.signature,
            "signingScheme": args.signing_scheme,
        });
        let body: OrderCancellations = from_value("cancellation", body_json)?;
        let client = make_client(&args.chain)?;
        let runtime = rt()?;
        runtime.block_on(async move {
            client
                .cancel_orders(&body)
                .await
                .map_err(|e| format!("[cow] cancel_orders: {e}"))?;
            ok(json!({ "cancelled": true }))
        })
    }
}

// ============================================================================
// GetCowTrades
// ============================================================================

pub(crate) struct GetCowTrades;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowTradesArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Owner address (provide exactly one of owner or order_uid)
    pub(crate) owner: Option<String>,
    /// Order UID (provide exactly one of owner or order_uid)
    pub(crate) order_uid: Option<String>,
    /// Pagination offset (ignored on v1 trades; v1 returns the full list)
    pub(crate) offset: Option<u32>,
    /// Maximum number of results to return (ignored on v1 trades)
    pub(crate) limit: Option<u32>,
}

impl DynAomiTool for GetCowTrades {
    type App = CowApp;
    type Args = GetCowTradesArgs;
    const NAME: &'static str = "get_cow_trades";
    const DESCRIPTION: &'static str = "Use when the user wants the on-chain settlement record (executed amounts, tx hashes) for either a wallet (`owner`) or one specific order (`order_uid`). Pass exactly one. Use get_cow_user_orders when the user wants the order book view rather than fills.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        match (&args.owner, &args.order_uid) {
            (Some(_), Some(_)) => {
                return Err(
                    "[cow] provide exactly one of `owner` or `order_uid`, not both".to_string(),
                );
            }
            (None, None) => {
                return Err("[cow] provide exactly one of `owner` or `order_uid`".to_string());
            }
            _ => {}
        }
        let client = make_client(&args.chain)?;
        let runtime = rt()?;
        let owner = args.owner.map(Address);
        let order_uid = args.order_uid.map(Uid);
        let _ = (args.offset, args.limit); // v1 endpoint doesn't paginate
        runtime.block_on(async move {
            let resp = client
                .get_trades(order_uid.as_ref(), owner.as_ref())
                .await
                .map_err(|e| format!("[cow] get_trades: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// GetCowNativePrice
// ============================================================================

pub(crate) struct GetCowNativePrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCowNativePriceArgs {
    /// Chain: ethereum, gnosis, arbitrum, base, polygon, avalanche, bsc, sepolia
    pub(crate) chain: String,
    /// Token contract address (0x...)
    pub(crate) token_address: String,
}

impl DynAomiTool for GetCowNativePrice {
    type App = CowApp;
    type Args = GetCowNativePriceArgs;
    const NAME: &'static str = "get_cow_native_price";
    const DESCRIPTION: &'static str = "Use to read CoW's internal estimate of a token's price in the chain's native asset (ETH on mainnet/arbitrum/base, xDAI on gnosis, etc.). Useful for sanity-checking a quote before signing. `token_address` must be a 0x address — symbol shorthand not supported here.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = make_client(&args.chain)?;
        let runtime = rt()?;
        let addr = Address(args.token_address);
        runtime.block_on(async move {
            let resp = client
                .get_token_native_price(&addr)
                .await
                .map_err(|e| format!("[cow] get_token_native_price: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}
