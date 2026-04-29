use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WalletEip712Request {
    pub(crate) typed_data: Value,
    pub(crate) description: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniQuoteRequest {
    #[serde(rename = "tradeType")]
    pub(crate) trade_type: &'static str,
    #[serde(rename = "fromChainId")]
    pub(crate) from_chain_id: u64,
    #[serde(rename = "toChainId")]
    pub(crate) to_chain_id: u64,
    #[serde(rename = "fromToken")]
    pub(crate) from_token: String,
    #[serde(rename = "toToken")]
    pub(crate) to_token: String,
    pub(crate) amount: String,
    #[serde(rename = "fromAddress")]
    pub(crate) from_address: String,
    #[serde(rename = "recipient", skip_serializing_if = "Option::is_none")]
    pub(crate) recipient: Option<String>,
    #[serde(rename = "slippageInBps", skip_serializing_if = "Option::is_none")]
    pub(crate) slippage_in_bps: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniDepositBuildCanonicalRequest {
    pub(crate) from: String,
    #[serde(rename = "quoteId")]
    pub(crate) quote_id: String,
    #[serde(rename = "routeId")]
    pub(crate) route_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniDepositBuildFromAddressRequest {
    #[serde(rename = "fromAddress")]
    pub(crate) from_address: String,
    #[serde(rename = "quoteId")]
    pub(crate) quote_id: String,
    #[serde(rename = "routeId")]
    pub(crate) route_id: String,
    #[serde(rename = "depositMethod")]
    pub(crate) deposit_method: &'static str,
    #[serde(rename = "slippageInBps", skip_serializing_if = "Option::is_none")]
    pub(crate) slippage_in_bps: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniDepositBuildUserAddressRequest {
    #[serde(rename = "userAddress")]
    pub(crate) user_address: String,
    #[serde(rename = "quoteId")]
    pub(crate) quote_id: String,
    #[serde(rename = "routeId")]
    pub(crate) route_id: String,
    #[serde(rename = "depositMethod")]
    pub(crate) deposit_method: &'static str,
    #[serde(rename = "slippageInBps", skip_serializing_if = "Option::is_none")]
    pub(crate) slippage_in_bps: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniDepositBuildLegacyRequest {
    #[serde(rename = "quoteId")]
    pub(crate) quote_id: String,
    pub(crate) user: String,
    #[serde(rename = "allowanceTarget", skip_serializing_if = "Option::is_none")]
    pub(crate) allowance_target: Option<String>,
    #[serde(rename = "slippageInBps", skip_serializing_if = "Option::is_none")]
    pub(crate) slippage_in_bps: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniStageTxRequest {
    pub(crate) to: Value,
    pub(crate) value: Value,
    #[serde(rename = "gas_limit")]
    pub(crate) gas_limit: Value,
    pub(crate) description: String,
    pub(crate) data: KhalaniStageTxData,
    pub(crate) kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniStageTxData {
    pub(crate) raw: Value,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniSignedEip712SubmitRequest {
    #[serde(rename = "quoteId")]
    pub(crate) quote_id: String,
    #[serde(rename = "routeId")]
    pub(crate) route_id: String,
    pub(crate) signature: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniSignedTransactionSubmitRequest {
    #[serde(rename = "quoteId")]
    pub(crate) quote_id: String,
    #[serde(rename = "routeId")]
    pub(crate) route_id: String,
    #[serde(rename = "txHash")]
    pub(crate) tx_hash: String,
    #[serde(rename = "transactionHash")]
    pub(crate) transaction_hash: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniLegacySubmitRequest<T> {
    #[serde(rename = "quoteId")]
    pub(crate) quote_id: String,
    #[serde(rename = "submittedData")]
    pub(crate) submitted_data: T,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniSignedEip712SubmittedData {
    #[serde(rename = "type")]
    pub(crate) submit_type: &'static str,
    pub(crate) signature: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct KhalaniSignedTransactionSubmittedData {
    #[serde(rename = "type")]
    pub(crate) submit_type: &'static str,
    #[serde(rename = "transactionHash")]
    pub(crate) transaction_hash: String,
}
