use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

impl DynAomiTool for GetCowSwapQuote {
    type App = CowApp;
    type Args = GetCowSwapQuoteArgs;
    const NAME: &'static str = "get_cow_swap_quote";
    const DESCRIPTION: &'static str = "Get a CoW Protocol swap quote and return the exact EIP-712 typed data plus submission template that must be preserved for signing and order placement.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
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

        let mut quote = client.get_quote(&args.chain, payload)?;
        let order_to_sign = canonicalize_quote_order(&quote)?;
        let typed_data = build_cow_order_typed_data(chain_id, order_to_sign.clone());
        let submit_args_template = build_cow_submit_args_template(
            &args.chain,
            &order_to_sign,
            &sender_address,
            &signing_scheme,
        )?;

        if let Value::Object(ref mut map) = quote {
            let description = format!(
                "Sign CoW Protocol order: swap {} {} to {} on {}.",
                args.amount, args.sell_token, args.buy_token, args.chain
            );
            map.insert("order_to_sign".to_string(), order_to_sign);
            map.insert("typed_data".to_string(), typed_data.clone());
            map.insert("submit_args_template".to_string(), submit_args_template);
            if signing_scheme.eq_ignore_ascii_case("eip712") {
                map.insert(
                    "next_step_hint".to_string(),
                    Value::String(
                        "Use send_eip712_to_wallet with the exact typed_data above, then call place_cow_order with submit_args_template after replacing signed_order.signature with the wallet callback signature.".to_string(),
                    ),
                );
                map.insert(
                    "SYSTEM_NEXT_ACTION".to_string(),
                    json!([{
                        "name": "send_eip712_to_wallet",
                        "args": {
                            "typed_data": typed_data,
                            "description": description,
                        },
                        "reason": "CoW requires the exact EIP-712 order from this quote to be signed.",
                        "condition": "Only after the user confirms the quoted order details."
                    }]),
                );
            } else {
                map.insert(
                    "next_step_hint".to_string(),
                    Value::String(
                        "Preserve order_to_sign and submit_args_template exactly. If you use ethsign, sign the CoW order with that scheme and place the order with the returned wallet signature.".to_string(),
                    ),
                );
            }
        }

        Ok(quote)
    }
}

impl DynAomiTool for PlaceCowOrder {
    type App = CowApp;
    type Args = PlaceCowOrderArgs;
    const NAME: &'static str = "place_cow_order";
    const DESCRIPTION: &'static str = "Submit a signed CoW Protocol orderbook payload to CoW /orders API on the requested chain. Prefer using the submit_args_template returned by get_cow_swap_quote and only fill in the wallet signature.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.place_order(&args.chain, args.signed_order)
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
