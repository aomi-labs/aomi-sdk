use crate::client::*;
use aomi_sdk::*;
use serde_json::Value;

impl DynAomiTool for GetMeta {
    type App = HyperliquidApp;
    type Args = GetMetaArgs;
    const NAME: &'static str = "get_meta";
    const DESCRIPTION: &'static str = "Get exchange metadata including the universe of tradeable perpetual assets and their size decimals.";

    fn run(
        _app: &HyperliquidApp,
        _args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let client = HyperliquidClient::new()?;
        client.get_meta()
    }
}

impl DynAomiTool for GetAllMids {
    type App = HyperliquidApp;
    type Args = GetAllMidsArgs;
    const NAME: &'static str = "get_all_mids";
    const DESCRIPTION: &'static str =
        "Get current mid-prices for all listed assets on Hyperliquid.";

    fn run(
        _app: &HyperliquidApp,
        _args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let client = HyperliquidClient::new()?;
        client.get_all_mids()
    }
}

impl DynAomiTool for GetL2Book {
    type App = HyperliquidApp;
    type Args = GetL2BookArgs;
    const NAME: &'static str = "get_l2_book";
    const DESCRIPTION: &'static str =
        "Get L2 order book snapshot (bid and ask levels) for a specific asset.";

    fn run(_app: &HyperliquidApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = HyperliquidClient::new()?;
        client.get_l2_book(&args.coin)
    }
}

impl DynAomiTool for GetClearinghouseState {
    type App = HyperliquidApp;
    type Args = GetClearinghouseStateArgs;
    const NAME: &'static str = "get_clearinghouse_state";
    const DESCRIPTION: &'static str = "Get account state including positions, margin summary, and account value for a user address.";

    fn run(_app: &HyperliquidApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = HyperliquidClient::new()?;
        client.get_clearinghouse_state(&args.user)
    }
}

impl DynAomiTool for GetOpenOrders {
    type App = HyperliquidApp;
    type Args = GetOpenOrdersArgs;
    const NAME: &'static str = "get_open_orders";
    const DESCRIPTION: &'static str = "Get all open (pending) orders for a user address, including coin, side, size, limit price, and order ID.";

    fn run(_app: &HyperliquidApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = HyperliquidClient::new()?;
        client.get_open_orders(&args.user)
    }
}

impl DynAomiTool for GetUserFills {
    type App = HyperliquidApp;
    type Args = GetUserFillsArgs;
    const NAME: &'static str = "get_user_fills";
    const DESCRIPTION: &'static str = "Get trade fill history for a user address, including fees, timestamps, and transaction hashes.";

    fn run(_app: &HyperliquidApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = HyperliquidClient::new()?;
        client.get_user_fills(&args.user)
    }
}

impl DynAomiTool for GetFundingHistory {
    type App = HyperliquidApp;
    type Args = GetFundingHistoryArgs;
    const NAME: &'static str = "get_funding_history";
    const DESCRIPTION: &'static str =
        "Get historical funding rate snapshots for a specific asset over a time range.";

    fn run(_app: &HyperliquidApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = HyperliquidClient::new()?;
        client.get_funding_history(&args.coin, args.start_time, args.end_time)
    }
}

impl DynAomiTool for GetCandleSnapshot {
    type App = HyperliquidApp;
    type Args = GetCandleSnapshotArgs;
    const NAME: &'static str = "get_candle_snapshot";
    const DESCRIPTION: &'static str =
        "Get OHLCV candlestick data for a specific asset at a given interval and time range.";

    fn run(_app: &HyperliquidApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = HyperliquidClient::new()?;
        client.get_candle_snapshot(&args.coin, &args.interval, args.start_time, args.end_time)
    }
}
