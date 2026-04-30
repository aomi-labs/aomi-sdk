use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SimmerRegisterRequest {
    pub(crate) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ImportKalshiMarketRequest {
    pub(crate) kalshi_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SimmerTradeRequest {
    pub(crate) market_id: String,
    pub(crate) side: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) shares: Option<f64>,
    pub(crate) venue: String,
    pub(crate) action: String,
    pub(crate) source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) dry_run: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reasoning: Option<String>,
}
