//! Curated tool layer for LI.FI cross-chain swap and bridge aggregator. Wraps
//! the generated progenitor client in `aomi_ext::lifi` (see
//! `ext/specs/lifi.yaml`).
//!
//! Designed for the user story: "move tokens across chains (or swap on the
//! same chain) via the best route LI.FI can find."
//!
//! Tool surface (6 user-centric tools):
//!   * `lifi_get_swap_quote`      — same-chain or cross-chain quote (no signing)
//!   * `lifi_build_swap_tx`       — composite quote + approval/main tx assembly
//!   * `lifi_build_bridge_tx`     — composite cross-chain bridge tx
//!   * `lifi_get_transfer_status` — track a cross-chain transfer
//!   * `lifi_list_chains`         — supported chains
//!   * `lifi_list_tokens`         — supported tokens
//!
//! TODO(tighten-spec): every response schema in `ext/specs/lifi.yaml` is
//! `additionalProperties: true`, so the generated client returns
//! `Map<String, Value>` and Value-walking is unavoidable here. Capture real
//! responses to `ext/specs/lifi.samples/` and run
//! `aomi-build tighten-spec lifi --shared` to infer concrete schemas; then
//! drop the UI-only fields from the inferred YAML and regenerate.

use aomi_ext::lifi::Client as GenClient;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct LifiApp;

const BASE_URL: &str = "https://li.quest";

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[lifi] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("lifi".to_string()));
            Value::Object(map)
        }
        other => json!({ "source": "lifi", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[lifi] runtime: {e}"))
}

/// Build a LI.FI client. The `LIFI_API_KEY` slot (header `x-lifi-api-key`)
/// is honoured when the host has provisioned it; quoting and status work
/// without it.
fn make_client(ctx: &DynToolCallCtx) -> Result<GenClient, String> {
    let mut builder = reqwest::ClientBuilder::new().timeout(Duration::from_secs(30));
    if let Ok(api_key) = resolve_secret_value(ctx, None, "LIFI_API_KEY", "") {
        let mut headers = reqwest::header::HeaderMap::new();
        let mut key = reqwest::header::HeaderValue::from_str(&api_key)
            .map_err(|e| format!("[lifi] invalid LIFI_API_KEY: {e}"))?;
        key.set_sensitive(true);
        headers.insert(
            reqwest::header::HeaderName::from_static("x-lifi-api-key"),
            key,
        );
        builder = builder.default_headers(headers);
    }
    let endpoint = std::env::var("LIFI_API_ENDPOINT").unwrap_or_else(|_| BASE_URL.to_string());
    let http = builder
        .build()
        .map_err(|e| format!("[lifi] failed to build HTTP client: {e}"))?;
    Ok(GenClient::new_with_client(&endpoint, http))
}

// ============================================================================
// Token / chain shorthand resolution (preserved from the handwritten client)
// ============================================================================

fn get_chain_info(chain: &str) -> Result<(&'static str, u64), String> {
    match chain.to_lowercase().as_str() {
        "ethereum" | "eth" | "mainnet" => Ok(("ethereum", 1)),
        "polygon" | "matic" => Ok(("polygon", 137)),
        "arbitrum" | "arb" | "arbitrum_one" => Ok(("arbitrum", 42161)),
        "optimism" | "op" => Ok(("optimism", 10)),
        "base" => Ok(("base", 8453)),
        "bsc" | "bnb" | "binance" => Ok(("bsc", 56)),
        "avalanche" | "avax" => Ok(("avalanche", 43114)),
        "gnosis" | "xdai" => Ok(("gnosis", 100)),
        "fantom" | "ftm" => Ok(("fantom", 250)),
        "linea" => Ok(("linea", 59144)),
        "scroll" => Ok(("scroll", 534352)),
        "zksync" | "zksync_era" => Ok(("zksync", 324)),
        _ => Err(format!("[lifi] unsupported chain: {chain}")),
    }
}

fn normalize_lifi_chain_id(chain: &str) -> Result<String, String> {
    if chain.chars().all(|c| c.is_ascii_digit()) {
        return Ok(chain.to_string());
    }
    let (_, id) = get_chain_info(chain)?;
    Ok(id.to_string())
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

fn get_token_decimals(chain: &str, token: &str) -> u8 {
    let token_lower = token.to_lowercase();
    if is_hex_address(token) {
        return 18;
    }
    match (chain, token_lower.as_str()) {
        (_, "usdc") | (_, "usdt") => 6,
        (_, "wbtc") => 8,
        _ => 18,
    }
}

fn amount_to_base_units(amount: f64, decimals: u8) -> Result<String, String> {
    if !amount.is_finite() || amount < 0.0 {
        return Err("[lifi] amount must be a finite non-negative number".to_string());
    }
    let scaled = amount * 10f64.powi(decimals as i32);
    if scaled > (u128::MAX as f64) {
        return Err("[lifi] amount is too large to convert to base units".to_string());
    }
    Ok((scaled.round() as u128).to_string())
}

/// Pull the executable `transactionRequest` out of a LI.FI quote JSON payload.
fn extract_main_tx(quote: &Value) -> Value {
    quote
        .get("transactionRequest")
        .cloned()
        .unwrap_or(Value::Null)
}

/// Build an ERC-20 approval tx for the LI.FI router if the sell token is an
/// ERC-20 and `estimate.approvalAddress` is present. Returns `null` for
/// native sells (no approval needed).
fn extract_approval_tx(quote: &Value, sell_token_addr: &str, amount_wei: &str) -> Value {
    let native = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";
    if sell_token_addr.eq_ignore_ascii_case(native) {
        return Value::Null;
    }
    let spender = match quote
        .get("estimate")
        .and_then(|e| e.get("approvalAddress"))
        .and_then(|v| v.as_str())
    {
        Some(s) => s.to_string(),
        None => return Value::Null,
    };

    // ERC-20 approve(address,uint256) selector = 0x095ea7b3
    let amount = match amount_wei.parse::<u128>() {
        Ok(v) => v,
        Err(_) => return Value::Null,
    };
    let spender_clean = spender.strip_prefix("0x").unwrap_or(&spender);
    let amount_hex = format!("{amount:x}");
    let spender_slot = format!("{spender_clean:0>64}");
    let amount_slot = format!("{amount_hex:0>64}");
    let data = format!("0x095ea7b3{spender_slot}{amount_slot}");
    json!({
        "to": sell_token_addr,
        "data": data,
        "value": "0x0",
        "description": "ERC-20 approval for LI.FI router",
    })
}

// ============================================================================
// LifiGetSwapQuote
// ============================================================================

pub(crate) struct LifiGetSwapQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct LifiGetSwapQuoteArgs {
    /// Source chain (name like "ethereum", "polygon", "base", "arbitrum", "optimism", "bsc", "avalanche").
    pub(crate) chain: String,
    /// Destination chain. Omit for same-chain swap.
    #[serde(default)]
    pub(crate) destination_chain: Option<String>,
    /// Sell token symbol (USDC, WETH, ETH, ...) or 0x... address.
    pub(crate) sell_token: String,
    /// Buy token symbol or 0x... address.
    pub(crate) buy_token: String,
    /// Sell amount in human-readable units (e.g. 100.0 = 100 USDC).
    pub(crate) amount: f64,
    /// Sender wallet address (0x...). Required.
    pub(crate) sender_address: String,
    /// Receiver wallet address. Defaults to sender.
    #[serde(default)]
    pub(crate) receiver_address: Option<String>,
}

impl DynAomiTool for LifiGetSwapQuote {
    type App = LifiApp;
    type Args = LifiGetSwapQuoteArgs;
    const NAME: &'static str = "lifi_get_swap_quote";
    const DESCRIPTION: &'static str = "Use to preview a same-chain or cross-chain swap via LI.FI (no signing). Returns expected `toAmount`, route summary, and gas/fee estimates. For execution, follow up with `lifi_build_swap_tx`.";

    fn run(_app: &LifiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let (chain_name, _) = get_chain_info(&args.chain)?;
        let from_decimals = get_token_decimals(chain_name, &args.sell_token);
        let amount_wei = amount_to_base_units(args.amount, from_decimals)?;
        let from_addr = get_token_address(chain_name, &args.sell_token)?;

        let dest_chain = args
            .destination_chain
            .as_deref()
            .unwrap_or(args.chain.as_str())
            .to_string();
        let (to_chain_name, _) = get_chain_info(&dest_chain)?;
        let to_addr = get_token_address(to_chain_name, &args.buy_token)?;
        let from_chain_id = normalize_lifi_chain_id(&args.chain)?;
        let to_chain_id = normalize_lifi_chain_id(&dest_chain)?;

        let client = make_client(&ctx)?;
        let runtime = rt()?;
        let receiver = args.receiver_address.clone();
        runtime.block_on(async move {
            let resp = client
                .get_quote(
                    &args.sender_address,
                    &amount_wei,
                    &from_chain_id,
                    &from_addr,
                    None,
                    receiver.as_deref(),
                    &to_chain_id,
                    &to_addr,
                )
                .await
                .map_err(|e| format!("[lifi] get_quote: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// LifiBuildSwapTx -- composite: quote + approval_tx (if needed) + main_tx
// ============================================================================

pub(crate) struct LifiBuildSwapTx;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct LifiBuildSwapTxArgs {
    /// Source chain name.
    pub(crate) chain: String,
    /// Destination chain. Omit for same-chain swap.
    #[serde(default)]
    pub(crate) destination_chain: Option<String>,
    /// Sell token symbol or 0x... address.
    pub(crate) sell_token: String,
    /// Buy token symbol or 0x... address.
    pub(crate) buy_token: String,
    /// Sell amount in human-readable units.
    pub(crate) amount: f64,
    /// Sender wallet address (0x...).
    pub(crate) sender_address: String,
    /// Receiver wallet address. Defaults to sender.
    #[serde(default)]
    pub(crate) receiver_address: Option<String>,
    /// Slippage tolerance as a decimal (0.005 = 0.5%).
    #[serde(default)]
    pub(crate) slippage: Option<f64>,
}

impl DynAomiTool for LifiBuildSwapTx {
    type App = LifiApp;
    type Args = LifiBuildSwapTxArgs;
    const NAME: &'static str = "lifi_build_swap_tx";
    const DESCRIPTION: &'static str = "Use when the user is ready to execute a same-chain or cross-chain swap via LI.FI. Returns `{ approval_tx?, main_tx, payload }`. If `approval_tx` is present (ERC-20 sell needing allowance), stage it first via `stage_tx` with `data: { raw }`, then stage `main_tx` the same way; `simulate_batch` on the staged ids; then `commit_tx` once per staged tx. Never re-encode LI.FI calldata.";

    fn run(_app: &LifiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let (chain_name, _) = get_chain_info(&args.chain)?;
        let from_decimals = get_token_decimals(chain_name, &args.sell_token);
        let amount_wei = amount_to_base_units(args.amount, from_decimals)?;
        let from_addr = get_token_address(chain_name, &args.sell_token)?;

        let dest_chain = args
            .destination_chain
            .clone()
            .unwrap_or_else(|| args.chain.clone());
        let (to_chain_name, _) = get_chain_info(&dest_chain)?;
        let to_addr = get_token_address(to_chain_name, &args.buy_token)?;
        let from_chain_id = normalize_lifi_chain_id(&args.chain)?;
        let to_chain_id = normalize_lifi_chain_id(&dest_chain)?;

        let client = make_client(&ctx)?;
        let runtime = rt()?;
        let sender = args.sender_address.clone();
        let receiver = args.receiver_address.clone();
        let slippage = args.slippage;
        let from_addr_for_async = from_addr.clone();
        let amount_wei_for_async = amount_wei.clone();
        let quote = runtime.block_on(async move {
            client
                .get_quote(
                    &sender,
                    &amount_wei_for_async,
                    &from_chain_id,
                    &from_addr_for_async,
                    slippage,
                    receiver.as_deref(),
                    &to_chain_id,
                    &to_addr,
                )
                .await
                .map_err(|e| format!("[lifi] get_quote: {e}"))
                .map(|r| r.into_inner())
        })?;
        let payload =
            serde_json::to_value(&quote).map_err(|e| format!("[lifi] serialize quote: {e}"))?;
        let main_tx = extract_main_tx(&payload);
        let approval_tx = extract_approval_tx(&payload, &from_addr, &amount_wei);

        ok(json!({
            "payload": payload,
            "approval_tx": approval_tx,
            "main_tx": main_tx,
            "note": "If approval_tx is non-null, stage it first with stage_tx { raw }, then stage main_tx the same way, then simulate_batch the staged pending_tx_id list, then commit_tx once per staged tx.",
        }))
    }
}

// ============================================================================
// LifiBuildBridgeTx -- cross-chain bridge (returns executable tx when possible)
// ============================================================================

pub(crate) struct LifiBuildBridgeTx;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct LifiBuildBridgeTxArgs {
    /// Source chain name or numeric ID.
    pub(crate) from_chain: String,
    /// Destination chain name or numeric ID.
    pub(crate) to_chain: String,
    /// Source token symbol or 0x... address.
    pub(crate) from_token: String,
    /// Destination token symbol or 0x... address.
    pub(crate) to_token: String,
    /// Bridge amount in human-readable units.
    pub(crate) amount: f64,
    /// Sender wallet address. Required for an executable bridge route.
    #[serde(default)]
    pub(crate) from_address: Option<String>,
    /// Receiver wallet address. Required for an executable bridge route.
    #[serde(default)]
    pub(crate) to_address: Option<String>,
    /// Slippage tolerance in basis points (default 50 = 0.5%).
    #[serde(default)]
    pub(crate) slippage_bps: Option<u32>,
}

impl DynAomiTool for LifiBuildBridgeTx {
    type App = LifiApp;
    type Args = LifiBuildBridgeTxArgs;
    const NAME: &'static str = "lifi_build_bridge_tx";
    const DESCRIPTION: &'static str = "Use when the user wants to bridge a token from one chain to another via LI.FI. Returns an executable bridge payload (with `executable_tx`) when both `from_address` and `to_address` are provided; otherwise returns a planning-only estimate. Stage and execute the same way as `lifi_build_swap_tx`. After executing, track on-chain finality with `lifi_get_transfer_status`.";

    fn run(_app: &LifiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let (from_chain_name, _) = get_chain_info(&args.from_chain)?;
        let (to_chain_name, _) = get_chain_info(&args.to_chain)?;
        let from_addr = get_token_address(from_chain_name, &args.from_token)?;
        let to_addr = get_token_address(to_chain_name, &args.to_token)?;
        let from_decimals = get_token_decimals(from_chain_name, &args.from_token);
        let amount_wei = amount_to_base_units(args.amount, from_decimals)?;
        let slippage_bps = args.slippage_bps.unwrap_or(50);
        let slippage = (slippage_bps as f64) / 10_000.0;
        let from_chain_id = normalize_lifi_chain_id(&args.from_chain)?;
        let to_chain_id = normalize_lifi_chain_id(&args.to_chain)?;

        let from_label = format!(
            "{} {} on {}",
            args.amount,
            args.from_token.to_uppercase(),
            args.from_chain.to_lowercase()
        );
        let to_label = format!(
            "{} on {}",
            args.to_token.to_uppercase(),
            args.to_chain.to_lowercase()
        );

        let from_address = args.from_address.unwrap_or_default();
        let to_address = args.to_address.unwrap_or_default();
        if !(is_hex_address(&from_address) && is_hex_address(&to_address)) {
            return ok(json!({
                "from": from_label,
                "to": to_label,
                "bridge": "planning-only",
                "executable_tx": Value::Null,
                "execution_supported": false,
                "warning": "Bridge quote is planning-only. Provide source and destination wallet addresses for executable routing.",
            }));
        }

        let client = make_client(&ctx)?;
        let runtime = rt()?;
        let from_addr_for_async = from_addr.clone();
        let amount_wei_for_async = amount_wei.clone();
        let quote_res = runtime.block_on(async move {
            client
                .get_quote(
                    &from_address,
                    &amount_wei_for_async,
                    &from_chain_id,
                    &from_addr_for_async,
                    Some(slippage),
                    Some(&to_address),
                    &to_chain_id,
                    &to_addr,
                )
                .await
                .map(|r| r.into_inner())
                .map_err(|e| format!("[lifi] bridge_quote: {e}"))
        });

        match quote_res {
            Ok(quote) => {
                let payload = serde_json::to_value(&quote)
                    .map_err(|e| format!("[lifi] serialize bridge quote: {e}"))?;
                // extract_main_tx / extract_approval_tx are composite necessities:
                // the wallet stages the executable tx separately, and the ERC-20
                // approval calldata is computed in Rust (not returned by LI.FI).
                let executable_tx = extract_main_tx(&payload);
                let approval_tx = extract_approval_tx(&payload, &from_addr, &amount_wei);
                ok(json!({
                    "from": from_label,
                    "to": to_label,
                    "approval_tx": approval_tx,
                    "executable_tx": executable_tx,
                    "execution_supported": !executable_tx.is_null(),
                    "payload": payload,
                }))
            }
            Err(e) => ok(json!({
                "from": from_label,
                "to": to_label,
                "executable_tx": Value::Null,
                "execution_supported": false,
                "warning": format!("LI.FI returned no executable route: {e}"),
            })),
        }
    }
}

// ============================================================================
// LifiGetTransferStatus
// ============================================================================

pub(crate) struct LifiGetTransferStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct LifiGetTransferStatusArgs {
    /// Source-chain transaction hash (the deposit tx the user signed).
    pub(crate) tx_hash: String,
    /// Source chain name or numeric ID. Optional but speeds lookup.
    #[serde(default)]
    pub(crate) from_chain: Option<String>,
    /// Destination chain name or numeric ID. Optional but speeds lookup.
    #[serde(default)]
    pub(crate) to_chain: Option<String>,
    /// Bridge name (e.g. "across", "stargate"). Optional but speeds lookup.
    #[serde(default)]
    pub(crate) bridge: Option<String>,
}

impl DynAomiTool for LifiGetTransferStatus {
    type App = LifiApp;
    type Args = LifiGetTransferStatusArgs;
    const NAME: &'static str = "lifi_get_transfer_status";
    const DESCRIPTION: &'static str = "Use to track a LI.FI cross-chain transfer by source-chain tx hash. Returns `status` (NOT_FOUND, INVALID, PENDING, DONE, FAILED), substatus, and the destination-chain receipt when complete. Poll periodically while status is PENDING.";

    fn run(_app: &LifiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let from_chain = args
            .from_chain
            .as_ref()
            .map(|c| normalize_lifi_chain_id(c))
            .transpose()?;
        let to_chain = args
            .to_chain
            .as_ref()
            .map(|c| normalize_lifi_chain_id(c))
            .transpose()?;
        let client = make_client(&ctx)?;
        let runtime = rt()?;
        let bridge = args.bridge.clone();
        let tx_hash = args.tx_hash.clone();
        runtime.block_on(async move {
            let resp = client
                .get_status(
                    bridge.as_deref(),
                    from_chain.as_deref(),
                    to_chain.as_deref(),
                    &tx_hash,
                )
                .await
                .map_err(|e| format!("[lifi] get_status: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// LifiListChains
// ============================================================================

pub(crate) struct LifiListChains;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct LifiListChainsArgs {
    /// Filter by chain type, e.g. "EVM", "SVM". Omit for all.
    #[serde(default)]
    pub(crate) chain_types: Option<String>,
}

impl DynAomiTool for LifiListChains {
    type App = LifiApp;
    type Args = LifiListChainsArgs;
    const NAME: &'static str = "lifi_list_chains";
    const DESCRIPTION: &'static str = "Use when the user asks 'what chains does LI.FI support?' Returns the supported chain list with names, ids, and native currency. Optionally filter by `chain_types` (EVM, SVM).";

    fn run(_app: &LifiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = make_client(&ctx)?;
        let runtime = rt()?;
        let chain_types = args.chain_types.clone();
        runtime.block_on(async move {
            let resp = client
                .get_chains(chain_types.as_deref())
                .await
                .map_err(|e| format!("[lifi] get_chains: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}

// ============================================================================
// LifiListTokens
// ============================================================================

pub(crate) struct LifiListTokens;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct LifiListTokensArgs {
    /// Comma-separated chain IDs to filter by (e.g. "1,137,8453").
    #[serde(default)]
    pub(crate) chains: Option<String>,
}

impl DynAomiTool for LifiListTokens {
    type App = LifiApp;
    type Args = LifiListTokensArgs;
    const NAME: &'static str = "lifi_list_tokens";
    const DESCRIPTION: &'static str = "Use when the user asks 'what tokens are bridgeable on chain X?' or needs a token's address/decimals. Returns the supported-token map keyed by chain ID. Pass `chains` (comma-separated chain IDs) to scope the response.";

    fn run(_app: &LifiApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = make_client(&ctx)?;
        let runtime = rt()?;
        let chains = args.chains.clone();
        runtime.block_on(async move {
            let resp = client
                .get_tokens(None, chains.as_deref())
                .await
                .map_err(|e| format!("[lifi] get_tokens: {e}"))?
                .into_inner();
            ok(resp)
        })
    }
}
