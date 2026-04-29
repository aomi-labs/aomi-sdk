use crate::client::*;
use crate::types::{CancelOrderRequest, PlaceOrderRequest, SetLeverageRequest};
use aomi_sdk::*;
use serde::Serialize;
use serde_json::Value;

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[okx] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("okx".to_string()));
            Value::Object(map)
        }
        other => serde_json::json!({ "source": "okx", "data": other }),
    })
}

fn resolve_okx_credentials(
    api_key: Option<&str>,
    secret_key: Option<&str>,
    passphrase: Option<&str>,
) -> Result<(String, String, String), String> {
    let api_key = resolve_secret_value(
        api_key,
        "OKX_API_KEY",
        "[okx] missing api_key argument and OKX_API_KEY environment variable",
    )?;
    let secret_key = resolve_secret_value(
        secret_key,
        "OKX_SECRET_KEY",
        "[okx] missing secret_key argument and OKX_SECRET_KEY environment variable",
    )?;
    let passphrase = resolve_secret_value(
        passphrase,
        "OKX_PASSPHRASE",
        "[okx] missing passphrase argument and OKX_PASSPHRASE environment variable",
    )?;
    Ok((api_key, secret_key, passphrase))
}

// ============================================================================
// Tool 1: GetTickers — GET /market/tickers
// ============================================================================

impl DynAomiTool for GetTickers {
    type App = OkxApp;
    type Args = GetTickersArgs;
    const NAME: &'static str = "okx_get_tickers";
    const DESCRIPTION: &'static str = "Get tickers for all instruments of a given type (SPOT, SWAP, FUTURES, OPTION). Returns price, volume, and 24h change data.";

    fn run(_app: &OkxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OkxClient::new()?;
        let path = format!("/market/tickers?instType={}", args.inst_type);
        ok(client.public_get(&path)?)
    }
}

// ============================================================================
// Tool 2: GetOrderBook — GET /market/books
// ============================================================================

impl DynAomiTool for GetOrderBook {
    type App = OkxApp;
    type Args = GetOrderBookArgs;
    const NAME: &'static str = "okx_get_order_book";
    const DESCRIPTION: &'static str =
        "Get order book (bids and asks) for an instrument. Returns price levels and quantities.";

    fn run(_app: &OkxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OkxClient::new()?;
        let mut path = format!("/market/books?instId={}", args.inst_id);
        if let Some(ref sz) = args.sz {
            path.push_str(&format!("&sz={sz}"));
        }
        ok(client.public_get(&path)?)
    }
}

// ============================================================================
// Tool 3: GetCandles — GET /market/candles
// ============================================================================

impl DynAomiTool for GetCandles {
    type App = OkxApp;
    type Args = GetCandlesArgs;
    const NAME: &'static str = "okx_get_candles";
    const DESCRIPTION: &'static str = "Get candlestick (OHLCV) data for an instrument. Supports various bar sizes: 1m, 5m, 15m, 30m, 1H, 4H, 1D, 1W, 1M.";

    fn run(_app: &OkxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OkxClient::new()?;
        let mut path = format!("/market/candles?instId={}", args.inst_id);
        if let Some(ref bar) = args.bar {
            path.push_str(&format!("&bar={bar}"));
        }
        if let Some(ref after) = args.after {
            path.push_str(&format!("&after={after}"));
        }
        if let Some(ref before) = args.before {
            path.push_str(&format!("&before={before}"));
        }
        if let Some(ref limit) = args.limit {
            path.push_str(&format!("&limit={limit}"));
        }
        ok(client.public_get(&path)?)
    }
}

// ============================================================================
// Tool 4: PlaceOrder — POST /trade/order
// ============================================================================

impl DynAomiTool for PlaceOrder {
    type App = OkxApp;
    type Args = PlaceOrderArgs;
    const NAME: &'static str = "okx_place_order";
    const DESCRIPTION: &'static str = "Place a new order. Requires API credentials. Use tdMode 'cash' for spot, 'cross' or 'isolated' for derivatives.";

    fn run(_app: &OkxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OkxClient::new()?;
        let (api_key, secret_key, passphrase) = resolve_okx_credentials(
            args.api_key.as_deref(),
            args.secret_key.as_deref(),
            args.passphrase.as_deref(),
        )?;
        let body = PlaceOrderRequest {
            inst_id: &args.inst_id,
            td_mode: &args.td_mode,
            side: &args.side,
            ord_type: &args.ord_type,
            sz: &args.sz,
            px: args.px.as_deref(),
        };
        let path = "/trade/order";
        ok(client.auth_post(path, &body, &api_key, &secret_key, &passphrase)?)
    }
}

// ============================================================================
// Tool 5: CancelOrder — POST /trade/cancel-order
// ============================================================================

impl DynAomiTool for CancelOrder {
    type App = OkxApp;
    type Args = CancelOrderArgs;
    const NAME: &'static str = "okx_cancel_order";
    const DESCRIPTION: &'static str =
        "Cancel an existing order by order ID. Requires API credentials.";

    fn run(_app: &OkxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OkxClient::new()?;
        let (api_key, secret_key, passphrase) = resolve_okx_credentials(
            args.api_key.as_deref(),
            args.secret_key.as_deref(),
            args.passphrase.as_deref(),
        )?;
        let body = CancelOrderRequest {
            inst_id: &args.inst_id,
            ord_id: &args.ord_id,
        };
        let path = "/trade/cancel-order";
        ok(client.auth_post(path, &body, &api_key, &secret_key, &passphrase)?)
    }
}

// ============================================================================
// Tool 6: GetBalance — GET /account/balance
// ============================================================================

impl DynAomiTool for GetBalance {
    type App = OkxApp;
    type Args = GetBalanceArgs;
    const NAME: &'static str = "okx_get_balance";
    const DESCRIPTION: &'static str = "Get account balance for the unified account. Optionally filter by currency. Requires API credentials.";

    fn run(_app: &OkxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OkxClient::new()?;
        let (api_key, secret_key, passphrase) = resolve_okx_credentials(
            args.api_key.as_deref(),
            args.secret_key.as_deref(),
            args.passphrase.as_deref(),
        )?;
        let mut path = "/account/balance".to_string();
        if let Some(ref ccy) = args.ccy {
            path.push_str(&format!("?ccy={ccy}"));
        }
        ok(client.auth_get(&path, &api_key, &secret_key, &passphrase)?)
    }
}

// ============================================================================
// Tool 7: GetPositions — GET /account/positions
// ============================================================================

impl DynAomiTool for GetPositions {
    type App = OkxApp;
    type Args = GetPositionsArgs;
    const NAME: &'static str = "okx_get_positions";
    const DESCRIPTION: &'static str = "Get current positions in the unified account. Optionally filter by instrument type and/or instrument ID. Requires API credentials.";

    fn run(_app: &OkxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OkxClient::new()?;
        let (api_key, secret_key, passphrase) = resolve_okx_credentials(
            args.api_key.as_deref(),
            args.secret_key.as_deref(),
            args.passphrase.as_deref(),
        )?;
        let mut path = "/account/positions".to_string();
        let mut params = Vec::new();
        if let Some(ref inst_type) = args.inst_type {
            params.push(format!("instType={inst_type}"));
        }
        if let Some(ref inst_id) = args.inst_id {
            params.push(format!("instId={inst_id}"));
        }
        if !params.is_empty() {
            path.push('?');
            path.push_str(&params.join("&"));
        }
        ok(client.auth_get(&path, &api_key, &secret_key, &passphrase)?)
    }
}

// ============================================================================
// Tool 8: SetLeverage — POST /account/set-leverage
// ============================================================================

impl DynAomiTool for SetLeverage {
    type App = OkxApp;
    type Args = SetLeverageArgs;
    const NAME: &'static str = "okx_set_leverage";
    const DESCRIPTION: &'static str = "Set leverage for an instrument. Requires API credentials. Margin mode must be 'cross' or 'isolated'.";

    fn run(_app: &OkxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = OkxClient::new()?;
        let (api_key, secret_key, passphrase) = resolve_okx_credentials(
            args.api_key.as_deref(),
            args.secret_key.as_deref(),
            args.passphrase.as_deref(),
        )?;
        let body = SetLeverageRequest {
            inst_id: &args.inst_id,
            lever: &args.lever,
            mgn_mode: &args.mgn_mode,
        };
        let path = "/account/set-leverage";
        ok(client.auth_post(path, &body, &api_key, &secret_key, &passphrase)?)
    }
}
