use crate::client::{
    PolymarketClient, build_polymarket_order_plan_from_market, submit_direct_order_plan,
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
    let order_plan = build_polymarket_order_plan_from_market(
        &market,
        &request.market_id_or_slug,
        &request.outcome,
        request.side.as_deref(),
        request.size_usd,
        request.shares,
        request.limit_price,
        request.order_type.as_deref(),
        request.post_only,
        request.signature_type.as_deref(),
        request.funder.as_deref(),
        "DIRECT_SDK",
        None,
    )?;

    submit_direct_order_plan(&order_plan, request.private_key.as_deref())
}
