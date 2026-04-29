use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuoteRequest<'a> {
    pub(crate) sell_token: &'a str,
    pub(crate) buy_token: &'a str,
    pub(crate) sell_amount_before_fee: &'a str,
    pub(crate) from: &'a str,
    pub(crate) kind: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) receiver: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) valid_to: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) partially_fillable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signing_scheme: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) slippage_bps: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CancelOrdersRequest<'a> {
    pub(crate) order_uids: &'a [String],
    pub(crate) signature: &'a str,
    pub(crate) signing_scheme: &'a str,
}

fn deserialize_optional_f64ish<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    Ok(value.and_then(|value| {
        value
            .as_f64()
            .or_else(|| value.as_str().and_then(|raw| raw.parse::<f64>().ok()))
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CowQuote {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) quote: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sell_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) buy_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sell_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) buy_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fee_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) valid_to: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) partially_fillable: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CowOrder {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sell_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) buy_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sell_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) buy_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) executed_sell_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) executed_buy_amount: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CowOrderStatus {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CowTrade {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) order_uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sell_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) buy_amount: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fee_amount: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CowNativePrice {
    #[serde(default, deserialize_with = "deserialize_optional_f64ish")]
    pub(crate) price: Option<f64>,
}
