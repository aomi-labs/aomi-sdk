use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaceBetRequest<'a> {
    pub(crate) contract_id: &'a str,
    pub(crate) amount: f64,
    pub(crate) outcome: &'a str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateMarketRequest<'a> {
    pub(crate) outcome_type: &'a str,
    pub(crate) question: &'a str,
    pub(crate) initial_prob: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) close_time: Option<u64>,
}
