//! Curated tool layer for Khalani Hyperstream.
//!
//! Six tools mapped from the API surface:
//!
//!   * `khalani_quote`            — POST /v1/quotes
//!   * `khalani_build_deposit`    — POST /v1/deposit/build, emits the routed
//!     `evm_stage_tx → simulate_batch → evm_commit_txs →
//!     submit_khalani_order` chain.
//!   * `submit_khalani_order`     — PUT /v1/deposit/submit, fired by the
//!     `OnBoundEvent` continuation.
//!   * `khalani_order_status`     — GET /v1/orders/{address}
//!   * `khalani_list_chains`      — GET /v1/chains
//!   * `khalani_search_tokens`    — GET /v1/tokens/search
//!
//! Helpers (`ok`, `rt`, `client`, `resolve_evm_wallet`) live at module top
//! and are reused by every tool so error and response shapes stay consistent.

use aomi_ext::khalani::Client as KhalaniClient;
use aomi_ext::khalani::types::{
    BuildDepositResponse, BuildDepositResponseApprovalsItem, DepositBuildRequest,
    DepositSubmitRequest, QuoteRequest, QuoteRequestTradeType,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

#[derive(Clone, Default)]
pub(crate) struct KhalaniApp;

const BASE_URL: &str = "https://api.hyperstream.dev";

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let v = serde_json::to_value(value).map_err(|e| format!("[khalani] serialize: {e}"))?;
    Ok(match v {
        Value::Object(mut m) => {
            m.insert("source".into(), Value::String("khalani".into()));
            Value::Object(m)
        }
        other => json!({ "source": "khalani", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[khalani] runtime: {e}"))
}

fn client() -> KhalaniClient {
    KhalaniClient::new(BASE_URL)
}

/// Pull the connected EVM wallet from the host context, falling back to an
/// explicit override.
fn resolve_evm_wallet(arg: Option<String>, ctx: &DynToolCallCtx) -> Result<String, String> {
    arg.or_else(|| ctx.attribute_string(&["domain", "evm", "address"]))
        .ok_or_else(|| {
            "[khalani] no EVM wallet address provided and none in context — pass `wallet` or connect an EVM wallet"
                .to_string()
        })
}

/// One on-chain transaction extracted from a Khalani build response.
///
/// Khalani returns `{ approvals: [eip1193_request, …], kind }`. The
/// `approvals` array contains a mix of `wallet_switchEthereumChain` and
/// `eth_sendTransaction` items; we only care about the latter. The final
/// `eth_sendTransaction` is always the deposit (`deposit: true`), preceded
/// by zero or more ERC-20 approval txs (`waitForReceipt: true`).
struct StagedTx {
    chain_id: u64,
    to: String,
    value: String,
    data: String,
    description: String,
}

fn extract_staged_txs(
    build_response: &BuildDepositResponse,
    quote_id: &str,
) -> Result<Vec<StagedTx>, String> {
    let mut staged = Vec::new();
    let mut current_chain_id = None;
    for entry in &build_response.approvals {
        if entry.request.method == "wallet_switchEthereumChain" {
            current_chain_id = entry
                .request
                .params
                .first()
                .and_then(|param| param.chain_id.as_deref())
                .map(parse_chain_id)
                .transpose()?;
            continue;
        }
        if entry.request.method != "eth_sendTransaction" {
            continue;
        }
        staged.push(stage_tx_from_entry(entry, quote_id, current_chain_id)?);
    }
    if staged.is_empty() {
        return Err(
            "[khalani] build response had no eth_sendTransaction entries to stage".to_string(),
        );
    }
    Ok(staged)
}

fn stage_tx_from_entry(
    entry: &BuildDepositResponseApprovalsItem,
    quote_id: &str,
    current_chain_id: Option<u64>,
) -> Result<StagedTx, String> {
    let params = entry
        .request
        .params
        .first()
        .ok_or_else(|| "[khalani] eth_sendTransaction missing params[0]".to_string())?;
    let chain_id = params
        .chain_id
        .as_deref()
        .map(parse_chain_id)
        .transpose()?
        .or(current_chain_id)
        .ok_or_else(|| "[khalani] eth_sendTransaction missing chainId".to_string())?;
    let to = params
        .to
        .clone()
        .ok_or_else(|| "[khalani] tx missing `to`".to_string())?;
    let data = params.data.clone().unwrap_or_else(|| "0x".to_string());
    let value = normalize_tx_value(params.value.as_deref())?;
    let is_deposit = entry.deposit.unwrap_or(false);
    let description = if is_deposit {
        format!("Khalani deposit for quote {quote_id}")
    } else {
        format!("Khalani approval for quote {quote_id}")
    };
    Ok(StagedTx {
        chain_id,
        to,
        value,
        data,
        description,
    })
}

fn parse_chain_id(value: &str) -> Result<u64, String> {
    let value = value.trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16)
            .map_err(|err| format!("[khalani] invalid hex chainId `{value}`: {err}"));
    }
    value
        .parse()
        .map_err(|err| format!("[khalani] invalid chainId `{value}`: {err}"))
}

fn normalize_tx_value(value: Option<&str>) -> Result<String, String> {
    let Some(value) = value else {
        return Ok("0".to_string());
    };
    let value = value.trim();
    if value.is_empty() {
        return Ok("0".to_string());
    }
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return u128::from_str_radix(hex, 16)
            .map(|value| value.to_string())
            .map_err(|err| format!("[khalani] invalid hex tx value `{value}`: {err}"));
    }
    Ok(value.to_string())
}

// ============================================================================
// Tool 1: Quote — POST /v1/quotes
// ============================================================================

pub(crate) struct Quote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct QuoteArgs {
    /// EVM chain ID of the source asset (1 = Ethereum, 10 = Optimism, 8453 = Base, …).
    pub from_chain_id: i64,
    /// EVM chain ID of the destination asset.
    pub to_chain_id: i64,
    /// Source token address (lowercase 0x…) or the sentinel `"native"` for the chain's native asset.
    pub from_token: String,
    /// Destination token address or `"native"`.
    pub to_token: String,
    /// Amount to swap, in the source token's base units string (e.g. "100000000" for 100 USDC).
    pub amount: String,
    /// Sender wallet (EVM address). Defaults to the host-connected EVM wallet.
    #[serde(default)]
    pub wallet: Option<String>,
    /// Recipient on the destination chain. Defaults to the sender wallet when omitted.
    #[serde(default)]
    pub recipient: Option<String>,
    /// Slippage tolerance in basis points (50 = 0.5%). Defaults to 50.
    #[serde(default)]
    pub slippage_bps: Option<i64>,
}

impl DynAomiTool for Quote {
    type App = KhalaniApp;
    type Args = QuoteArgs;
    const NAME: &'static str = "khalani_quote";
    const DESCRIPTION: &'static str = "Use to quote a cross-chain swap or transfer via Khalani Hyperstream. Returns a `quoteId` plus route candidates with expected output and fees. Pass the resulting `quoteId` to `khalani_build_deposit` to execute. Amount is in source-token base units (e.g. 100 USDC = '100000000').";

    fn run(_app: &KhalaniApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let from_address = resolve_evm_wallet(args.wallet.clone(), &ctx)?;
        let body = QuoteRequest {
            amount: args.amount,
            from_address,
            from_chain_id: args.from_chain_id,
            from_token: args.from_token,
            recipient: args.recipient,
            slippage_in_bps: Some(args.slippage_bps.unwrap_or(50)),
            to_chain_id: args.to_chain_id,
            to_token: args.to_token,
            trade_type: QuoteRequestTradeType::ExactInput,
        };
        let runtime = rt()?;
        let response = runtime
            .block_on(async { client().get_quote(&body).await })
            .map_err(|e| format!("[khalani] quote: {e}"))?
            .into_inner();
        ok(response)
    }
}

// ============================================================================
// Tool 2: BuildDeposit — POST /v1/deposit/build (producer route)
// ============================================================================

pub(crate) struct BuildDeposit;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BuildDepositArgs {
    /// `quoteId` returned by `khalani_quote`.
    pub quote_id: String,
    /// `routeId` of the chosen route from the quote response. Optional when the quote returned a single route.
    #[serde(default)]
    pub route_id: Option<String>,
    /// Sender EVM wallet. Defaults to the host-connected EVM wallet.
    #[serde(default)]
    pub wallet: Option<String>,
    /// Slippage tolerance in basis points. Use 50 unless the user requested a different tolerance.
    #[serde(default)]
    pub slippage_bps: Option<i64>,
}

impl DynAomiTool for BuildDeposit {
    type App = KhalaniApp;
    type Args = BuildDepositArgs;
    const NAME: &'static str = "khalani_build_deposit";
    const DESCRIPTION: &'static str = "Use after `khalani_quote` once the user has selected a route. Pass `quote_id`, the chosen route's `route_id`, and `slippage_bps` (usually 50). Builds approval/deposit calldata and emits a routed plan: call the returned `evm_stage_tx` steps in order, then `simulate_batch`, then `evm_commit_txs`; once commit returns a tx hash, call the deferred `submit_khalani_order` route.";

    fn run_with_routes(
        _app: &KhalaniApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let wallet = resolve_evm_wallet(args.wallet.clone(), &ctx)?;
        let body = DepositBuildRequest {
            allowance_target: None,
            deposit_method: None,
            from: Some(wallet.clone()),
            from_address: Some(wallet.clone()),
            quote_id: Some(args.quote_id.clone()),
            route_id: args.route_id.clone(),
            slippage_in_bps: Some(args.slippage_bps.unwrap_or(50)),
            user: Some(wallet.clone()),
            user_address: Some(wallet.clone()),
        };

        let runtime = rt()?;
        let response = runtime
            .block_on(async { client().build_deposit(&body).await })
            .map_err(|e| format!("[khalani] build_deposit: {e}"))?
            .into_inner();
        let staged = extract_staged_txs(&response, &args.quote_id)?;

        let submit_template = json!({
            "quote_id": args.quote_id,
            "route_id": args.route_id,
            "wallet": wallet,
        });

        let preview = json!({
            "status": "ready_to_stage",
            "quote_id": args.quote_id,
            "route_id": args.route_id,
            "tx_count": staged.len(),
            "tx_targets": staged.iter().map(|t| t.to.clone()).collect::<Vec<_>>(),
            "execution_order": [
                "stage every evm_stage_tx route step in order",
                "simulate_batch with the returned pending_tx_id list",
                "evm_commit_txs with the same ordered pending_tx_id list",
                "submit_khalani_order with the tx_hash returned by evm_commit_txs"
            ],
        });

        let last_index = staged.len() - 1;
        let stage_args: Vec<Value> = staged
            .iter()
            .enumerate()
            .map(|(i, tx)| {
                json!({
                    "chain_id": tx.chain_id,
                    "to": tx.to,
                    "description": tx.description,
                    "data": { "raw": tx.data },
                    "value": tx.value,
                    "kind": if i == last_index { "bridge" } else { "erc20_approve" },
                })
            })
            .collect();

        ToolReturn::route(ok(preview)?)
            .next(|next| {
                for (i, args) in stage_args.iter().enumerate() {
                    let step = next.add_named("evm_stage_tx", args.clone());
                    if i == last_index {
                        step.note(
                            "Stage the Khalani deposit. CRITICAL: copy the `data.raw` and `to` \
                             fields BYTE-FOR-BYTE from the args below — do not abbreviate, \
                             reformat, or truncate the calldata.",
                        );
                    } else {
                        step.note(
                            "Stage the ERC-20 approval. CRITICAL: copy `data.raw` and `to` \
                             byte-for-byte; do not abbreviate or modify the calldata.",
                        );
                    }
                }
                next.add_named("simulate_batch", json!({ "transactions": [] }))
                    .note(
                        "After every evm_stage_tx step returns, replace `transactions` with the \
                         ordered staged ids, e.g. [{\"id\": 1}, {\"id\": 2}]. Do not simulate \
                         the deposit by itself; approvals must be first in the same batch.",
                    );
                next.add_named("evm_commit_txs", json!({ "tx_ids": [] }))
                    .bind_as("transaction_hash")
                    .note(
                        "Only after simulation succeeds, replace `tx_ids` with the same ordered \
                         pending_tx_id list and submit the batch. This route binds \
                         `transaction_hash`; use the returned `tx_hash` for the deferred \
                         submit_khalani_order step.",
                    );
            })
            .after::<SubmitOrder>(submit_template)
            .awaits("transaction_hash")
            .note(
                "Commit returned a transaction hash — register the order with Khalani using \
                 transaction_hash set to that tx hash.",
            )
            .try_build()
            .map_err(|e| format!("[khalani] route build: {e}"))
    }
}

// ============================================================================
// Tool 3: SubmitOrder — PUT /v1/deposit/submit (continuation)
// ============================================================================

pub(crate) struct SubmitOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SubmitOrderArgs {
    /// Quote id this submission belongs to.
    pub quote_id: String,
    /// Route id chosen at build time.
    #[serde(default)]
    pub route_id: Option<String>,
    /// Connected wallet address. Forwarded from `khalani_build_deposit` so the submission carries the deposit's `from`. The runtime fills it in automatically.
    #[serde(default)]
    #[allow(dead_code)]
    pub wallet: Option<String>,
    /// On-chain deposit transaction hash. Spliced in automatically by the OnBoundEvent continuation; never invent one.
    #[serde(default)]
    pub transaction_hash: Option<String>,
    /// EIP-712 signature when the chosen route uses signed-typed-data instead of a raw deposit tx. Optional.
    #[serde(default)]
    pub signature: Option<String>,
}

impl DynAomiTool for SubmitOrder {
    type App = KhalaniApp;
    type Args = SubmitOrderArgs;
    const NAME: &'static str = "submit_khalani_order";
    const DESCRIPTION: &'static str = "Register a confirmed Khalani deposit so the solver network can pick it up. Call after the routed `evm_commit_txs` step returns a tx hash; pass that hash as `transaction_hash`.";

    fn run(_app: &KhalaniApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let tx_hash = args.transaction_hash.clone();
        let body = DepositSubmitRequest {
            quote_id: Some(args.quote_id),
            route_id: args.route_id,
            signature: args.signature,
            submitted_data: Map::new(),
            transaction_hash: tx_hash.clone(),
            tx_hash,
        };
        let runtime = rt()?;
        let response = runtime
            .block_on(async { client().submit_deposit(&body).await })
            .map_err(|e| format!("[khalani] submit_deposit: {e}"))?
            .into_inner();
        ok(response)
    }
}

// ============================================================================
// Tool 4: OrderStatus — GET /v1/orders/{address}
// ============================================================================

pub(crate) struct OrderStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct OrderStatusArgs {
    /// Wallet address whose orders to fetch. Defaults to the host-connected EVM wallet.
    #[serde(default)]
    pub wallet: Option<String>,
    /// Comma-separated `orderId`s to filter to. Use this to follow a single recently-submitted order.
    #[serde(default)]
    pub order_ids: Option<String>,
    /// Filter by status (e.g. "FILLED", "PENDING", "FAILED"). Optional.
    #[serde(default)]
    pub status: Option<String>,
    /// Page size (default 20).
    #[serde(default)]
    pub limit: Option<i64>,
}

impl DynAomiTool for OrderStatus {
    type App = KhalaniApp;
    type Args = OrderStatusArgs;
    const NAME: &'static str = "khalani_order_status";
    const DESCRIPTION: &'static str = "Use after `submit_khalani_order` to poll an order until it reaches a terminal state (FILLED, FAILED, EXPIRED). Filter to a single order with `order_ids`. Returns the most recent orders for the wallet first.";

    fn run(_app: &KhalaniApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let address = resolve_evm_wallet(args.wallet, &ctx)?;
        let limit = Some(args.limit.unwrap_or(20));
        let runtime = rt()?;
        let response = runtime
            .block_on(async {
                client()
                    .get_orders_by_address(
                        &address,
                        limit,
                        None,
                        args.order_ids.as_deref(),
                        args.status.as_deref(),
                    )
                    .await
            })
            .map_err(|e| format!("[khalani] order_status: {e}"))?
            .into_inner();
        ok(response)
    }
}

// ============================================================================
// Tool 5: ListChains — GET /v1/chains
// ============================================================================

pub(crate) struct ListChains;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ListChainsArgs {}

impl DynAomiTool for ListChains {
    type App = KhalaniApp;
    type Args = ListChainsArgs;
    const NAME: &'static str = "khalani_list_chains";
    const DESCRIPTION: &'static str = "List the chains Khalani Hyperstream supports, with viem-style metadata (id, name, native currency). Use when the user asks 'what chains does Khalani support?' or you need a chain ID for a follow-up call.";

    fn run(_app: &KhalaniApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        // The `Chain` schema in ext/specs/khalani.yaml is already trimmed to
        // id/name/nativeCurrency — fields not in the spec are silently dropped
        // on deserialise, so the typed response IS the slim shape.
        let response = runtime
            .block_on(async { client().list_chains().await })
            .map_err(|e| format!("[khalani] list_chains: {e}"))?
            .into_inner();
        ok(json!({ "chains": response }))
    }
}

// ============================================================================
// Tool 6: SearchTokens — GET /v1/tokens/search
// ============================================================================

pub(crate) struct SearchTokens;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchTokensArgs {
    /// Substring match on symbol / name / address. Required.
    pub q: String,
    /// Restrict results to a single chain id.
    #[serde(default)]
    pub chain_id: Option<i64>,
    /// Page size (default 20).
    #[serde(default)]
    pub limit: Option<i64>,
}

impl DynAomiTool for SearchTokens {
    type App = KhalaniApp;
    type Args = SearchTokensArgs;
    const NAME: &'static str = "khalani_search_tokens";
    const DESCRIPTION: &'static str = "Search Khalani's supported-token catalogue by symbol, name, or address. Use to resolve a token symbol the user typed (e.g. 'USDC on Base') into the address + decimals you need for `khalani_quote`.";

    fn run(_app: &KhalaniApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let limit = Some(args.limit.unwrap_or(20));
        let runtime = rt()?;
        // `Token` in ext/specs/khalani.yaml is trimmed to address/chainId/
        // symbol/decimals/name — logoURI and extensions are dropped on
        // deserialise, so we forward the typed response directly.
        let response = runtime
            .block_on(async {
                client()
                    .search_tokens(args.chain_id, limit, None, &args.q)
                    .await
            })
            .map_err(|e| format!("[khalani] search_tokens: {e}"))?
            .into_inner();
        ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::extract_staged_txs;
    use aomi_ext::khalani::types::GetQuoteResponse;

    #[test]
    fn quote_response_allows_routes_without_supported_deposit_methods() {
        let sample = include_str!("../../../ext/specs/khalani.samples/getQuote.200.json");
        let quote: GetQuoteResponse =
            serde_json::from_str(sample).expect("quote sample should deserialize");

        let across = quote
            .routes
            .iter()
            .find(|route| route.route_id == "Across")
            .expect("sample should include Across route");

        assert!(across.quote.supported_deposit_methods.is_empty());
    }

    #[test]
    fn build_deposit_routes_preserve_chain_id_and_normalize_value() {
        let sample = include_str!("../../../ext/specs/khalani.samples/buildDeposit.200.json");
        let response = serde_json::from_str(sample).expect("build sample should deserialize");

        let staged = extract_staged_txs(&response, "quote-1").expect("sample should stage txs");

        assert!(!staged.is_empty());
        assert!(staged.iter().all(|tx| tx.chain_id == 1));
        assert!(staged.iter().all(|tx| tx.value == "0"));
    }
}
