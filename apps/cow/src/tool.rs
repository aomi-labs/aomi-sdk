use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

impl DynAomiTool for GetCowSwapQuote {
    type App = CowApp;
    type Args = GetCowSwapQuoteArgs;
    const NAME: &'static str = "get_cow_swap_quote";
    const DESCRIPTION: &'static str = "Get a CoW Protocol swap quote with fee estimation.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        let (chain_name, _) = get_chain_info(&args.chain)?;
        let sell_addr = get_token_address(chain_name, &args.sell_token)?;
        let buy_addr = get_token_address(chain_name, &args.buy_token)?;
        let decimals = get_token_decimals(chain_name, &args.sell_token);
        let amount_base = amount_to_base_units(args.amount, decimals)?;

        let mut payload = json!({
            "sellToken": sell_addr,
            "buyToken": buy_addr,
            "sellAmountBeforeFee": amount_base,
            "from": args.sender_address,
            "kind": args.order_side.clone().unwrap_or_else(|| "sell".to_string()),
        });
        if let Some(receiver) = args.receiver_address.clone() {
            payload["receiver"] = Value::String(receiver);
        }
        if let Some(valid_to) = args.valid_to {
            payload["validTo"] = json!(valid_to);
        }
        if let Some(partially_fillable) = args.partially_fillable {
            payload["partiallyFillable"] = json!(partially_fillable);
        }
        if let Some(signing_scheme) = args.signing_scheme.clone() {
            payload["signingScheme"] = Value::String(signing_scheme);
        }
        if let Some(slippage) = args.slippage {
            payload["slippageBps"] = json!((slippage * 10_000.0) as u32);
        }

        client.get_quote(&args.chain, payload)
    }
}

impl DynAomiTool for PlaceCowOrder {
    type App = CowApp;
    type Args = PlaceCowOrderArgs;
    const NAME: &'static str = "place_cow_order";
    const DESCRIPTION: &'static str = "Submit a signed CoW Protocol orderbook payload to CoW /orders API on the requested chain. Use the host's wallet/signing tools for any required user approval.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.place_order(&args.chain, args.signed_order)
    }
}

impl DynAomiTool for GetCowOrder {
    type App = CowApp;
    type Args = GetCowOrderArgs;
    const NAME: &'static str = "get_cow_order";
    const DESCRIPTION: &'static str =
        "Get the full order object for a CoW Protocol order by UID (status, executed amounts, fees, etc.).";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.get_order(&args.chain, &args.order_uid)
    }
}

impl DynAomiTool for GetCowOrderStatus {
    type App = CowApp;
    type Args = GetCowOrderStatusArgs;
    const NAME: &'static str = "get_cow_order_status";
    const DESCRIPTION: &'static str =
        "Get the competition status of a CoW Protocol order (open/scheduled/active/solved/executing/traded/cancelled).";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.get_order_status(&args.chain, &args.order_uid)
    }
}

impl DynAomiTool for GetCowUserOrders {
    type App = CowApp;
    type Args = GetCowUserOrdersArgs;
    const NAME: &'static str = "get_cow_user_orders";
    const DESCRIPTION: &'static str =
        "Get a paginated list of CoW Protocol orders for a given owner address, sorted by creation date.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.get_user_orders(
            &args.chain,
            &args.owner_address,
            args.offset,
            args.limit,
        )
    }
}

impl DynAomiTool for CancelCowOrders {
    type App = CowApp;
    type Args = CancelCowOrdersArgs;
    const NAME: &'static str = "cancel_cow_orders";
    const DESCRIPTION: &'static str =
        "Cancel one or more open CoW Protocol orders. Requires the cancellation signature from the order owner.";

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
                return Err(
                    "[cow] provide exactly one of `owner` or `order_uid`".to_string(),
                );
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
    const DESCRIPTION: &'static str =
        "Get the full lifecycle debug info for a CoW Protocol order, including events and auction participation.";

    fn run(_app: &CowApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = CowClient::new()?;
        client.debug_order(&args.chain, &args.order_uid)
    }
}
