use aomi_sdk::schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize)]
pub(crate) struct CreateQuoteRequest<'a> {
    pub(crate) text: &'a str,
    pub(crate) maker_owner_id: &'a str,
    pub(crate) maker_shard: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct FeedEvidence {
    /// Price feed source name
    pub(crate) source: String,
    /// Asset the price is for
    pub(crate) asset: String,
    /// Price from this feed
    pub(crate) price: f64,
    /// Unix timestamp of the price
    pub(crate) timestamp: i64,
    /// Cryptographic signature
    pub(crate) signature: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct FillQuoteRequest<'a> {
    pub(crate) taker_owner_id: &'a str,
    pub(crate) taker_shard: u64,
    pub(crate) size: f64,
    pub(crate) price: f64,
    pub(crate) feed_evidence: &'a [FeedEvidence],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Quote {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) status: String,
    pub(crate) asset: String,
    pub(crate) direction: String,
    pub(crate) size: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) price_limit: Option<f64>,
    pub(crate) currency: String,
    pub(crate) expires_at: i64,
    pub(crate) created_at: i64,
    pub(crate) maker_owner_id: String,
    pub(crate) maker_shard: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) local_law: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) constraints_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FillResponse {
    pub(crate) success: bool,
    pub(crate) fill_id: String,
    pub(crate) quote_id: String,
    pub(crate) message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) receipt: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) proof: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Receipt {
    pub(crate) id: String,
    pub(crate) quote_id: String,
    pub(crate) success: bool,
    pub(crate) status: String,
    pub(crate) taker_owner_id: String,
    pub(crate) taker_shard: u64,
    pub(crate) size: f64,
    pub(crate) price: f64,
    pub(crate) attempted_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error_code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error_message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sdl_hash: Option<String>,
}
