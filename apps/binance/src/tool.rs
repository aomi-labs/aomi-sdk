use crate::client::*;
use aomi_sdk::*;
use serde_json::Value;

fn resolve_binance_credentials(
    api_key: Option<&str>,
    secret_key: Option<&str>,
) -> Result<(String, String), String> {
    let api_key = resolve_secret_value(
        api_key,
        "BINANCE_API_KEY",
        "[binance] missing api_key argument and BINANCE_API_KEY environment variable",
    )?;
    let secret_key = resolve_secret_value(
        secret_key,
        "BINANCE_SECRET_KEY",
        "[binance] missing secret_key argument and BINANCE_SECRET_KEY environment variable",
    )?;
    Ok((api_key, secret_key))
}

// ============================================================================
// Tool 1: GetPrice — GET /ticker/price (public)
// ============================================================================

impl DynAomiTool for GetPrice {
    type App = BinanceApp;
    type Args = GetPriceArgs;
    const NAME: &'static str = "binance_get_price";
    const DESCRIPTION: &'static str =
        "Get the latest price for a trading pair, or all trading pairs if no symbol is specified.";

    fn run(_app: &BinanceApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BinanceClient::new()?;
        let query = match &args.symbol {
            Some(s) => format!("symbol={s}"),
            None => String::new(),
        };
        client.public_get(SPOT_BASE_URL, "/ticker/price", &query)
    }
}

// ============================================================================
// Tool 2: GetDepth — GET /depth (public)
// ============================================================================

impl DynAomiTool for GetDepth {
    type App = BinanceApp;
    type Args = GetDepthArgs;
    const NAME: &'static str = "binance_get_depth";
    const DESCRIPTION: &'static str = "Get order book depth (bids and asks) for a trading pair.";

    fn run(_app: &BinanceApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BinanceClient::new()?;
        let mut query = format!("symbol={}", args.symbol);
        if let Some(limit) = args.limit {
            query.push_str(&format!("&limit={limit}"));
        }
        client.public_get(SPOT_BASE_URL, "/depth", &query)
    }
}

// ============================================================================
// Tool 3: GetKlines — GET /klines (public)
// ============================================================================

impl DynAomiTool for GetKlines {
    type App = BinanceApp;
    type Args = GetKlinesArgs;
    const NAME: &'static str = "binance_get_klines";
    const DESCRIPTION: &'static str = "Get candlestick/kline data for a trading pair. Returns arrays of [open_time, open, high, low, close, volume, close_time, quote_volume, trades, taker_buy_base_vol, taker_buy_quote_vol, ignore].";

    fn run(_app: &BinanceApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BinanceClient::new()?;
        let mut query = format!("symbol={}&interval={}", args.symbol, args.interval);
        if let Some(start) = args.start_time {
            query.push_str(&format!("&startTime={start}"));
        }
        if let Some(end) = args.end_time {
            query.push_str(&format!("&endTime={end}"));
        }
        if let Some(limit) = args.limit {
            query.push_str(&format!("&limit={limit}"));
        }
        client.public_get(SPOT_BASE_URL, "/klines", &query)
    }
}

// ============================================================================
// Tool 4: Get24hrStats — GET /ticker/24hr (public)
// ============================================================================

impl DynAomiTool for Get24hrStats {
    type App = BinanceApp;
    type Args = Get24hrStatsArgs;
    const NAME: &'static str = "binance_get_24hr_stats";
    const DESCRIPTION: &'static str = "Get 24-hour rolling window price change statistics for a trading pair, or all pairs if no symbol is specified.";

    fn run(_app: &BinanceApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BinanceClient::new()?;
        let query = match &args.symbol {
            Some(s) => format!("symbol={s}"),
            None => String::new(),
        };
        client.public_get(SPOT_BASE_URL, "/ticker/24hr", &query)
    }
}

// ============================================================================
// Tool 5: PlaceOrder — POST /order (signed)
// ============================================================================

impl DynAomiTool for PlaceOrder {
    type App = BinanceApp;
    type Args = PlaceOrderArgs;
    const NAME: &'static str = "binance_place_order";
    const DESCRIPTION: &'static str = "Place a new spot order on Binance. Supports LIMIT, MARKET, STOP_LOSS_LIMIT, and TAKE_PROFIT_LIMIT order types. Requires API credentials.";

    fn run(_app: &BinanceApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BinanceClient::new()?;
        let (api_key, secret_key) =
            resolve_binance_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        let mut query = format!(
            "symbol={}&side={}&type={}",
            args.symbol, args.side, args.order_type
        );
        if let Some(ref tif) = args.time_in_force {
            query.push_str(&format!("&timeInForce={tif}"));
        }
        if let Some(ref qty) = args.quantity {
            query.push_str(&format!("&quantity={qty}"));
        }
        if let Some(ref price) = args.price {
            query.push_str(&format!("&price={price}"));
        }
        client.signed_post(SPOT_BASE_URL, "/order", &api_key, &secret_key, &query)
    }
}

// ============================================================================
// Tool 6: CancelOrder — DELETE /order (signed)
// ============================================================================

impl DynAomiTool for CancelOrder {
    type App = BinanceApp;
    type Args = CancelOrderArgs;
    const NAME: &'static str = "binance_cancel_order";
    const DESCRIPTION: &'static str = "Cancel an active spot order on Binance. Provide either order_id or orig_client_order_id. Requires API credentials.";

    fn run(_app: &BinanceApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BinanceClient::new()?;
        let (api_key, secret_key) =
            resolve_binance_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        let mut query = format!("symbol={}", args.symbol);
        if let Some(oid) = args.order_id {
            query.push_str(&format!("&orderId={oid}"));
        }
        if let Some(ref cid) = args.orig_client_order_id {
            query.push_str(&format!("&origClientOrderId={cid}"));
        }
        client.signed_delete(SPOT_BASE_URL, "/order", &api_key, &secret_key, &query)
    }
}

// ============================================================================
// Tool 7: GetAccount — GET /account (signed)
// ============================================================================

impl DynAomiTool for GetAccount {
    type App = BinanceApp;
    type Args = GetAccountArgs;
    const NAME: &'static str = "binance_get_account";
    const DESCRIPTION: &'static str = "Get account information including balances for all assets on Binance spot. Requires API credentials.";

    fn run(_app: &BinanceApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BinanceClient::new()?;
        let (api_key, secret_key) =
            resolve_binance_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        client.signed_get(SPOT_BASE_URL, "/account", &api_key, &secret_key, "")
    }
}

// ============================================================================
// Tool 8: GetTrades — GET /myTrades (signed)
// ============================================================================

impl DynAomiTool for GetTrades {
    type App = BinanceApp;
    type Args = GetTradesArgs;
    const NAME: &'static str = "binance_get_trades";
    const DESCRIPTION: &'static str =
        "Get trade history for a specific trading pair on Binance spot. Requires API credentials.";

    fn run(_app: &BinanceApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = BinanceClient::new()?;
        let (api_key, secret_key) =
            resolve_binance_credentials(args.api_key.as_deref(), args.secret_key.as_deref())?;
        let mut query = format!("symbol={}", args.symbol);
        if let Some(from_id) = args.from_id {
            query.push_str(&format!("&fromId={from_id}"));
        }
        if let Some(start) = args.start_time {
            query.push_str(&format!("&startTime={start}"));
        }
        if let Some(end) = args.end_time {
            query.push_str(&format!("&endTime={end}"));
        }
        if let Some(limit) = args.limit {
            query.push_str(&format!("&limit={limit}"));
        }
        client.signed_get(SPOT_BASE_URL, "/myTrades", &api_key, &secret_key, &query)
    }
}
