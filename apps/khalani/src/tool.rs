use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[khalani] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("khalani".to_string()));
            Value::Object(map)
        }
        other => serde_json::json!({ "source": "khalani", "data": other }),
    })
}

fn to_json_value<T: Serialize>(value: &T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("failed to encode JSON payload: {e}"))
}

impl DynAomiTool for GetKhalaniQuote {
    type App = KhalaniApp;
    type Args = GetKhalaniQuoteArgs;

    const NAME: &'static str = "get_khalani_quote";
    const DESCRIPTION: &'static str =
        "Fetch a Khalani quote for a same-chain or cross-chain swap route.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = KhalaniClient::new()?;
        let sender_address = resolve_sender_address(&ctx, args.sender_address.as_deref())?;
        let destination_chain = args
            .destination_chain
            .clone()
            .unwrap_or_else(|| args.chain.clone());
        let from_chain_id = resolve_chain_id(&args.chain)?;
        let to_chain_id = resolve_chain_id(&destination_chain)?;
        let sell_token = client.resolve_token(&args.sell_token, from_chain_id)?;
        let buy_token = client.resolve_token(&args.buy_token, to_chain_id)?;
        let amount_base_units = amount_to_base_units(args.amount, sell_token.decimals)?;

        ok(client.get_quote(
            from_chain_id,
            to_chain_id,
            &sell_token.address,
            &buy_token.address,
            &amount_base_units,
            &sender_address,
            args.receiver_address.as_deref(),
            slippage_to_bps(args.slippage),
        )?)
    }
}

// ============================================================================
// Tool 2: build_khalani_order
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct BuildKhalaniOrderArgs {
    /// Source chain: ethereum, arbitrum, polygon, base, etc.
    chain: String,
    /// Destination chain for cross-chain routes. Defaults to the source chain.
    destination_chain: Option<String>,
    /// Token to swap from.
    sell_token: String,
    /// Token to swap to.
    buy_token: String,
    /// Human-readable sell amount.
    amount: f64,
    /// Sender/taker wallet address. Defaults to the connected wallet.
    sender_address: Option<String>,
    /// Recipient wallet address. Defaults to sender.
    receiver_address: Option<String>,
    /// Slippage decimal (0.005 = 0.5%).
    slippage: Option<f64>,
}

pub(crate) struct BuildKhalaniOrder;

impl DynAomiTool for BuildKhalaniOrder {
    type App = KhalaniApp;
    type Args = BuildKhalaniOrderArgs;

    const NAME: &'static str = "build_khalani_order";
    const DESCRIPTION: &'static str = "Build a Khalani execution step and return the next explicit wallet action. This tool never sends the wallet request itself.";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let client = KhalaniClient::new()?;
        let sender_address = resolve_sender_address(&ctx, args.sender_address.as_deref())?;
        let destination_chain = args
            .destination_chain
            .clone()
            .unwrap_or_else(|| args.chain.clone());
        let from_chain_id = resolve_chain_id(&args.chain)?;
        let to_chain_id = resolve_chain_id(&destination_chain)?;
        let sell_token = client.resolve_token(&args.sell_token, from_chain_id)?;
        let buy_token = client.resolve_token(&args.buy_token, to_chain_id)?;
        let amount_base_units = amount_to_base_units(args.amount, sell_token.decimals)?;

        // Step 1: Get a fresh quote
        let quote = client.get_quote(
            from_chain_id,
            to_chain_id,
            &sell_token.address,
            &buy_token.address,
            &amount_base_units,
            &sender_address,
            args.receiver_address.as_deref(),
            slippage_to_bps(args.slippage),
        )?;

        let quote_entry = normalize_khalani_quote_response(&quote)
            .ok_or_else(|| "Khalani quote response is empty".to_string())?;
        let quote_id = extract_khalani_quote_id(&quote_entry)
            .ok_or_else(|| "Khalani quote missing quoteId".to_string())?;
        let route_id = extract_khalani_route_id(&quote_entry);
        let summary = extract_quote_summary(&quote_entry);

        let slippage_bps = slippage_to_bps(args.slippage);

        // Step 2: Try multiple build payload variants
        let mut build_payloads: Vec<Value> = Vec::new();

        if let Some(ref rid) = route_id {
            build_payloads.push(to_json_value(&KhalaniDepositBuildCanonicalRequest {
                from: sender_address.clone(),
                quote_id: quote_id.clone(),
                route_id: rid.clone(),
            })?);
            build_payloads.push(to_json_value(&KhalaniDepositBuildFromAddressRequest {
                from_address: sender_address.clone(),
                quote_id: quote_id.clone(),
                route_id: rid.clone(),
                deposit_method: "CONTRACT_CALL",
                slippage_in_bps: slippage_bps,
            })?);
            build_payloads.push(to_json_value(&KhalaniDepositBuildUserAddressRequest {
                user_address: sender_address.clone(),
                quote_id: quote_id.clone(),
                route_id: rid.clone(),
                deposit_method: "CONTRACT_CALL",
                slippage_in_bps: slippage_bps,
            })?);
        }

        build_payloads.push(to_json_value(&KhalaniDepositBuildLegacyRequest {
            quote_id: quote_id.clone(),
            user: sender_address.clone(),
            allowance_target: extract_khalani_allowance_target(&quote_entry),
            slippage_in_bps: slippage_bps,
        })?);

        let mut build: Option<Value> = None;
        let mut last_build_error: Option<String> = None;
        for payload in build_payloads {
            match client.build_deposit(&payload) {
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

        // Step 3: Determine flow from build response

        // Approval flow: find the first executable tx.
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
                        "args_template": to_json_value(&SubmitKhalaniOrderArgs {
                            quote_id: quote_id.clone(),
                            route_id: route_id.clone(),
                            submit_type: "SIGNED_TRANSACTION".to_string(),
                            transaction_hash: None,
                            signature: None,
                        })?,
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
                        "args_template": to_json_value(&BuildKhalaniOrderArgs {
                            chain: args.chain.clone(),
                            destination_chain: args.destination_chain.clone(),
                            sell_token: args.sell_token.clone(),
                            buy_token: args.buy_token.clone(),
                            amount: args.amount,
                            sender_address: Some(sender_address.clone()),
                            receiver_address: args.receiver_address.clone(),
                            slippage: args.slippage,
                        })?,
                    }),
                )
            };

            return build_khalani_result(
                &quote_id,
                &route_id,
                "APPROVAL_FLOW",
                summary,
                "stage_tx",
                build_stage_tx_request(&tx, description),
                build_transaction_preflight(&tx),
                follow_up,
            );
        }

        // Permit2 / EIP-712 signature flow.
        let tx_type = extract_khalani_transaction_type(&build);
        if tx_type.eq_ignore_ascii_case("PERMIT2") {
            let typed_data = extract_khalani_typed_data(&build)
                .ok_or_else(|| "Khalani build missing typed data".to_string())?;
            return build_khalani_result(
                &quote_id,
                &route_id,
                &tx_type,
                summary,
                "commit_eip712",
                to_json_value(&WalletEip712Request {
                    typed_data,
                    description: format!(
                        "Khalani Permit2 signature for {} {} -> {}",
                        args.amount, args.sell_token, args.buy_token
                    ),
                })?,
                None,
                json!({
                    "step": "submit_khalani_order",
                    "args_template": to_json_value(&SubmitKhalaniOrderArgs {
                        quote_id: quote_id.clone(),
                        route_id: route_id.clone(),
                        submit_type: "SIGNED_EIP712".to_string(),
                        transaction_hash: None,
                        signature: None,
                    })?,
                }),
            );
        }

        // Contract call execution flow.
        let tx = extract_khalani_contract_call_tx(&build)
            .ok_or_else(|| "Khalani build missing executable transaction".to_string())?;
        build_khalani_result(
            &quote_id,
            &route_id,
            &tx_type,
            summary,
            "stage_tx",
            build_stage_tx_request(
                &tx,
                format!(
                    "Khalani swap {} {} to {} on {}",
                    args.amount, args.sell_token, args.buy_token, args.chain
                ),
            ),
            build_transaction_preflight(&tx),
            json!({
                "step": "submit_khalani_order",
                "args_template": to_json_value(&SubmitKhalaniOrderArgs {
                    quote_id: quote_id.clone(),
                    route_id: route_id.clone(),
                    submit_type: "SIGNED_TRANSACTION".to_string(),
                    transaction_hash: None,
                    signature: None,
                })?,
            }),
        )
    }
}

// ============================================================================
// Tool 3: submit_khalani_order
// ============================================================================

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SubmitKhalaniOrderArgs {
    /// Khalani quote ID to submit.
    quote_id: String,
    /// Khalani route ID when provided by build output.
    route_id: Option<String>,
    /// SIGNED_TRANSACTION or SIGNED_EIP712.
    submit_type: String,
    /// Wallet transaction hash for SIGNED_TRANSACTION submit.
    transaction_hash: Option<String>,
    /// Wallet signature for SIGNED_EIP712 submit.
    signature: Option<String>,
}

pub(crate) struct SubmitKhalaniOrder;

impl DynAomiTool for SubmitKhalaniOrder {
    type App = KhalaniApp;
    type Args = SubmitKhalaniOrderArgs;

    const NAME: &'static str = "submit_khalani_order";
    const DESCRIPTION: &'static str =
        "Submit a wallet-completed Khalani order using a transaction hash or EIP-712 signature.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = KhalaniClient::new()?;
        let submit_type = args.submit_type.to_uppercase();
        let value = match submit_type.as_str() {
            "SIGNED_EIP712" => {
                let signature = args.signature.clone().ok_or_else(|| {
                    "submit_khalani_order requires signature for SIGNED_EIP712".to_string()
                })?;
                if let Some(route_id) = args.route_id.clone() {
                    client.submit_deposit(&KhalaniSignedEip712SubmitRequest {
                        quote_id: args.quote_id,
                        route_id,
                        signature,
                    })?
                } else {
                    client.submit_deposit(&KhalaniLegacySubmitRequest {
                        quote_id: args.quote_id,
                        submitted_data: KhalaniSignedEip712SubmittedData {
                            submit_type: "SIGNED_EIP712",
                            signature,
                        },
                    })?
                }
            }
            "SIGNED_TRANSACTION" => {
                let tx_hash = args.transaction_hash.clone().ok_or_else(|| {
                    "submit_khalani_order requires transaction_hash for SIGNED_TRANSACTION"
                        .to_string()
                })?;
                if let Some(route_id) = args.route_id.clone() {
                    client.submit_deposit(&KhalaniSignedTransactionSubmitRequest {
                        quote_id: args.quote_id,
                        route_id,
                        tx_hash: tx_hash.clone(),
                        transaction_hash: tx_hash,
                    })?
                } else {
                    client.submit_deposit(&KhalaniLegacySubmitRequest {
                        quote_id: args.quote_id,
                        submitted_data: KhalaniSignedTransactionSubmittedData {
                            submit_type: "SIGNED_TRANSACTION",
                            transaction_hash: tx_hash,
                        },
                    })?
                }
            }
            other => {
                return Err(format!(
                    "submit_khalani_order unsupported submit_type '{other}'"
                ));
            }
        };
        ok(value)
    }
}

// ============================================================================
// Tool 4: get_khalani_order_status
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetKhalaniOrderStatusArgs {
    /// Khalani order ID.
    order_id: String,
    /// Optional wallet address used for the documented orders lookup. Defaults to the connected wallet.
    address: Option<String>,
}

pub(crate) struct GetKhalaniOrderStatus;

impl DynAomiTool for GetKhalaniOrderStatus {
    type App = KhalaniApp;
    type Args = GetKhalaniOrderStatusArgs;

    const NAME: &'static str = "get_khalani_order_status";
    const DESCRIPTION: &'static str = "Fetch a Khalani order by order_id.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = KhalaniClient::new()?;
        let address = resolve_sender_address(&ctx, args.address.as_deref())?;
        let value =
            client.get_orders_by_address(&address, None, Some(1), Some(0), Some(&args.order_id))?;
        let order = value
            .get("data")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .cloned()
            .ok_or_else(|| format!("No Khalani order found for order_id '{}'", args.order_id))?;

        Ok(json!({
            "source": "khalani",
            "order": order,
        }))
    }
}

// ============================================================================
// Tool 5: get_khalani_orders_by_address
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetKhalaniOrdersByAddressArgs {
    /// Wallet address.
    address: String,
    /// Optional Khalani order status filter.
    status: Option<String>,
    /// Page size.
    limit: Option<u32>,
    /// Pagination offset.
    offset: Option<u32>,
}

pub(crate) struct GetKhalaniOrdersByAddress;

impl DynAomiTool for GetKhalaniOrdersByAddress {
    type App = KhalaniApp;
    type Args = GetKhalaniOrdersByAddressArgs;

    const NAME: &'static str = "get_khalani_orders_by_address";
    const DESCRIPTION: &'static str = "Fetch Khalani orders for a wallet address.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(KhalaniClient::new()?.get_orders_by_address(
            &args.address,
            args.status.as_deref(),
            args.limit,
            args.offset,
            None,
        )?)
    }
}

// ============================================================================
// Tool 6: get_khalani_tokens
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetKhalaniTokensArgs {
    /// Numeric chain ID filter.
    chain_id: Option<u64>,
    /// Page size.
    limit: Option<u32>,
    /// Pagination offset.
    offset: Option<u32>,
    /// Token symbol/name search string.
    query: Option<String>,
}

pub(crate) struct GetKhalaniTokens;

impl DynAomiTool for GetKhalaniTokens {
    type App = KhalaniApp;
    type Args = GetKhalaniTokensArgs;

    const NAME: &'static str = "get_khalani_tokens";
    const DESCRIPTION: &'static str = "Fetch Khalani tokens with optional chain/query filters.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(KhalaniClient::new()?.get_tokens(
            args.chain_id,
            args.limit,
            args.offset,
            args.query.as_deref(),
        )?)
    }
}

// ============================================================================
// Tool 7: search_khalani_tokens
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct SearchKhalaniTokensArgs {
    /// Token symbol/name search string.
    query: String,
    /// Numeric chain ID filter.
    chain_id: Option<u64>,
    /// Maximum results.
    limit: Option<u32>,
    /// Pagination offset.
    offset: Option<u32>,
}

pub(crate) struct SearchKhalaniTokens;

impl DynAomiTool for SearchKhalaniTokens {
    type App = KhalaniApp;
    type Args = SearchKhalaniTokensArgs;

    const NAME: &'static str = "search_khalani_tokens";
    const DESCRIPTION: &'static str = "Search Khalani token metadata.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(KhalaniClient::new()?.search_tokens(
            &args.query,
            args.chain_id,
            args.limit,
            args.offset,
        )?)
    }
}

// ============================================================================
// Tool 8: get_khalani_chains
// ============================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetKhalaniChainsArgs {}

pub(crate) struct GetKhalaniChains;

impl DynAomiTool for GetKhalaniChains {
    type App = KhalaniApp;
    type Args = GetKhalaniChainsArgs;

    const NAME: &'static str = "get_khalani_chains";
    const DESCRIPTION: &'static str = "Fetch Khalani supported chains.";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(KhalaniClient::new()?.get_chains()?)
    }
}
