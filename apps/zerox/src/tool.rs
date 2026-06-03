//! Curated tool layer for 0x Swap API v2. Wraps the generated progenitor
//! client in `aomi_ext::zerox` (see `ext/specs/zerox.yaml`).
//!
//! Designed for the user story: "swap on EVM via 0x — quote, execute on-chain
//! through AllowanceHolder, or sign EIP-712 for a gasless relayer trade."
//!
//! Tool surface (5 user-centric tools):
//!   * `zerox_get_price`            — AllowanceHolder price quote (no signing)
//!   * `zerox_build_swap`           — AllowanceHolder firm quote + executable tx
//!   * `zerox_get_gasless_quote`    — gasless quote with EIP-712 typed data
//!   * `zerox_submit_gasless_swap`  — submit signed gasless trade to relayer
//!   * `zerox_get_gasless_status`   — poll a submitted gasless trade

use aomi_ext::zerox::Client as GenClient;
use aomi_ext::zerox::types::{GaslessSubmitRequest, SwapQuote};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

const NATIVE_TOKEN_ADDR: &str = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";

/// Split a 65-byte EIP-712 signature hex (`0x` + r||s||v) into the
/// `{v, r, s}` triple 0x's gasless API expects under
/// `trade.signature` / `approval.signature`. Also normalises `v` from
/// 0/1 to 27/28.
fn split_signature_vrs(hex_sig: &str) -> Result<Value, String> {
    let trimmed = hex_sig.trim_start_matches("0x").trim_start_matches("0X");
    if trimmed.len() != 130 {
        return Err(format!(
            "[0x] signature must be 65 bytes (130 hex chars); got {} chars",
            trimmed.len()
        ));
    }
    let r = format!("0x{}", &trimmed[..64]);
    let s = format!("0x{}", &trimmed[64..128]);
    let mut v: u8 = u8::from_str_radix(&trimmed[128..130], 16)
        .map_err(|e| format!("[0x] signature `v` not valid hex: {e}"))?;
    if v < 27 {
        v += 27;
    }
    // signatureType=2 is EIP-712 per 0x's Settler/MetaTransaction schema.
    Ok(json!({ "signatureType": 2, "v": v, "r": r, "s": s }))
}

#[derive(Clone, Default)]
pub(crate) struct ZeroxApp;

const BASE_URL: &str = "https://api.0x.org";

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[0x] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("0x".to_string()));
            Value::Object(map)
        }
        other => json!({ "source": "0x", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[0x] runtime: {e}"))
}

fn resolve_key(ctx: &DynToolCallCtx, arg: Option<&str>) -> Result<String, String> {
    resolve_secret_value(
        ctx,
        arg,
        "ZEROX_API_KEY",
        "[0x] missing api_key argument and ZEROX_API_KEY environment variable",
    )
}

/// Build a generated client carrying the `0x-api-key` and `0x-version: v2`
/// headers that production 0x demands.
fn make_client(api_key: &str) -> Result<GenClient, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    let mut key = reqwest::header::HeaderValue::from_str(api_key)
        .map_err(|e| format!("[0x] invalid api_key: {e}"))?;
    key.set_sensitive(true);
    headers.insert(reqwest::header::HeaderName::from_static("0x-api-key"), key);
    headers.insert(
        reqwest::header::HeaderName::from_static("0x-version"),
        reqwest::header::HeaderValue::from_static("v2"),
    );

    let endpoint = std::env::var("ZEROX_API_ENDPOINT").unwrap_or_else(|_| BASE_URL.to_string());
    let http = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .default_headers(headers)
        .build()
        .map_err(|e| format!("[0x] failed to build HTTP client: {e}"))?;
    Ok(GenClient::new_with_client(&endpoint, http))
}

// ============================================================================
// Token / chain shorthand resolution (preserved from the handwritten client)
// ============================================================================

fn get_chain_info(chain: &str) -> Result<(&'static str, i64), String> {
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
        _ => Err(format!("[0x] unknown token {token} on chain {chain}")),
    }
}

fn get_token_decimals(chain: &str, token: &str) -> u8 {
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

fn amount_to_base_units(amount: f64, decimals: u8) -> Result<String, String> {
    if !amount.is_finite() || amount < 0.0 {
        return Err("[0x] amount must be a finite non-negative number".to_string());
    }
    let scaled = amount * 10f64.powi(decimals as i32);
    if scaled > (u128::MAX as f64) {
        return Err("[0x] amount is too large to convert to base units".to_string());
    }
    Ok((scaled.round() as u128).to_string())
}

// ============================================================================
// Tool: ZeroxGetPrice (price discovery, no taker required)
// ============================================================================

pub(crate) struct ZeroxGetPrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ZeroxGetPriceArgs {
    /// Optional 0x API key. Falls back to ZEROX_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Chain name (ethereum, polygon, arbitrum, optimism, base, bsc, avalanche).
    pub(crate) chain: String,
    /// Sell token symbol (e.g. "USDC", "ETH") or 0x... contract address.
    pub(crate) sell_token: String,
    /// Buy token symbol or 0x... contract address.
    pub(crate) buy_token: String,
    /// Sell amount in human-readable units (e.g. 100.0 for 100 USDC).
    pub(crate) amount: f64,
    /// Optional taker address (improves quote accuracy when included).
    #[serde(default)]
    pub(crate) sender_address: Option<String>,
    /// Slippage tolerance as a decimal (0.005 = 0.5%). Default: 0.01 (1%).
    #[serde(default)]
    pub(crate) slippage: Option<f64>,
}

impl DynAomiTool for ZeroxGetPrice {
    type App = ZeroxApp;
    type Args = ZeroxGetPriceArgs;
    const NAME: &'static str = "zerox_get_price";
    const DESCRIPTION: &'static str = "Use when the user asks for a 0x swap price (no signing). Returns expected `buyAmount` and routing for selling `amount` of `sell_token` for `buy_token`. Uses the AllowanceHolder pricing path so the quote reflects actual execution cost. No wallet signature required.";

    fn run(_app: &ZeroxApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let (chain_name, chain_id) = get_chain_info(&args.chain)?;
        let sell_addr = get_token_address(chain_name, &args.sell_token)?;
        let buy_addr = get_token_address(chain_name, &args.buy_token)?;
        let decimals = get_token_decimals(chain_name, &args.sell_token);
        let amount_wei = amount_to_base_units(args.amount, decimals)?;
        let slippage = args.slippage.unwrap_or(0.01);
        let taker = args.sender_address.clone();

        let client = make_client(&api_key)?;
        let runtime = rt()?;
        runtime.block_on(async move {
            let resp = client
                .get_allowance_holder_price(
                    &buy_addr,
                    chain_id,
                    &amount_wei,
                    &sell_addr,
                    Some(slippage),
                    taker.as_deref(),
                )
                .await
                .map_err(|e| format!("[0x] get_allowance_holder_price: {e}"))?
                .into_inner();
            ok::<SwapQuote>(resp)
        })
    }
}

// ============================================================================
// Tool: ZeroxBuildSwap (executable AllowanceHolder quote)
// ============================================================================

pub(crate) struct ZeroxBuildSwap;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ZeroxBuildSwapArgs {
    /// Optional 0x API key. Falls back to ZEROX_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Chain name (ethereum, polygon, arbitrum, optimism, base, bsc, avalanche).
    pub(crate) chain: String,
    /// Sell token symbol or 0x... address.
    pub(crate) sell_token: String,
    /// Buy token symbol or 0x... address.
    pub(crate) buy_token: String,
    /// Sell amount in human-readable units.
    pub(crate) amount: f64,
    /// Taker wallet address (the address that will execute the swap).
    pub(crate) sender_address: String,
    /// Slippage tolerance as a decimal (0.005 = 0.5%). Default: 0.01.
    #[serde(default)]
    pub(crate) slippage: Option<f64>,
}

impl DynAomiTool for ZeroxBuildSwap {
    type App = ZeroxApp;
    type Args = ZeroxBuildSwapArgs;
    const NAME: &'static str = "zerox_build_swap";
    const DESCRIPTION: &'static str = "Use when the user wants to execute a 0x swap on-chain. Composite tool: fetches an AllowanceHolder firm quote, checks ERC-20 allowance for the 0x AllowanceHolder, then routes the (optional) approval + swap transactions through the host wallet and binds the resulting tx hash. The LLM does not call stage_tx, simulate, or commit — the route handles it. For gasless (relayer-paid) execution, use zerox_get_gasless_quote instead.";

    fn run_with_routes(
        _app: &ZeroxApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let (chain_name, chain_id) = get_chain_info(&args.chain)?;
        let sell_addr = get_token_address(chain_name, &args.sell_token)?;
        let buy_addr = get_token_address(chain_name, &args.buy_token)?;
        let decimals = get_token_decimals(chain_name, &args.sell_token);
        let amount_wei = amount_to_base_units(args.amount, decimals)?;
        let slippage = args.slippage.unwrap_or(0.01);
        let taker = args.sender_address.clone();

        let client = make_client(&api_key)?;
        let runtime = rt()?;
        let (buy_addr_q, sell_addr_q, amount_q, taker_q) = (
            buy_addr.clone(),
            sell_addr.clone(),
            amount_wei.clone(),
            taker.clone(),
        );
        let quote = runtime
            .block_on(async move {
                client
                    .get_allowance_holder_quote(
                        &buy_addr_q,
                        chain_id,
                        &amount_q,
                        &sell_addr_q,
                        Some(slippage),
                        Some(&taker_q),
                    )
                    .await
            })
            .map_err(|e| format!("[0x] get_allowance_holder_quote: {e}"))?
            .into_inner();

        // SwapQuote wraps an untyped Map<String, Value>; walk into it for the
        // fields we need to stage txs.
        let quote_map: &serde_json::Map<String, Value> = &quote.0;
        let tx = quote_map
            .get("transaction")
            .and_then(Value::as_object)
            .ok_or_else(|| "[0x] quote response missing `transaction`".to_string())?;
        let tx_to = tx
            .get("to")
            .and_then(Value::as_str)
            .ok_or_else(|| "[0x] quote response missing `transaction.to`".to_string())?
            .to_string();
        let tx_data = tx
            .get("data")
            .and_then(Value::as_str)
            .ok_or_else(|| "[0x] quote response missing `transaction.data`".to_string())?
            .to_string();
        let tx_value = tx
            .get("value")
            .and_then(Value::as_str)
            .unwrap_or("0")
            .to_string();
        let tx_gas = tx
            .get("gas")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<u64>().ok());

        // ERC-20 approval check. Native sells (ETH/MATIC/BNB/AVAX) never need
        // approval. For ERC-20s, 0x returns `issues.allowance: null` when the
        // taker already has enough; an object with `spender` + `actual` when not.
        let sell_addr_lower = sell_addr.to_ascii_lowercase();
        let is_native_sell = sell_addr_lower == NATIVE_TOKEN_ADDR.to_ascii_lowercase();
        let approval_spender: Option<String> = (!is_native_sell)
            .then(|| {
                quote_map
                    .get("issues")
                    .and_then(|i| i.get("allowance"))
                    .and_then(Value::as_object)
                    .and_then(|a| a.get("spender"))
                    .and_then(Value::as_str)
                    .map(|s| s.to_string())
            })
            .flatten();

        let mut stage_args: Vec<Value> = Vec::new();
        if let Some(ref spender) = approval_spender {
            stage_args.push(json!({
                "to":          sell_addr,
                "description": format!("0x ERC-20 approval for {} on chain {chain_id}", args.sell_token),
                "data":        { "encode": { "signature": "approve(address,uint256)", "args": [spender, amount_wei] } },
                "value":       "0",
                "kind":        "erc20_approve",
            }));
        }
        stage_args.push(json!({
            "to":          tx_to,
            "description": format!(
                "0x swap {} → {} on chain {chain_id} (slippage {}%)",
                args.sell_token, args.buy_token, slippage * 100.0
            ),
            "data":        { "raw": tx_data },
            "value":       tx_value,
            "gas_limit":   tx_gas,
            "kind":        "swap",
        }));
        let last_index = stage_args.len() - 1;

        // Preview JSON for the LLM while the host runs the wallet.
        let preview = json!({
            "status":         "awaiting_wallet",
            "chain_id":       chain_id,
            "needs_approval": approval_spender.is_some(),
            "tx_count":       stage_args.len(),
            "slippage":       slippage,
            "quote":          &quote,
        });

        ToolReturn::route(ok(preview)?)
            .next(|next| {
                for (i, args) in stage_args.iter().enumerate() {
                    let step = next.add::<host::StageTx>(args.clone());
                    if i == last_index {
                        step.note(
                            "Stage the 0x swap. CRITICAL: copy `data.raw` and `to` BYTE-FOR-BYTE \
                             from the args below — do not abbreviate, reformat, or truncate the \
                             calldata. After this step the host automatically simulates and \
                             commits the staged txs and waits for the wallet.",
                        )
                        .enforce(EnforcementPolicy::Continue, |enforce| {
                            enforce.add::<host::SimulateBatch>(json!({}));
                            enforce
                                .add::<host::CommitTxs>(json!({ "aa_preference": "auto" }))
                                .bind_as("transaction_hash");
                        });
                    } else {
                        step.note(
                            "Stage the ERC-20 approval for the 0x AllowanceHolder. CRITICAL: \
                             copy `data` and `to` byte-for-byte; do not abbreviate or modify.",
                        );
                    }
                }
            })
            // No `.after::<>` — 0x swap is atomic per chain, so once commit_txs
            // lands the swap is done. The bound `transaction_hash` ends the route.
            .try_build()
            .map_err(|e| format!("[0x] route build: {e}"))
    }
}

// ============================================================================
// Tool: ZeroxGetGaslessQuote (EIP-712 quote for signing)
// ============================================================================

pub(crate) struct ZeroxGetGaslessQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ZeroxGetGaslessQuoteArgs {
    /// Optional 0x API key. Falls back to ZEROX_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// Chain name (ethereum, polygon, arbitrum, optimism, base, bsc, avalanche).
    pub(crate) chain: String,
    /// Sell token (must be ERC-20, not native ETH/MATIC/BNB/AVAX).
    pub(crate) sell_token: String,
    /// Buy token symbol or 0x... address.
    pub(crate) buy_token: String,
    /// Sell amount in human-readable units.
    pub(crate) amount: f64,
    /// Taker wallet address (the address that signs the EIP-712 payloads).
    pub(crate) sender_address: String,
    /// Slippage tolerance as a decimal (0.005 = 0.5%). Default: 0.01.
    #[serde(default)]
    pub(crate) slippage: Option<f64>,
}

impl DynAomiTool for ZeroxGetGaslessQuote {
    type App = ZeroxApp;
    type Args = ZeroxGetGaslessQuoteArgs;
    const NAME: &'static str = "zerox_get_gasless_quote";
    const DESCRIPTION: &'static str = "Use when the user wants a gasless 0x swap (relayer pays gas). Composite tool: fetches the gasless quote, routes the optional Permit2 approval signature first, then routes the Settler trade signature and submits the signed payload to 0x. The LLM does NOT call zerox_submit_gasless_swap separately. Sell token must be ERC-20 (not native).";

    fn run_with_routes(
        _app: &ZeroxApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let (chain_name, chain_id) = get_chain_info(&args.chain)?;
        let sell_addr = get_token_address(chain_name, &args.sell_token)?;
        let buy_addr = get_token_address(chain_name, &args.buy_token)?;
        let decimals = get_token_decimals(chain_name, &args.sell_token);
        let amount_wei = amount_to_base_units(args.amount, decimals)?;
        let slippage = args.slippage.unwrap_or(0.01);
        let taker = args.sender_address.clone();

        let client = make_client(&api_key)?;
        let runtime = rt()?;
        let quote = runtime
            .block_on(async move {
                client
                    .get_gasless_quote(
                        &buy_addr,
                        chain_id,
                        &amount_wei,
                        &sell_addr,
                        Some(slippage),
                        Some(&taker),
                    )
                    .await
            })
            .map_err(|e| format!("[0x] get_gasless_quote: {e}"))?
            .into_inner();

        let quote_map: &serde_json::Map<String, Value> = &quote.0;
        let trade_obj = quote_map
            .get("trade")
            .and_then(Value::as_object)
            .cloned()
            .ok_or_else(|| {
                "[0x] gasless quote missing `trade` field; cannot route signing".to_string()
            })?;
        let trade_typed_data = trade_obj
            .get("eip712")
            .cloned()
            .ok_or_else(|| "[0x] gasless quote `trade` missing `eip712` typed data".to_string())?;
        let approval_obj = quote_map
            .get("approval")
            .and_then(Value::as_object)
            .cloned();
        let approval_typed_data = approval_obj
            .as_ref()
            .and_then(|obj| obj.get("eip712").cloned());

        let preview = json!({
            "status":   "awaiting_wallet",
            "chain_id": chain_id,
            "quote":    &quote,
        });
        let submit_template = json!({
            "chain_id": chain_id as u64,
            "trade": Value::Object(trade_obj),
            "approval": approval_obj.map(Value::Object),
            "trade_signature": Value::Null,
            "approval_signature": Value::Null,
        });

        let builder = ToolReturn::route(ok(preview)?).next(|next| {
            if let Some(approval_typed_data) = approval_typed_data.clone() {
                next.add::<host::EvmCommitMessage>(json!({
                    "typed_data": approval_typed_data,
                    "description": format!(
                        "0x gasless: sign the Permit2 approval for {} on {}",
                        args.sell_token, chain_name,
                    ),
                }))
                .bind_as("approval_signature")
                .note("Sign the Permit2 approval first. The trade signature comes after this callback.");
            } else {
                next.add::<host::EvmCommitMessage>(json!({
                    "typed_data": trade_typed_data,
                    "description": format!(
                        "0x gasless trade: swap {} {} to {} on {}",
                        args.amount, args.sell_token, args.buy_token, chain_name,
                    ),
                }))
                    .bind_as("trade_signature")
                    .note("Sign the 0x gasless trade EIP-712 payload byte-for-byte.");
            }
        });

        if approval_typed_data.is_some() {
            return builder
                .after::<ZeroxSubmitGaslessSwap>(submit_template)
                .awaits("approval_signature")
                .note("Approval signed — request the 0x gasless trade signature next.")
                .try_build();
        }

        builder
            .after::<ZeroxSubmitGaslessSwap>(submit_template)
            .awaits("trade_signature")
            .note("Trade signed — submit the signed payload to the 0x relayer.")
            .try_build()
    }
}

// ============================================================================
// Tool: ZeroxSubmitGaslessSwap
// ============================================================================

pub(crate) struct ZeroxSubmitGaslessSwap;

/// Routed continuation of `zerox_get_gasless_quote`. Signature fields are
/// bound by the host wallet via `host::EvmCommitMessage`; everything else is
/// pre-filled from the quote so the LLM never re-types order fields between
/// sign and submit.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct ZeroxSubmitGaslessSwapArgs {
    /// Optional 0x API key. Falls back to ZEROX_API_KEY when omitted. Hidden
    /// from the LLM tool schema; operator-injected only.
    #[serde(default)]
    #[schemars(skip)]
    pub(crate) api_key: Option<String>,
    /// Numeric chain ID (e.g. 1 for Ethereum, 137 for Polygon).
    pub(crate) chain_id: u64,
    /// `trade` envelope from `zerox_get_gasless_quote` (carries the EIP-712
    /// typed data; the tool grafts the wallet signature onto it before
    /// submitting).
    pub(crate) trade: Value,
    /// `approval` envelope from `zerox_get_gasless_quote`, if the gasless
    /// quote required a Permit2 approval.
    #[serde(default)]
    pub(crate) approval: Option<Value>,
    /// Wallet signature for the trade EIP-712, auto-filled by the route plan's
    /// `trade_signature` alias. Either the raw `0x...` signature string or the
    /// full `evm_commit_message` completion payload.
    #[serde(default)]
    pub(crate) trade_signature: Option<Value>,
    /// Wallet signature for the Permit2 approval EIP-712, auto-filled by the
    /// route plan's `approval_signature` alias.
    #[serde(default)]
    pub(crate) approval_signature: Option<Value>,
}

fn signature_hex(signature: Value, label: &str) -> Result<String, String> {
    match signature {
        Value::String(s) => Ok(s),
        Value::Object(mut obj) => obj
            .remove("signature")
            .and_then(|v| v.as_str().map(str::to_string))
            .ok_or_else(|| format!("[0x] {label} payload missing `signature` field")),
        Value::Null => Err(format!("[0x] missing {label} signature")),
        _ => Err(format!("[0x] unsupported {label} signature shape")),
    }
}

fn graft_signature(
    envelope: &mut serde_json::Map<String, Value>,
    signature: Value,
    label: &str,
) -> Result<(), String> {
    let sig_hex = signature_hex(signature, label)?;
    envelope.insert("signature".to_string(), split_signature_vrs(&sig_hex)?);
    Ok(())
}

impl DynAomiTool for ZeroxSubmitGaslessSwap {
    type App = ZeroxApp;
    type Args = ZeroxSubmitGaslessSwapArgs;
    const NAME: &'static str = "zerox_submit_gasless_swap";
    const DESCRIPTION: &'static str = "Routed continuation of zerox_get_gasless_quote. Normally invoked automatically after the host wallet binds the approval and/or trade EIP-712 signatures. Requests the trade signature if only the approval signature has been collected, otherwise submits the signed payloads to 0x and returns a tradeHash.";

    fn run_with_routes(
        _app: &ZeroxApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let mut trade_map = match args.trade {
            Value::Object(m) => m,
            _ => return Err("[0x] `trade` must be a JSON object".to_string()),
        };
        let mut approval_map = match args.approval {
            Some(Value::Object(m)) => m,
            Some(Value::Null) | None => serde_json::Map::new(),
            Some(_) => {
                return Err("[0x] `approval` must be a JSON object when provided".to_string());
            }
        };

        if !approval_map.is_empty() {
            if let Some(sig) = args.approval_signature {
                graft_signature(&mut approval_map, sig, "approval")?;
            } else if !approval_map.contains_key("signature") {
                return Err(
                    "[0x] `approval_signature` is required for this gasless quote".to_string(),
                );
            }
        } else if args.approval_signature.is_some() {
            return Err(
                "[0x] `approval_signature` provided but no `approval` envelope to attach it to"
                    .to_string(),
            );
        }

        if let Some(sig) = args.trade_signature {
            graft_signature(&mut trade_map, sig, "trade")?;
        } else if !trade_map.contains_key("signature") {
            let trade_typed_data = trade_map
                .get("eip712")
                .cloned()
                .ok_or_else(|| "[0x] unsigned trade missing `eip712` typed data".to_string())?;
            let submit_template = json!({
                "chain_id": args.chain_id,
                "trade": Value::Object(trade_map.clone()),
                "approval": if approval_map.is_empty() {
                    Value::Null
                } else {
                    Value::Object(approval_map.clone())
                },
                "trade_signature": Value::Null,
                "approval_signature": Value::Null,
            });
            let preview = json!({
                "source": "0x",
                "status": "awaiting_trade_signature",
                "chain_id": args.chain_id,
                "approval_signed": !approval_map.is_empty(),
            });
            return ToolReturn::route(preview)
                .next(|next| {
                    next.add::<host::EvmCommitMessage>(json!({
                        "typed_data": trade_typed_data,
                        "description": format!("0x gasless trade on chain {}", args.chain_id),
                    }))
                    .bind_as("trade_signature")
                    .note("Sign the 0x gasless trade EIP-712 payload byte-for-byte.");
                })
                .after::<ZeroxSubmitGaslessSwap>(submit_template)
                .awaits("trade_signature")
                .note("Trade signed — submit the signed payload to the 0x relayer.")
                .try_build();
        }

        let body = GaslessSubmitRequest {
            chain_id: args.chain_id as i64,
            trade: trade_map,
            approval: approval_map,
        };
        let client = make_client(&api_key)?;
        let runtime = rt()?;
        let value = runtime.block_on(async move {
            let resp = client
                .submit_gasless(&body)
                .await
                .map_err(|e| format!("[0x] submit_gasless: {e}"))?
                .into_inner();
            ok::<SwapQuote>(resp)
        })?;
        Ok(ToolReturn::value(value))
    }
}

// ============================================================================
// Tool: ZeroxGetGaslessStatus
// ============================================================================

pub(crate) struct ZeroxGetGaslessStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ZeroxGetGaslessStatusArgs {
    /// Optional 0x API key. Falls back to ZEROX_API_KEY when omitted.
    #[serde(default)]
    pub(crate) api_key: Option<String>,
    /// `tradeHash` returned by `zerox_submit_gasless_swap`.
    pub(crate) trade_hash: String,
    /// Numeric chain ID where the trade was submitted.
    pub(crate) chain_id: u64,
}

impl DynAomiTool for ZeroxGetGaslessStatus {
    type App = ZeroxApp;
    type Args = ZeroxGetGaslessStatusArgs;
    const NAME: &'static str = "zerox_get_gasless_status";
    const DESCRIPTION: &'static str = "Use to track a submitted gasless trade. Returns status that progresses pending -> succeeded -> confirmed. Confirmed means the on-chain settlement is finalized.";

    fn run(_app: &ZeroxApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let client = make_client(&api_key)?;
        let runtime = rt()?;
        let trade_hash = args.trade_hash.clone();
        let chain_id = args.chain_id as i64;
        runtime.block_on(async move {
            let resp = client
                .get_gasless_status(&trade_hash, chain_id)
                .await
                .map_err(|e| format!("[0x] get_gasless_status: {e}"))?
                .into_inner();
            ok::<SwapQuote>(resp)
        })
    }
}
