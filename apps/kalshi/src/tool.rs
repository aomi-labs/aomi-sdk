use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[kalshi] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("kalshi".to_string()));
            Value::Object(map)
        }
        other => json!({ "source": "kalshi", "data": other }),
    })
}

fn resolve_simmer_api_key(api_key: Option<&str>) -> Result<String, String> {
    resolve_secret_value(
        api_key,
        "SIMMER_API_KEY",
        "[simmer] missing api_key argument and SIMMER_API_KEY environment variable",
    )
}

pub(crate) struct SimmerRegister;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SimmerRegisterArgs {
    /// Agent name (e.g., "aomi-kalshi")
    name: String,
    /// Brief description of what the agent does
    description: Option<String>,
}

impl DynAomiTool for SimmerRegister {
    type App = KalshiApp;
    type Args = SimmerRegisterArgs;
    const NAME: &'static str = "simmer_register";
    const DESCRIPTION: &'static str =
        "Register a new Simmer agent for Kalshi trading. Returns an API key and claim URL.";

    fn run(_app: &KalshiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let result = simmer_register_agent(&args.name, args.description.as_deref())?;
        ok(json!({
            "status": "registered",
            "agent_id": result.get("agent_id"),
            "api_key": result.get("api_key"),
            "claim_code": result.get("claim_code"),
            "claim_url": result.get("claim_url"),
            "starting_balance": result.get("starting_balance"),
            "limits": result.get("limits"),
            "next_steps": [
                "1. Save the api_key securely (use /apikey simmer <key>)",
                "2. Send claim_url to the user",
                "3. Use venue='sim' for sandbox trades before going live",
                "4. After the user claims, import a Kalshi market URL before trading it live"
            ]
        }))
    }
}

pub(crate) struct SimmerStatus;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SimmerStatusArgs {
    /// Simmer API key (sk_...)
    api_key: Option<String>,
}

impl DynAomiTool for SimmerStatus {
    type App = KalshiApp;
    type Args = SimmerStatusArgs;
    const NAME: &'static str = "simmer_status";
    const DESCRIPTION: &'static str = "Get Simmer agent status, balances, claim state, and whether live Kalshi trading is enabled.";

    fn run(_app: &KalshiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_simmer_api_key(args.api_key.as_deref())?;
        let status = SimmerClient::new(&api_key)?.get_agent_status()?;
        ok(json!({
            "agent_id": status.get("agent_id"),
            "name": status.get("name"),
            "status": status.get("status"),
            "sim_balance": status.get("sim_balance"),
            "usd_balance": status.get("balance_usd").or_else(|| status.get("balance")),
            "live_trading_enabled": status.get("real_trading_enabled"),
            "claim_url": status.get("claim_url"),
            "limits": status.get("limits"),
        }))
    }
}

pub(crate) struct SimmerBriefing;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SimmerBriefingArgs {
    /// Simmer API key (sk_...)
    api_key: Option<String>,
    /// ISO timestamp to get changes since (optional, defaults to backend default)
    since: Option<String>,
}

impl DynAomiTool for SimmerBriefing {
    type App = KalshiApp;
    type Args = SimmerBriefingArgs;
    const NAME: &'static str = "simmer_briefing";
    const DESCRIPTION: &'static str = "Get a Simmer briefing covering portfolio, positions, opportunities, risk alerts, and performance.";

    fn run(_app: &KalshiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_simmer_api_key(args.api_key.as_deref())?;
        let briefing = SimmerClient::new(&api_key)?.get_briefing(args.since.as_deref())?;
        ok(json!({
            "portfolio": briefing.get("portfolio"),
            "positions": briefing.get("positions"),
            "opportunities": briefing.get("opportunities"),
            "risk_alerts": briefing.get("risk_alerts"),
            "performance": briefing.get("performance"),
            "checked_at": briefing.get("checked_at"),
        }))
    }
}

pub(crate) struct SearchSimmerMarkets;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SearchSimmerMarketsArgs {
    /// Simmer API key (sk_...)
    api_key: Option<String>,
    /// Optional free-text search query
    query: Option<String>,
    /// Optional minimum external volume filter (default in API docs: 10000)
    min_volume: Option<f64>,
    /// Maximum number of markets to return (default: 20)
    limit: Option<u32>,
}

impl DynAomiTool for SearchSimmerMarkets {
    type App = KalshiApp;
    type Args = SearchSimmerMarketsArgs;
    const NAME: &'static str = "search_simmer_markets";
    const DESCRIPTION: &'static str = "List importable Kalshi markets from Simmer. Returns Kalshi URLs that can be imported into Simmer before trading.";

    fn run(_app: &KalshiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_simmer_api_key(args.api_key.as_deref())?;
        let result = SimmerClient::new(&api_key)?.list_importable_kalshi_markets(
            args.query.as_deref(),
            args.limit,
            args.min_volume,
        )?;
        let markets = result
            .get("markets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        ok(json!({
            "markets_count": markets.len(),
            "venue": "kalshi",
            "markets": markets,
            "next_step_hint": "Call import_kalshi_market with a market's url before requesting context or placing an order."
        }))
    }
}

pub(crate) struct ImportKalshiMarket;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct ImportKalshiMarketArgs {
    /// Simmer API key (sk_...)
    api_key: Option<String>,
    /// Kalshi market URL returned by search_simmer_markets
    kalshi_url: String,
}

impl DynAomiTool for ImportKalshiMarket {
    type App = KalshiApp;
    type Args = ImportKalshiMarketArgs;
    const NAME: &'static str = "import_kalshi_market";
    const DESCRIPTION: &'static str = "Import a Kalshi market into Simmer and return the Simmer market_id UUID required for context and trading calls.";

    fn run(_app: &KalshiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_simmer_api_key(args.api_key.as_deref())?;
        let result = SimmerClient::new(&api_key)?.import_kalshi_market(&args.kalshi_url)?;
        ok(json!({
            "market_id": result.get("market_id"),
            "market": result.get("market"),
            "url": result.get("url").or_else(|| result.get("kalshi_url")),
            "status": result.get("status"),
        }))
    }
}

pub(crate) struct FetchSimmerMarketContext;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct FetchSimmerMarketContextArgs {
    /// Simmer API key (sk_...)
    api_key: Option<String>,
    /// Simmer market ID to analyze before trading
    market_id: String,
}

impl DynAomiTool for FetchSimmerMarketContext {
    type App = KalshiApp;
    type Args = FetchSimmerMarketContextArgs;
    const NAME: &'static str = "fetch_simmer_market_context";
    const DESCRIPTION: &'static str = "Get detailed context for an imported Kalshi market before trading, including warnings, slippage, fees, and resolution criteria.";

    fn run(_app: &KalshiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_simmer_api_key(args.api_key.as_deref())?;
        let context = SimmerClient::new(&api_key)?.get_market_context(&args.market_id)?;
        ok(json!({
            "market": context.get("market"),
            "position": context.get("position"),
            "warnings": context.get("warnings"),
            "slippage_estimate": context.get("slippage_estimate"),
            "time_to_resolution": context.get("time_to_resolution"),
            "resolution_criteria": context.get("resolution_criteria"),
            "fees": {
                "is_paid": context.get("is_paid"),
                "fee_rate_bps": context.get("fee_rate_bps"),
                "note": context.get("fee_note"),
            }
        }))
    }
}

pub(crate) struct SimmerPlaceOrder;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SimmerPlaceOrderArgs {
    /// Simmer API key (sk_...)
    api_key: Option<String>,
    /// Simmer market ID to trade on
    market_id: String,
    /// Outcome to bet on: "yes" or "no"
    side: String,
    /// Amount in USD (for buys). Provide either amount or shares.
    amount: Option<f64>,
    /// Shares quantity (required for sells; optional for buys). Provide either amount or shares.
    shares: Option<f64>,
    /// Trading venue: sim (sandbox) or kalshi (live)
    venue: Option<String>,
    /// Action: "buy" or "sell" (default: buy)
    action: Option<String>,
    /// Validate without executing. Supported for real-money venues like kalshi.
    dry_run: Option<bool>,
    /// Your thesis for this trade -- displayed publicly on Simmer
    reasoning: Option<String>,
}

impl DynAomiTool for SimmerPlaceOrder {
    type App = KalshiApp;
    type Args = SimmerPlaceOrderArgs;
    const NAME: &'static str = "simmer_place_order";
    const DESCRIPTION: &'static str = "Place an order via Simmer for Kalshi or the sim sandbox. Supports dry_run on live venues and shares-based sells.";

    fn run(_app: &KalshiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let venue = args
            .venue
            .as_deref()
            .map(parse_venue)
            .transpose()?
            .unwrap_or_else(|| "sim".to_string());
        let action = args
            .action
            .as_deref()
            .unwrap_or("buy")
            .trim()
            .to_ascii_lowercase();

        if action != "buy" && action != "sell" {
            return Err("action must be buy or sell".to_string());
        }

        match (args.amount, args.shares) {
            (Some(_), Some(_)) => {
                return Err("Provide either amount or shares, not both.".to_string());
            }
            (None, None) => {
                return Err("Provide one of amount or shares.".to_string());
            }
            _ => {}
        }

        if action == "sell" && args.shares.is_none() {
            return Err("Sell orders must use shares, not amount.".to_string());
        }

        let api_key = resolve_simmer_api_key(args.api_key.as_deref())?;
        let request = SimmerTradeRequest {
            market_id: args.market_id.clone(),
            side: args.side.to_lowercase(),
            amount: args.amount,
            shares: args.shares,
            venue: venue.clone(),
            action: action.clone(),
            source: "sdk:aomi".to_string(),
            dry_run: args.dry_run,
            reasoning: args.reasoning.clone(),
        };

        match SimmerClient::new(&api_key)?.trade(&request) {
            Ok(response) => ok(json!({
                "status": "success",
                "trade_id": response.get("trade_id"),
                "market_id": response.get("market_id"),
                "side": response.get("side"),
                "shares": response.get("shares_bought").or_else(|| response.get("shares_sold")),
                "cost": response.get("cost"),
                "average_price": response.get("average_price"),
                "venue": response.get("venue"),
                "reasoning": args.reasoning,
                "dry_run": args.dry_run,
            })),
            Err(e) => ok(json!({
                "status": "error",
                "message": e,
                "order_details": {
                    "market_id": args.market_id,
                    "side": args.side,
                    "amount": args.amount,
                    "shares": args.shares,
                    "venue": venue,
                    "dry_run": args.dry_run,
                }
            })),
        }
    }
}

pub(crate) struct SimmerGetPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SimmerGetPositionsArgs {
    /// Simmer API key (sk_...)
    api_key: Option<String>,
    /// Optional venue filter: sim or kalshi
    venue: Option<String>,
}

impl DynAomiTool for SimmerGetPositions {
    type App = KalshiApp;
    type Args = SimmerGetPositionsArgs;
    const NAME: &'static str = "simmer_get_positions";
    const DESCRIPTION: &'static str = "Get positions for the sim sandbox or Kalshi venue.";

    fn run(_app: &KalshiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let venue = args
            .venue
            .as_deref()
            .map(parse_venue)
            .transpose()?
            .unwrap_or_else(|| "sim".to_string());

        let api_key = resolve_simmer_api_key(args.api_key.as_deref())?;
        let result = SimmerClient::new(&api_key)?.get_positions(Some(venue.as_str()))?;
        let positions = result
            .get("positions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let total_pnl: f64 = positions
            .iter()
            .filter_map(|p| p.get("pnl").and_then(Value::as_f64))
            .sum();

        ok(json!({
            "positions_count": positions.len(),
            "venue": venue,
            "total_pnl": format!("${:.2}", total_pnl),
            "positions": positions,
        }))
    }
}

pub(crate) struct SimmerGetPortfolio;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SimmerGetPortfolioArgs {
    /// Simmer API key (sk_...)
    api_key: Option<String>,
}

impl DynAomiTool for SimmerGetPortfolio {
    type App = KalshiApp;
    type Args = SimmerGetPortfolioArgs;
    const NAME: &'static str = "simmer_get_portfolio";
    const DESCRIPTION: &'static str =
        "Get portfolio summary from Simmer for the linked Kalshi account.";

    fn run(_app: &KalshiApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = resolve_simmer_api_key(args.api_key.as_deref())?;
        let portfolio = SimmerClient::new(&api_key)?.get_portfolio()?;
        ok(json!({
            "balance": portfolio.get("balance"),
            "currency": portfolio.get("currency"),
            "positions_value": portfolio.get("positions_value"),
            "total_value": portfolio.get("total_value"),
            "realized_pnl": portfolio.get("realized_pnl"),
            "unrealized_pnl": portfolio.get("unrealized_pnl"),
        }))
    }
}
