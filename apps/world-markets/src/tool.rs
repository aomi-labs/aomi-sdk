use crate::client::{self, WorldMarketsApp};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::{DynAomiTool, DynToolCallCtx};
use serde::Deserialize;
use serde_json::{Value, json};

// ============================================================================
// world_list_markets
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListArgs {}

pub struct ListWorldMarkets;

impl DynAomiTool for ListWorldMarkets {
    type App = WorldMarketsApp;
    type Args = ListArgs;
    const NAME: &'static str = "world_list_markets";
    const DESCRIPTION: &'static str = "List every open World Market with its implied probability.";

    fn run(_app: &WorldMarketsApp, _args: ListArgs, _ctx: DynToolCallCtx) -> Result<Value, String> {
        Ok(json!({ "markets": client::MARKETS }))
    }
}

// ============================================================================
// world_get_market
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArgs {
    /// Market id from `world_list_markets`.
    pub market_id: u64,
}

pub struct GetWorldMarket;

impl DynAomiTool for GetWorldMarket {
    type App = WorldMarketsApp;
    type Args = GetArgs;
    const NAME: &'static str = "world_get_market";
    const DESCRIPTION: &'static str = "Market detail: question, implied YES/NO prices, chain.";

    fn run(_app: &WorldMarketsApp, args: GetArgs, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let market = client::market(args.market_id)
            .ok_or_else(|| format!("unknown market id {}", args.market_id))?;
        Ok(json!({
            "market": market,
            "yes_price_usdc": client::share_price_usdc(market, true),
            "no_price_usdc": client::share_price_usdc(market, false),
        }))
    }
}

// ============================================================================
// world_preview_trade
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct PreviewArgs {
    pub market_id: u64,
    /// `true` = YES shares, `false` = NO shares.
    pub yes: bool,
    /// USD to spend, whole dollars.
    pub usd_amount: u64,
}

pub struct PreviewWorldTrade;

impl DynAomiTool for PreviewWorldTrade {
    type App = WorldMarketsApp;
    type Args = PreviewArgs;
    const NAME: &'static str = "world_preview_trade";
    const DESCRIPTION: &'static str = "Preview a trade: shares bought, max payout, and the safety-limit verdict. \
         Always call this before world_build_trade.";

    fn run(
        _app: &WorldMarketsApp,
        args: PreviewArgs,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let market = client::market(args.market_id)
            .ok_or_else(|| format!("unknown market id {}", args.market_id))?;
        let price = client::share_price_usdc(market, args.yes);
        let usdc_amount = args.usd_amount * 10u64.pow(client::USDC_DECIMALS);
        let shares = usdc_amount / price.max(1);
        Ok(json!({
            "market_id": market.id,
            "side": if args.yes { "YES" } else { "NO" },
            "spend_usd": args.usd_amount,
            "share_price_usdc": price,
            "shares": shares,
            "max_payout_usd": shares,
        }))
    }
}

// ============================================================================
// world_build_trade
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BuildArgs {
    pub market_id: u64,
    pub yes: bool,
    /// USD to spend, whole dollars.
    pub usd_amount: u64,
}

pub struct BuildWorldTrade;

impl DynAomiTool for BuildWorldTrade {
    type App = WorldMarketsApp;
    type Args = BuildArgs;
    const NAME: &'static str = "world_build_trade";
    const DESCRIPTION: &'static str = "Build router calldata for a previewed trade. Stage the returned transaction with \
         stage_tx exactly as given; the app's guard table vets the staged target, selector, \
         chain, and `usd_amount` against hard_cap / confirm_cap.";

    fn run(_app: &WorldMarketsApp, args: BuildArgs, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let market = client::market(args.market_id)
            .ok_or_else(|| format!("unknown market id {}", args.market_id))?;
        let usdc_amount = args.usd_amount * 10u64.pow(client::USDC_DECIMALS);
        Ok(json!({
            "transaction": {
                "to": client::ROUTER,
                "data": client::buy_calldata(market.id, args.yes, usdc_amount),
                "value": "0",
                "chain_id": market.chain_id,
            },
            "usd_amount": args.usd_amount,
            "next_step": "stage this transaction with stage_tx, then commit_txs after user confirmation",
        }))
    }
}
