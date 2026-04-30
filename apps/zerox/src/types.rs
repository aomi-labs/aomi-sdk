use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SwapQuoteQuery<'a> {
    pub(crate) chain_id: u64,
    pub(crate) sell_token: &'a str,
    pub(crate) buy_token: &'a str,
    pub(crate) sell_amount: &'a str,
    pub(crate) slippage_percentage: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) taker: Option<&'a str>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourcesQuery {
    pub(crate) chain_id: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GaslessStatusQuery {
    pub(crate) chain_id: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GaslessSubmitRequest<'a> {
    pub(crate) chain_id: u64,
    pub(crate) trade: &'a Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) approval: Option<&'a Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZeroxTransactionPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) gas: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) gas_price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_fee_per_gas: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) max_priority_fee_per_gas: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZeroxSwapQuotePayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) chain_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) buy_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sell_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) min_buy_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) gross_buy_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) estimated_price_impact: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) price: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) allowance_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) liquidity_available: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) transaction: Option<ZeroxTransactionPayload>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fees: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) issues: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) route: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) token_metadata: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) approval: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) trade: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) trade_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZeroxChainPayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) chain_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) chain_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ZeroxLiquiditySourcePayload {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) proportion: Option<String>,
}
