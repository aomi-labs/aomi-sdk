use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

fn resolve_bybit_credentials(
    api_key: Option<&str>,
    secret_key: Option<&str>,
) -> Result<(String, String), String> {
    let api_key = resolve_secret_value(
        api_key,
        "BYBIT_API_KEY",
        "[bybit] missing api_key argument and BYBIT_API_KEY environment variable",
    )?;
    let secret_key = resolve_secret_value(
        secret_key,
        "BYBIT_SECRET_KEY",
        "[bybit] missing secret_key argument and BYBIT_SECRET_KEY environment variable",
    )?;
    Ok((api_key, secret_key))
}

// ============================================================================
// Tool 1: GetTickers (public)
// ============================================================================

impl DynAomiTool for GetTickers {
    type App = BybitApp;
    type Args = GetTickersArgs;
    const NAME: &'static str = "bybit_get_tickers";
    const DESCRIPTION: &'static str =
        "Get price tickers for a given category. Optionally filter by symbol.";

    fn run(_app: &BybitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BybitClient::new()?;
        let mut params = vec![format!("category={}", args.category)];
        if let Some(ref sym) = args.symbol {
            params.push(format!("symbol={sym}"));
        }
        let query = params.join("&");
        let resp = client.public_get("/market/tickers", &query)?;
        Ok(resp)
    }
}

// ============================================================================
// Tool 2: GetOrderbook (public)
// ============================================================================

impl DynAomiTool for GetOrderbook {
    type App = BybitApp;
    type Args = GetOrderbookArgs;
    const NAME: &'static str = "bybit_get_orderbook";
    const DESCRIPTION: &'static str = "Get order book snapshot for a symbol.";

    fn run(_app: &BybitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BybitClient::new()?;
        let mut params = vec![
            format!("category={}", args.category),
            format!("symbol={}", args.symbol),
        ];
        if let Some(limit) = args.limit {
            params.push(format!("limit={limit}"));
        }
        let query = params.join("&");
        let resp = client.public_get("/market/orderbook", &query)?;
        Ok(resp)
    }
}

// ============================================================================
// Tool 3: GetKline (public)
// ============================================================================

impl DynAomiTool for GetKline {
    type App = BybitApp;
    type Args = GetKlineArgs;
    const NAME: &'static str = "bybit_get_kline";
    const DESCRIPTION: &'static str =
        "Get candlestick/kline data for a symbol at a given interval.";

    fn run(_app: &BybitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BybitClient::new()?;
        let mut params = vec![
            format!("category={}", args.category),
            format!("symbol={}", args.symbol),
            format!("interval={}", args.interval),
        ];
        if let Some(start) = args.start {
            params.push(format!("start={start}"));
        }
        if let Some(end) = args.end {
            params.push(format!("end={end}"));
        }
        let query = params.join("&");
        let resp = client.public_get("/market/kline", &query)?;
        Ok(resp)
    }
}

// ============================================================================
// Tool 4: CreateOrder (authenticated POST)
// ============================================================================

impl DynAomiTool for CreateOrder {
    type App = BybitApp;
    type Args = CreateOrderArgs;
    const NAME: &'static str = "bybit_create_order";
    const DESCRIPTION: &'static str =
        "Place a new order. Requires api_key and secret_key for authentication.";

    fn run(_app: &BybitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BybitClient::new()?;
        let (api_key, secret_key) =
            resolve_bybit_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        let mut body = json!({
            "category": args.category,
            "symbol": args.symbol,
            "side": args.side,
            "orderType": args.order_type,
            "qty": args.qty,
        });
        if let Some(ref price) = args.price {
            body.as_object_mut()
                .unwrap()
                .insert("price".to_string(), json!(price));
        }
        let resp = client.auth_post("/order/create", &body, &api_key, &secret_key)?;
        Ok(resp)
    }
}

// ============================================================================
// Tool 5: CancelOrder (authenticated POST)
// ============================================================================

impl DynAomiTool for CancelOrder {
    type App = BybitApp;
    type Args = CancelOrderArgs;
    const NAME: &'static str = "bybit_cancel_order";
    const DESCRIPTION: &'static str =
        "Cancel an existing order. Requires api_key and secret_key for authentication.";

    fn run(_app: &BybitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BybitClient::new()?;
        let (api_key, secret_key) =
            resolve_bybit_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        let body = json!({
            "category": args.category,
            "symbol": args.symbol,
            "orderId": args.order_id,
        });
        let resp = client.auth_post("/order/cancel", &body, &api_key, &secret_key)?;
        Ok(resp)
    }
}

// ============================================================================
// Tool 6: AmendOrder (authenticated POST)
// ============================================================================

impl DynAomiTool for AmendOrder {
    type App = BybitApp;
    type Args = AmendOrderArgs;
    const NAME: &'static str = "bybit_amend_order";
    const DESCRIPTION: &'static str = "Modify an existing order (quantity and/or price). Requires api_key and secret_key for authentication.";

    fn run(_app: &BybitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BybitClient::new()?;
        let (api_key, secret_key) =
            resolve_bybit_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        let mut body = json!({
            "category": args.category,
            "symbol": args.symbol,
            "orderId": args.order_id,
        });
        if let Some(ref qty) = args.qty {
            body.as_object_mut()
                .unwrap()
                .insert("qty".to_string(), json!(qty));
        }
        if let Some(ref price) = args.price {
            body.as_object_mut()
                .unwrap()
                .insert("price".to_string(), json!(price));
        }
        let resp = client.auth_post("/order/amend", &body, &api_key, &secret_key)?;
        Ok(resp)
    }
}

// ============================================================================
// Tool 7: GetPositions (authenticated GET)
// ============================================================================

impl DynAomiTool for GetPositions {
    type App = BybitApp;
    type Args = GetPositionsArgs;
    const NAME: &'static str = "bybit_get_positions";
    const DESCRIPTION: &'static str =
        "Get open positions. Requires api_key and secret_key for authentication.";

    fn run(_app: &BybitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BybitClient::new()?;
        let (api_key, secret_key) =
            resolve_bybit_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        let mut params = vec![format!("category={}", args.category)];
        if let Some(ref sym) = args.symbol {
            params.push(format!("symbol={sym}"));
        }
        let query = params.join("&");
        let resp = client.auth_get("/position/list", &query, &api_key, &secret_key)?;
        Ok(resp)
    }
}

// ============================================================================
// Tool 8: GetWalletBalance (authenticated GET)
// ============================================================================

impl DynAomiTool for GetWalletBalance {
    type App = BybitApp;
    type Args = GetWalletBalanceArgs;
    const NAME: &'static str = "bybit_get_wallet_balance";
    const DESCRIPTION: &'static str =
        "Get account wallet balance. Requires api_key and secret_key for authentication.";

    fn run(_app: &BybitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BybitClient::new()?;
        let (api_key, secret_key) =
            resolve_bybit_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        let query = format!("accountType={}", args.account_type);
        let resp = client.auth_get("/account/wallet-balance", &query, &api_key, &secret_key)?;
        Ok(resp)
    }
}

// ============================================================================
// Tool 9: SetLeverage (authenticated POST)
// ============================================================================

impl DynAomiTool for SetLeverage {
    type App = BybitApp;
    type Args = SetLeverageArgs;
    const NAME: &'static str = "bybit_set_leverage";
    const DESCRIPTION: &'static str =
        "Set leverage for a symbol. Requires api_key and secret_key for authentication.";

    fn run(_app: &BybitApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BybitClient::new()?;
        let (api_key, secret_key) =
            resolve_bybit_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        let body = json!({
            "category": args.category,
            "symbol": args.symbol,
            "buyLeverage": args.buy_leverage,
            "sellLeverage": args.sell_leverage,
        });
        let resp = client.auth_post("/position/set-leverage", &body, &api_key, &secret_key)?;
        Ok(resp)
    }
}
