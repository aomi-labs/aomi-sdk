use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

const COW_ORDER_FLOW_VERSION: &str = "cow-eip712-fee-rolled-into-sell-v1";
const COW_WALLET_SIGNATURE_CONDITION: &str =
    "Only after the user explicitly confirms the quoted order. Do not rewrite typed_data or ask for reconfirmation first.";
const COW_PLACE_ORDER_CONDITION: &str =
    "After wallet callback reports signature success; include only the callback signature and preserve submit_args_template exactly.";
const COW_NEXT_STEP_HINT: &str =
    "After confirmation, send wallet_signature_request as-is. After a successful wallet signature callback, call place_cow_order immediately with submit_args_template plus the callback signature. Do not claim success until place_cow_order returns success.";
const COW_SUBMISSION_GUARDRAIL: &str =
    "A CoW order is not submitted until place_cow_order returns success. Never invent an order UID or claim the order is live before that tool succeeds.";

fn build_cow_follow_up_actions(submit_args: Value) -> Value {
    json!([
        {
            "name": "send_eip712_to_wallet",
            "args_source": "wallet_signature_request",
            "reason": "CoW requires the exact EIP-712 wallet_signature_request from this quote to be signed.",
            "condition": COW_WALLET_SIGNATURE_CONDITION,
        },
        {
            "name": "place_cow_order",
            "args": submit_args,
            "reason": "Submit the exact quoted CoW order immediately after the matching wallet signature callback. Do not claim success unless this tool returns success.",
            "condition": COW_PLACE_ORDER_CONDITION,
        }
    ])
}

fn build_submission_normalization(quote: &Value, order_to_sign: &Value) -> Value {
    json!({
        "flow_version": COW_ORDER_FLOW_VERSION,
        "raw_quote_sell_amount": quote.pointer("/quote/sellAmount").cloned().unwrap_or(Value::Null),
        "raw_quote_fee_amount": quote.pointer("/quote/feeAmount").cloned().unwrap_or(Value::Null),
        "signable_sell_amount": order_to_sign.get("sellAmount").cloned().unwrap_or(Value::Null),
        "signable_fee_amount": order_to_sign.get("feeAmount").cloned().unwrap_or(Value::Null),
        "fee_rolled_into_sell_amount": true,
        "note": "For CoW sell orders, the raw quote fee is informational. The signable order rolls that fee into sellAmount and submits feeAmount as zero."
    })
}

impl DynAomiTool for GetCowSwapQuote {
    type App = CowApp;
    type Args = GetCowSwapQuoteArgs;
    const NAME: &'static str = "get_cow_swap_quote";
    const DESCRIPTION: &'static str = "Get a CoW Protocol swap quote and return the app-owned wallet_signature_request plus compact submit_args_template for signing and order placement.";

    fn run(_app: &CowApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        let (chain_name, chain_id) = get_chain_info(&args.chain)?;
        let sell_addr = get_token_address(chain_name, &args.sell_token)?;
        let buy_addr = get_token_address(chain_name, &args.buy_token)?;
        let decimals = get_token_decimals(chain_name, &args.sell_token);
        let amount_base = amount_to_base_units(args.amount, decimals)?;
        let sender_address = args.sender_address.clone();
        let receiver = args
            .receiver_address
            .clone()
            .unwrap_or_else(|| sender_address.clone());
        let order_side = args
            .order_side
            .clone()
            .unwrap_or_else(|| "sell".to_string());
        let signing_scheme = args
            .signing_scheme
            .clone()
            .unwrap_or_else(|| "eip712".to_string());

        let mut payload = json!({
            "sellToken": sell_addr,
            "buyToken": buy_addr,
            "sellAmountBeforeFee": amount_base,
            "from": sender_address.clone(),
            "receiver": receiver,
            "kind": order_side,
            "signingScheme": signing_scheme.clone(),
        });
        if let Some(valid_to) = args.valid_to {
            payload["validTo"] = json!(valid_to);
        }
        if let Some(partially_fillable) = args.partially_fillable {
            payload["partiallyFillable"] = json!(partially_fillable);
        }
        if let Some(slippage) = args.slippage {
            payload["slippageBps"] = json!((slippage * 10_000.0) as u32);
        }

        let quote = client.get_quote(&args.chain, payload)?;
        let order_to_sign = canonicalize_quote_order(&quote)?;
        let typed_data = build_cow_order_typed_data(chain_id, order_to_sign.clone());
        let description = format!(
            "Sign CoW Protocol order: swap {} {} to {} on {}.",
            args.amount, args.sell_token, args.buy_token, args.chain
        );
        let wallet_signature_request =
            build_cow_wallet_signature_request(&typed_data, &description)?;
        let quote_id = build_cow_quote_id(&ctx.session_id, &ctx.call_id);
        let orderbook_quote_id = quote.get("id").and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
        });
        let signed_order = build_cow_signed_order_payload(
            &order_to_sign,
            &sender_address,
            &signing_scheme,
            orderbook_quote_id,
        )?;
        let submit_args_template = build_cow_submit_args_template(&args.chain, &quote_id);
        let submission_normalization = build_submission_normalization(&quote, &order_to_sign);
        store_pending_cow_quote(&quote_id, &ctx.session_id, &args.chain, &signed_order)?;

        let mut response = json!({
            "expiration": quote.get("expiration").cloned().unwrap_or(Value::Null),
            "flow_version": COW_ORDER_FLOW_VERSION,
            "from": quote.get("from").cloned().unwrap_or(Value::String(sender_address)),
            "id": quote.get("id").cloned().unwrap_or(Value::Null),
            "next_step_hint": COW_NEXT_STEP_HINT,
            "protocolFeeBps": quote.get("protocolFeeBps").cloned().unwrap_or(Value::Null),
            "quote": quote.get("quote").cloned().unwrap_or(Value::Null),
            "quote_id": quote_id,
            "source": quote.get("source").cloned().unwrap_or(Value::String("cow".to_string())),
            "submission_guardrail": COW_SUBMISSION_GUARDRAIL,
            "submission_normalization": submission_normalization,
            "submit_args_template": submit_args_template.clone(),
            "verified": quote.get("verified").cloned().unwrap_or(Value::Bool(false)),
            "wallet_signature_request": wallet_signature_request,
        });

        if signing_scheme.eq_ignore_ascii_case("eip712") {
            response["SYSTEM_NEXT_ACTION"] =
                build_cow_follow_up_actions(submit_args_template);
        } else {
            response["next_step_hint"] = Value::String(
                "Use wallet_signature_request for signing, preserve submit_args_template exactly, and place the order with the returned wallet signature.".to_string(),
            );
        }

        Ok(response)
    }
}

impl DynAomiTool for PlaceCowOrder {
    type App = CowApp;
    type Args = PlaceCowOrderArgs;
    const NAME: &'static str = "place_cow_order";
    const DESCRIPTION: &'static str = "Submit a signed CoW Protocol orderbook payload to CoW /orders API on the requested chain. Prefer using the submit_args_template returned by get_cow_swap_quote and only fill in the wallet signature.";

    fn run(_app: &CowApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        let resolved = resolve_cow_order_submission(
            &ctx.session_id,
            &args.chain,
            args.quote_id.as_deref(),
            args.signed_order,
            args.signature.as_deref(),
        )?;
        verify_cow_order_signature(&resolved.chain, &resolved.signed_order)?;
        client.place_order(&resolved.chain, resolved.signed_order)
    }
}

impl DynAomiTool for GetCowOrder {
    type App = CowApp;
    type Args = GetCowOrderArgs;
    const NAME: &'static str = "get_cow_order";
    const DESCRIPTION: &'static str = "Get the full order object for a CoW Protocol order by UID (status, executed amounts, fees, etc.).";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.get_order(&args.chain, &args.order_uid)
    }
}

impl DynAomiTool for GetCowOrderStatus {
    type App = CowApp;
    type Args = GetCowOrderStatusArgs;
    const NAME: &'static str = "get_cow_order_status";
    const DESCRIPTION: &'static str = "Get the competition status of a CoW Protocol order (open/scheduled/active/solved/executing/traded/cancelled).";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.get_order_status(&args.chain, &args.order_uid)
    }
}

impl DynAomiTool for GetCowUserOrders {
    type App = CowApp;
    type Args = GetCowUserOrdersArgs;
    const NAME: &'static str = "get_cow_user_orders";
    const DESCRIPTION: &'static str = "Get a paginated list of CoW Protocol orders for a given owner address, sorted by creation date.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.get_user_orders(&args.chain, &args.owner_address, args.offset, args.limit)
    }
}

impl DynAomiTool for CancelCowOrders {
    type App = CowApp;
    type Args = CancelCowOrdersArgs;
    const NAME: &'static str = "cancel_cow_orders";
    const DESCRIPTION: &'static str = "Cancel one or more open CoW Protocol orders. Requires the cancellation signature from the order owner.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        let payload = json!({
            "orderUids": args.order_uids,
            "signature": args.signature,
            "signingScheme": args.signing_scheme,
        });
        client.cancel_orders(&args.chain, payload)
    }
}

impl DynAomiTool for GetCowTrades {
    type App = CowApp;
    type Args = GetCowTradesArgs;
    const NAME: &'static str = "get_cow_trades";
    const DESCRIPTION: &'static str =
        "Get trade execution history from CoW Protocol. Provide exactly one of owner or order_uid.";

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
        let client = CowClient::new()?;
        client.get_trades(
            &args.chain,
            args.owner.as_deref(),
            args.order_uid.as_deref(),
            args.offset,
            args.limit,
        )
    }
}

impl DynAomiTool for GetCowNativePrice {
    type App = CowApp;
    type Args = GetCowNativePriceArgs;
    const NAME: &'static str = "get_cow_native_price";
    const DESCRIPTION: &'static str =
        "Get the price of a token relative to the chain's native currency via CoW Protocol.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.get_native_price(&args.chain, &args.token_address)
    }
}

impl DynAomiTool for GetCowOrdersByTx {
    type App = CowApp;
    type Args = GetCowOrdersByTxArgs;
    const NAME: &'static str = "get_cow_orders_by_tx";
    const DESCRIPTION: &'static str =
        "Get all CoW Protocol orders that were settled in a specific transaction.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.get_orders_by_tx(&args.chain, &args.tx_hash)
    }
}

impl DynAomiTool for DebugCowOrder {
    type App = CowApp;
    type Args = DebugCowOrderArgs;
    const NAME: &'static str = "debug_cow_order";
    const DESCRIPTION: &'static str = "Get the full lifecycle debug info for a CoW Protocol order, including events and auction participation.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.debug_order(&args.chain, &args.order_uid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_quote_response_fits_persistence_budget() {
        let quote = json!({
            "expiration": "2026-04-29T02:02:03.084220776Z",
            "from": "0x9cb9ec43b1dcbe0ea37bfa9a99f2c9aae2ebf2eb",
            "id": 8396734,
            "protocolFeeBps": "2",
            "quote": {
                "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "buyAmount": "41932174910877",
                "buyToken": "0x7ceb23fd6bc0add59e62ac25578270cff1b9f619",
                "buyTokenBalance": "erc20",
                "feeAmount": "3772",
                "gasAmount": "208119",
                "gasPrice": "198318424205",
                "kind": "sell",
                "partiallyFillable": false,
                "receiver": "0x9cb9ec43b1dcbe0ea37bfa9a99f2c9aae2ebf2eb",
                "sellAmount": "96228",
                "sellToken": "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",
                "sellTokenBalance": "erc20",
                "validTo": 1777429803
            },
            "source": "cow",
            "verified": true
        });
        let order_to_sign = json!({
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "buyAmount": "41932174910877",
            "buyToken": "0x7ceb23fd6bc0add59e62ac25578270cff1b9f619",
            "buyTokenBalance": "erc20",
            "feeAmount": "0",
            "kind": "sell",
            "partiallyFillable": false,
            "receiver": "0x9cb9ec43b1dcbe0ea37bfa9a99f2c9aae2ebf2eb",
            "sellAmount": "100000",
            "sellToken": "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359",
            "sellTokenBalance": "erc20",
            "validTo": 1777429803
        });
        let typed_data = build_cow_order_typed_data(137, order_to_sign.clone());
        let wallet_signature_request = build_cow_wallet_signature_request(
            &typed_data,
            "Sign CoW Protocol order: swap 0.1 USDC to WETH on polygon.",
        )
        .expect("wallet signature request should build");
        let submit_args_template = build_cow_submit_args_template("polygon", "cowq:test:1");
        let response = json!({
            "expiration": quote["expiration"],
            "flow_version": COW_ORDER_FLOW_VERSION,
            "from": quote["from"],
            "id": quote["id"],
            "next_step_hint": "After the user confirms, follow SYSTEM_NEXT_ACTION exactly. Use wallet_signature_request as-is, treat raw quote fee fields as informational only, preserve submit_args_template exactly, and let place_cow_order consume the matching wallet callback signature.",
            "orderbook_quote_id": quote["id"],
            "protocolFeeBps": quote["protocolFeeBps"],
            "quote": quote["quote"],
            "quote_id": "cowq:test:1",
            "source": quote["source"],
            "submission_guardrail": "A CoW order is not submitted until place_cow_order returns success. Never invent an order UID or claim the order is live before that tool succeeds.",
            "submission_normalization": build_submission_normalization(&quote, &order_to_sign),
            "submit_args_template": submit_args_template.clone(),
            "verified": quote["verified"],
            "wallet_signature_request": wallet_signature_request,
            "SYSTEM_NEXT_ACTION": build_cow_follow_up_actions(submit_args_template),
        });

        let serialized =
            serde_json::to_string_pretty(&response).expect("response should serialize");
        assert!(
            serialized.chars().count() < 6_000,
            "response exceeded persistence budget: {} chars",
            serialized.chars().count()
        );
    }
}
