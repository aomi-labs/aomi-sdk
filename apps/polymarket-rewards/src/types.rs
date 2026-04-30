use serde::Serialize;
use serde_json::Value;

use crate::client::{QuoteExecutionMode, QuoteOrderTemplate};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BuildQuotePlanSubmitTemplate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) confirmation: Option<String>,
    pub(crate) execution_mode: QuoteExecutionMode,
    pub(crate) condition_id: String,
    pub(crate) yes_bid_order: QuoteOrderTemplate,
    pub(crate) no_bid_order: QuoteOrderTemplate,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WalletEip712Request {
    pub(crate) typed_data: Value,
    pub(crate) description: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SessionPendingTransaction {
    pub(crate) id: String,
    pub(crate) kind: &'static str,
    #[serde(rename = "chainId")]
    pub(crate) chain_id: u64,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) value: String,
    pub(crate) data: String,
    pub(crate) gas: String,
    pub(crate) description: String,
    #[serde(rename = "typedData")]
    pub(crate) typed_data: Value,
    #[serde(rename = "groupId")]
    pub(crate) group_id: String,
    #[serde(rename = "createdAt")]
    pub(crate) created_at: i64,
    pub(crate) state: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SignedQuoteOrderHttpBody {
    pub(crate) owner: String,
    #[serde(rename = "orderType")]
    pub(crate) order_type: Value,
    pub(crate) order: SignedQuoteOrderHttpPayload,
    #[serde(rename = "postOnly", skip_serializing_if = "Option::is_none")]
    pub(crate) post_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SignedQuoteOrderHttpPayload {
    pub(crate) salt: u64,
    pub(crate) maker: String,
    pub(crate) signer: String,
    pub(crate) taker: String,
    #[serde(rename = "tokenId")]
    pub(crate) token_id: String,
    #[serde(rename = "makerAmount")]
    pub(crate) maker_amount: String,
    #[serde(rename = "takerAmount")]
    pub(crate) taker_amount: String,
    pub(crate) expiration: String,
    pub(crate) nonce: String,
    #[serde(rename = "feeRateBps")]
    pub(crate) fee_rate_bps: String,
    pub(crate) side: Value,
    #[serde(rename = "signatureType")]
    pub(crate) signature_type: u8,
    pub(crate) signature: String,
}
