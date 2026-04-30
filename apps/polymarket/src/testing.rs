use crate::client::{
    BuildOrderPlanRequest, GetMarketsParams, Market, PolymarketClient,
    build_polymarket_order_plan_from_market, extract_yes_no_prices, submit_direct_order_plan,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const SMOKE_MARKET_SEARCH_LIMIT: u32 = 200;
const SMOKE_ORDER_USDC: f64 = 1.0;
const SMOKE_MIN_PRICE: f64 = 0.05;
const SMOKE_MAX_PRICE: f64 = 0.95;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveTradeRequest {
    pub market_id_or_slug: String,
    pub outcome: String,
    pub side: Option<String>,
    pub size_usd: Option<f64>,
    pub shares: Option<f64>,
    pub limit_price: Option<f64>,
    pub order_type: Option<String>,
    pub post_only: Option<bool>,
    pub signature_type: Option<String>,
    pub funder: Option<String>,
    pub private_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveTradeSelection {
    pub market_id_or_slug: String,
    pub question: String,
    pub outcome: String,
    pub side: String,
    pub amount_usdc: f64,
    pub yes_price: Option<f64>,
    pub no_price: Option<f64>,
    pub liquidity: Option<f64>,
    pub volume: Option<f64>,
}

#[derive(Debug, Clone)]
struct DiscoveredLiveTrade {
    market: Market,
    selection: LiveTradeSelection,
}

pub fn place_live_order(request: LiveTradeRequest) -> Result<Value, String> {
    let client = PolymarketClient::new()?;
    let market = client.get_market(&request.market_id_or_slug)?;
    let order_plan = build_polymarket_order_plan_from_market(BuildOrderPlanRequest {
        market: &market,
        market_id_or_slug: &request.market_id_or_slug,
        outcome: &request.outcome,
        side: request.side.as_deref(),
        size_usd: request.size_usd,
        shares: request.shares,
        limit_price: request.limit_price,
        order_type: request.order_type.as_deref(),
        post_only: request.post_only,
        signature_type: request.signature_type.as_deref(),
        funder: request.funder.as_deref(),
        execution_mode: "DIRECT_SDK",
        wallet_address: None,
    })?;

    submit_direct_order_plan(&order_plan, request.private_key.as_deref())
}

pub fn place_live_smoke_order(private_key: Option<String>) -> Result<Value, String> {
    let client = PolymarketClient::new()?;
    let discovered = discover_live_trade(&client)?;
    let request = LiveTradeRequest {
        market_id_or_slug: discovered.selection.market_id_or_slug.clone(),
        outcome: discovered.selection.outcome.clone(),
        side: Some(discovered.selection.side.clone()),
        size_usd: Some(discovered.selection.amount_usdc),
        shares: None,
        limit_price: None,
        order_type: None,
        post_only: None,
        signature_type: Some("proxy".to_string()),
        funder: None,
        private_key,
    };

    let order_plan = build_polymarket_order_plan_from_market(BuildOrderPlanRequest {
        market: &discovered.market,
        market_id_or_slug: &request.market_id_or_slug,
        outcome: &request.outcome,
        side: request.side.as_deref(),
        size_usd: request.size_usd,
        shares: request.shares,
        limit_price: request.limit_price,
        order_type: request.order_type.as_deref(),
        post_only: request.post_only,
        signature_type: request.signature_type.as_deref(),
        funder: request.funder.as_deref(),
        execution_mode: "DIRECT_SDK",
        wallet_address: None,
    })?;

    match submit_direct_order_plan(&order_plan, request.private_key.as_deref()) {
        Ok(mut result) => {
            let Some(obj) = result.as_object_mut() else {
                return Err("smoke order result was not an object".to_string());
            };
            obj.insert("reached_submission".to_string(), json!(true));
            obj.insert("smoke_selection".to_string(), json!(discovered.selection));
            Ok(result)
        }
        Err(error) if is_expected_live_submission_rejection(&error) => Ok(json!({
            "submitted": false,
            "reached_submission": true,
            "error": error,
            "smoke_selection": discovered.selection,
        })),
        Err(error) => Err(error),
    }
}

fn discover_live_trade(client: &PolymarketClient) -> Result<DiscoveredLiveTrade, String> {
    let mut markets = client.get_markets(&GetMarketsParams {
        limit: Some(SMOKE_MARKET_SEARCH_LIMIT),
        offset: Some(0),
        active: Some(true),
        closed: Some(false),
        archived: Some(false),
        tag: None,
    })?;

    markets.sort_by(|a, b| {
        b.liquidity_num
            .unwrap_or(0.0)
            .total_cmp(&a.liquidity_num.unwrap_or(0.0))
            .then_with(|| {
                b.volume_num
                    .unwrap_or(0.0)
                    .total_cmp(&a.volume_num.unwrap_or(0.0))
            })
    });

    for market in markets {
        let Some(question) = market.question.clone() else {
            continue;
        };
        let Some(market_id_or_slug) = market.slug.clone().or_else(|| market.id.clone()) else {
            continue;
        };
        let (yes_price, no_price) = extract_yes_no_prices(&market);
        let Some(outcome) = choose_smoke_outcome(yes_price, no_price) else {
            continue;
        };

        return Ok(DiscoveredLiveTrade {
            selection: LiveTradeSelection {
                market_id_or_slug,
                question,
                outcome: outcome.to_string(),
                side: "BUY".to_string(),
                amount_usdc: SMOKE_ORDER_USDC,
                yes_price,
                no_price,
                liquidity: market.liquidity_num,
                volume: market.volume_num,
            },
            market,
        });
    }

    Err("could not find a suitable active Polymarket yes/no market for the smoke test".to_string())
}

fn choose_smoke_outcome(yes_price: Option<f64>, no_price: Option<f64>) -> Option<&'static str> {
    let mut candidates = Vec::new();
    if let Some(price) =
        yes_price.filter(|price| *price >= SMOKE_MIN_PRICE && *price <= SMOKE_MAX_PRICE)
    {
        candidates.push(("YES", price));
    }
    if let Some(price) =
        no_price.filter(|price| *price >= SMOKE_MIN_PRICE && *price <= SMOKE_MAX_PRICE)
    {
        candidates.push(("NO", price));
    }

    candidates
        .into_iter()
        .min_by(|(_, left), (_, right)| ((*left - 0.5).abs()).total_cmp(&((*right - 0.5).abs())))
        .map(|(outcome, _)| outcome)
}

fn is_expected_live_submission_rejection(error: &str) -> bool {
    error.contains("making POST call to /order")
        && (error.contains("not enough balance / allowance")
            || error.contains("balance is not enough")
            || error.contains("allowance"))
}
