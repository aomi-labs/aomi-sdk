use crate::client::{
    BuildOrderPlanRequest, PolymarketClient, build_polymarket_order_plan_from_market,
    submit_direct_order_plan,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
