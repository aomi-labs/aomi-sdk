//! Curated tool layer for 1inch Swap API v6.0.
//!
//! Wraps the generated client in `aomi_ext::oneinch` (see `ext/specs/oneinch.yaml`).
//! Designed for the user story: "swap tokens on EVM chains via 1inch — get a
//! quote, build the swap transaction the user signs, manage approvals."
//!
//! The 6 mechanical stubs from `aomi-build gen-tool` collapse into 5
//! user-centric tools:
//!
//!   * `oneinch_get_quote`        — price-only quote for a swap
//!   * `oneinch_build_swap_tx`    — composite quote + allowance check + swap tx
//!   * `oneinch_check_allowance`  — current router allowance for an ERC-20
//!   * `oneinch_get_approve_tx`   — raw ERC-20 approval calldata
//!   * `oneinch_list_tokens`      — supported token list for a chain

use aomi_ext::oneinch::Client as GenClient;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct OneinchApp;

const BASE_URL: &str = "https://api.1inch.dev/swap/v6.0";

/// EVM chain ids 1inch officially supports for Swap API v6.0.
const SUPPORTED_CHAINS: &[i64] = &[1, 10, 56, 100, 137, 8453, 42161, 43114];

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value).map_err(|e| format!("[1inch] serialize: {e}"))?;
    Ok(match value {
        Value::Object(mut m) => {
            m.insert("source".into(), Value::String("1inch".into()));
            Value::Object(m)
        }
        other => json!({ "source": "1inch", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[1inch] runtime: {e}"))
}

fn resolve_key(ctx: &DynToolCallCtx, arg: Option<&str>) -> Result<String, String> {
    resolve_secret_value(
        ctx,
        arg,
        "ONEINCH_API_KEY",
        "[1inch] missing api_key argument and ONEINCH_API_KEY env var",
    )
}

fn validate_chain(chain_id: i64) -> Result<(), String> {
    if SUPPORTED_CHAINS.contains(&chain_id) {
        Ok(())
    } else {
        Err(format!(
            "[1inch] unsupported chain_id {chain_id}. Supported: {SUPPORTED_CHAINS:?}"
        ))
    }
}

/// Build a generated client carrying a `Bearer <api_key>` default header.
/// 1inch requires the bearer token on every request; the spec's `securitySchemes`
/// only declares the header name, so we wire it in here.
fn make_client(api_key: &str) -> Result<GenClient, String> {
    let mut headers = HeaderMap::new();
    let mut bearer = HeaderValue::from_str(&format!("Bearer {api_key}"))
        .map_err(|e| format!("[1inch] invalid api_key: {e}"))?;
    bearer.set_sensitive(true);
    headers.insert(AUTHORIZATION, bearer);

    let http = reqwest::ClientBuilder::new()
        .timeout(Duration::from_secs(30))
        .default_headers(headers)
        .build()
        .map_err(|e| format!("[1inch] failed to build HTTP client: {e}"))?;

    Ok(GenClient::new_with_client(BASE_URL, http))
}

/// Parse a u128 token amount; surfaces a friendly error rather than letting the
/// API reject the request later.
fn parse_amount(s: &str) -> Result<u128, String> {
    s.parse::<u128>()
        .map_err(|_| format!("[1inch] amount must be a non-negative integer string, got {s:?}"))
}

// ============================================================================
// Tool 1: oneinch_get_quote — price-only quote (no wallet, no tx)
// ============================================================================

pub(crate) struct GetQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetQuoteArgs {
    /// Optional 1inch developer API key. Falls back to ONEINCH_API_KEY env var.
    #[serde(default)]
    pub api_key: Option<String>,
    /// EVM chain id. Defaults to 1 (Ethereum). Supported: 1, 10, 56, 100, 137, 8453, 42161, 43114.
    #[serde(default)]
    pub chain_id: Option<i64>,
    /// Source token address (0x...). Use 0xEeeE...EEeE for native ETH/MATIC/BNB/AVAX.
    pub src: String,
    /// Destination token address (0x...). Use 0xEeeE...EEeE for native asset.
    pub dst: String,
    /// Sell amount in source-token base units (wei for 18-dec tokens; "1000000" = 1 USDC).
    pub amount: String,
}

impl DynAomiTool for GetQuote {
    type App = OneinchApp;
    type Args = GetQuoteArgs;
    const NAME: &'static str = "oneinch_get_quote";
    const DESCRIPTION: &'static str = "Use when the user asks for a 1inch swap price (no transaction). Returns the optimal route across DEXs and the expected `dstAmount` for selling `amount` of `src` for `dst` on the given chain. No wallet address required.";

    fn run(_app: &OneinchApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        validate_chain(chain_id)?;
        parse_amount(&args.amount)?;

        let client = make_client(&api_key)?;
        let runtime = rt()?;
        let result = runtime
            .block_on(async move {
                client
                    .get_quote(
                        chain_id,
                        args.amount.as_str(),
                        args.dst.as_str(),
                        None,
                        args.src.as_str(),
                    )
                    .await
            })
            .map_err(|e| format!("[1inch] get_quote: {e}"))?;
        ok(result.into_inner())
    }
}

// ============================================================================
// Tool 2: oneinch_build_swap_tx — composite quote + allowance + swap tx
// ============================================================================

pub(crate) struct BuildSwapTx;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BuildSwapTxArgs {
    /// Optional 1inch developer API key. Falls back to ONEINCH_API_KEY env var.
    #[serde(default)]
    pub api_key: Option<String>,
    /// EVM chain id. Defaults to 1 (Ethereum).
    #[serde(default)]
    pub chain_id: Option<i64>,
    /// Source token address. Use 0xEeeE...EEeE for native asset.
    pub src: String,
    /// Destination token address.
    pub dst: String,
    /// Sell amount in source-token base units.
    pub amount: String,
    /// Sender wallet address (the address that will sign and execute the swap).
    pub from: String,
    /// Maximum acceptable slippage as a percent (1 = 1%, 0.5 = 0.5%). Defaults to 1.0.
    #[serde(default)]
    pub slippage: Option<f64>,
}

const NATIVE_SENTINEL: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

fn is_native(addr: &str) -> bool {
    addr.eq_ignore_ascii_case(NATIVE_SENTINEL)
}

/// Build the `host::evm_stage_tx` args object from a 1inch `Transaction`.
/// 1inch returns fully-encoded calldata, so we use `data: { raw }` (the host
/// will not re-encode). Optional fields default conservatively.
fn stage_tx_args(
    tx: &aomi_ext::oneinch::types::Transaction,
    description: &str,
    kind: &str,
) -> Result<Value, String> {
    let to = tx
        .to
        .clone()
        .ok_or_else(|| "[1inch] tx missing `to`".to_string())?;
    let data = tx
        .data
        .clone()
        .ok_or_else(|| "[1inch] tx missing `data`".to_string())?;
    let value = tx.value.clone().unwrap_or_else(|| "0".to_string());
    Ok(json!({
        "to": to,
        "description": description,
        "data": { "raw": data },
        "value": value,
        "gas_limit": tx.gas,
        "kind": kind,
    }))
}

impl DynAomiTool for BuildSwapTx {
    type App = OneinchApp;
    type Args = BuildSwapTxArgs;
    const NAME: &'static str = "oneinch_build_swap_tx";
    const DESCRIPTION: &'static str = "Use when the user is ready to execute a 1inch swap. Composite tool: fetches a quote, checks ERC-20 allowance for the 1inch router (skipped for native sells), routes the (optional) approval and swap transactions through the host wallet, and binds the resulting tx hash. The LLM does not need to call evm_stage_tx, simulate, or commit — the route handles it.";

    fn run_with_routes(
        _app: &OneinchApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        validate_chain(chain_id)?;
        let amount_u = parse_amount(&args.amount)?;
        let slippage = args.slippage.unwrap_or(1.0);

        let client = make_client(&api_key)?;
        let runtime = rt()?;
        let src = args.src.clone();
        let dst = args.dst.clone();
        let amount = args.amount.clone();
        let from = args.from.clone();

        // 1. Quote + (optional) approve + swap, all in one runtime hop.
        let (quote, approve_tx_opt, swap) = runtime.block_on(async move {
            let quote = client
                .get_quote(chain_id, amount.as_str(), dst.as_str(), None, src.as_str())
                .await
                .map_err(|e| format!("[1inch] quote step: {e}"))?
                .into_inner();

            let mut approve_tx_opt: Option<aomi_ext::oneinch::types::Transaction> = None;
            if !is_native(&src) {
                let allow = client
                    .get_allowance(chain_id, src.as_str(), from.as_str())
                    .await
                    .map_err(|e| format!("[1inch] allowance step: {e}"))?
                    .into_inner();
                let current = allow.allowance.clone().unwrap_or_default();
                let needs_approve = match current.parse::<u128>() {
                    Ok(v) => v < amount_u,
                    Err(_) => true, // unparseable → assume insufficient
                };
                if needs_approve {
                    approve_tx_opt = Some(
                        client
                            .get_approve_transaction(chain_id, Some(amount.as_str()), src.as_str())
                            .await
                            .map_err(|e| format!("[1inch] approve_tx step: {e}"))?
                            .into_inner(),
                    );
                }
            }

            let swap = client
                .get_swap(
                    chain_id,
                    amount.as_str(),
                    dst.as_str(),
                    from.as_str(),
                    None,
                    slippage,
                    src.as_str(),
                )
                .await
                .map_err(|e| format!("[1inch] swap step: {e}"))?
                .into_inner();
            Ok::<_, String>((quote, approve_tx_opt, swap))
        })?;

        // 2. Build the routed wallet steps.
        let swap_tx = swap
            .tx
            .clone()
            .ok_or_else(|| "[1inch] swap response missing `tx`".to_string())?;
        let mut stage_args: Vec<Value> = Vec::new();
        if let Some(ref approve_tx) = approve_tx_opt {
            stage_args.push(stage_tx_args(
                approve_tx,
                &format!("1inch ERC-20 approval for {} (chain {chain_id})", args.src),
                "erc20_approve",
            )?);
        }
        stage_args.push(stage_tx_args(
            &swap_tx,
            &format!(
                "1inch swap {} → {} on chain {chain_id} (slippage {slippage}%)",
                args.src, args.dst
            ),
            "swap",
        )?);
        let last_index = stage_args.len() - 1;

        // 3. Preview the LLM sees while the wallet runs.
        let preview = json!({
            "status": "awaiting_wallet",
            "chain_id": chain_id,
            "slippage": slippage,
            "tx_count": stage_args.len(),
            "needs_approval": approve_tx_opt.is_some(),
            "quote": quote,
        });

        ToolReturn::route(ok(preview)?)
            .next(|next| {
                for (i, args) in stage_args.iter().enumerate() {
                    let step = next.add::<host::StageTx>(args.clone());
                    if i == last_index {
                        step.note(
                            "Follow the host-owned route continuation using compatible resources actually issued for this operation. Preserve source restrictions and prerequisite ordering; do not copy opaque bytes into model arguments. Simulate and commit the same complete ordered staged-resource cohort. Full raw payloads and callback fields remain unchanged in host routes.",
                        )
                        .enforce(EnforcementPolicy::Continue, |enforce| {
                            enforce.add::<host::SimulateBatch>(json!({}));
                            enforce
                                .add::<host::CommitTxs>(json!({ "aa_preference": "auto" }))
                                .bind_as("transaction_hash");
                        });
                    } else {
                        step.note(
                            "Follow the host-owned route continuation using compatible resources actually issued for this operation. Preserve source restrictions and prerequisite ordering; do not copy opaque bytes into model arguments. Simulate and commit the same complete ordered staged-resource cohort. Full raw payloads and callback fields remain unchanged in host routes.",
                        );
                    }
                }
            })
            // No `.after::<>` / `.awaits` — 1inch is atomic per chain, so once
            // evm_commit_txs lands the swap is done. The bound `transaction_hash`
            // ends the route.
            .try_build()
            .map_err(|e| format!("[1inch] route build: {e}"))
    }
}

// ============================================================================
// Tool 3: oneinch_check_allowance
// ============================================================================

pub(crate) struct CheckAllowance;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CheckAllowanceArgs {
    /// Optional 1inch developer API key. Falls back to ONEINCH_API_KEY env var.
    #[serde(default)]
    pub api_key: Option<String>,
    /// EVM chain id. Defaults to 1 (Ethereum).
    #[serde(default)]
    pub chain_id: Option<i64>,
    /// ERC-20 token contract address.
    pub token_address: String,
    /// Wallet address whose allowance is being checked.
    pub wallet_address: String,
}

impl DynAomiTool for CheckAllowance {
    type App = OneinchApp;
    type Args = CheckAllowanceArgs;
    const NAME: &'static str = "oneinch_check_allowance";
    const DESCRIPTION: &'static str = "Use before any ERC-20 swap to confirm the wallet has granted the 1inch router enough allowance. Returns the current allowance in token base units. If less than the swap amount, build an approval with `oneinch_get_approve_tx` (or use `oneinch_build_swap_tx` which handles this for you).";

    fn run(_app: &OneinchApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        validate_chain(chain_id)?;

        let client = make_client(&api_key)?;
        let runtime = rt()?;
        let result = runtime
            .block_on(async move {
                client
                    .get_allowance(
                        chain_id,
                        args.token_address.as_str(),
                        args.wallet_address.as_str(),
                    )
                    .await
            })
            .map_err(|e| format!("[1inch] check_allowance: {e}"))?;
        ok(result.into_inner())
    }
}

// ============================================================================
// Tool 4: oneinch_get_approve_tx
// ============================================================================

pub(crate) struct GetApproveTx;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetApproveTxArgs {
    /// Optional 1inch developer API key. Falls back to ONEINCH_API_KEY env var.
    #[serde(default)]
    pub api_key: Option<String>,
    /// EVM chain id. Defaults to 1 (Ethereum).
    #[serde(default)]
    pub chain_id: Option<i64>,
    /// ERC-20 token contract address to approve for the 1inch router.
    pub token_address: String,
    /// Approval amount in token base units. Omit for an unlimited approval.
    #[serde(default)]
    pub amount: Option<String>,
}

impl DynAomiTool for GetApproveTx {
    type App = OneinchApp;
    type Args = GetApproveTxArgs;
    const NAME: &'static str = "oneinch_get_approve_tx";
    const DESCRIPTION: &'static str = "Use when `oneinch_check_allowance` shows insufficient allowance. Returns a raw ERC-20 approval tx (to=token, data=approve calldata, value=0) targeting the 1inch router. Use a compatible approval resource or host route actually issued for this result; do not copy or re-encode calldata. Omit `amount` for unlimited approval.";

    fn run(_app: &OneinchApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        validate_chain(chain_id)?;

        let client = make_client(&api_key)?;
        let runtime = rt()?;
        let result = runtime
            .block_on(async move {
                client
                    .get_approve_transaction(
                        chain_id,
                        args.amount.as_deref(),
                        args.token_address.as_str(),
                    )
                    .await
            })
            .map_err(|e| format!("[1inch] get_approve_tx: {e}"))?;
        ok(result.into_inner())
    }
}

// ============================================================================
// Tool 5: oneinch_list_tokens
// ============================================================================

pub(crate) struct ListTokens;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ListTokensArgs {
    /// Optional 1inch developer API key. Falls back to ONEINCH_API_KEY env var.
    #[serde(default)]
    pub api_key: Option<String>,
    /// EVM chain id. Defaults to 1 (Ethereum).
    #[serde(default)]
    pub chain_id: Option<i64>,
}

impl DynAomiTool for ListTokens {
    type App = OneinchApp;
    type Args = ListTokensArgs;
    const NAME: &'static str = "oneinch_list_tokens";
    const DESCRIPTION: &'static str = "Use when the user asks 'what's the address of <symbol> on <chain>?' or wants to discover swappable tokens. Returns the full token map (address -> symbol/name/decimals/logo) supported by 1inch on the chain.";

    fn run(_app: &OneinchApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_key(&ctx, args.api_key.as_deref())?;
        let chain_id = args.chain_id.unwrap_or(1);
        validate_chain(chain_id)?;

        let client = make_client(&api_key)?;
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client.get_tokens(chain_id).await })
            .map_err(|e| format!("[1inch] list_tokens: {e}"))?;
        ok(result.into_inner())
    }
}
