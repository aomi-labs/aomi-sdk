use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

impl DynAomiTool for GetGmxPrices {
    type App = GmxApp;
    type Args = GetGmxPricesArgs;
    const NAME: &'static str = "get_gmx_prices";
    const DESCRIPTION: &'static str = "Get current token prices from GMX oracle feeds. Returns min/max prices and token symbols for all listed tokens.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let client = GmxClient::new(args.chain.as_deref())?;
        let data = client.get_prices()?;
        Ok(GmxClient::with_source(json!({ "tickers": data }), chain))
    }
}

impl DynAomiTool for GetGmxSignedPrices {
    type App = GmxApp;
    type Args = GetGmxSignedPricesArgs;
    const NAME: &'static str = "get_gmx_signed_prices";
    const DESCRIPTION: &'static str = "Get latest oracle-signed prices from GMX keepers. These are the prices used for on-chain order execution and settlement.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let client = GmxClient::new(args.chain.as_deref())?;
        let data = client.get_signed_prices()?;
        Ok(GmxClient::with_source(data, chain))
    }
}

impl DynAomiTool for GetGmxMarkets {
    type App = GmxApp;
    type Args = GetGmxMarketsArgs;
    const NAME: &'static str = "get_gmx_markets";
    const DESCRIPTION: &'static str = "Get all GM markets on GMX v2 including market addresses, long/short tokens, funding rates, and open interest.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let client = GmxClient::new(args.chain.as_deref())?;
        let data = client.get_markets()?;
        Ok(GmxClient::with_source(json!({ "markets": data }), chain))
    }
}

impl DynAomiTool for GetGmxPositions {
    type App = GmxApp;
    type Args = GetGmxPositionsArgs;
    const NAME: &'static str = "get_gmx_positions";
    const DESCRIPTION: &'static str = "Get open leveraged positions for a specific account on GMX v2. Requires an Ethereum address.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let client = GmxClient::new(args.chain.as_deref())?;
        let data = client.get_positions(&args.account)?;
        Ok(GmxClient::with_source(
            json!({
                "account": args.account,
                "positions": data,
            }),
            chain,
        ))
    }
}

impl DynAomiTool for GetGmxOrders {
    type App = GmxApp;
    type Args = GetGmxOrdersArgs;
    const NAME: &'static str = "get_gmx_orders";
    const DESCRIPTION: &'static str = "Get pending orders (limit, trigger, stop-loss, take-profit) for a specific account on GMX v2. Requires an Ethereum address.";

    fn run(_app: &GmxApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let chain = resolve_chain_label(args.chain.as_deref());
        let client = GmxClient::new(args.chain.as_deref())?;
        let data = client.get_orders(&args.account)?;
        Ok(GmxClient::with_source(
            json!({
                "account": args.account,
                "orders": data,
            }),
            chain,
        ))
    }
}
