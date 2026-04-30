use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WalletEip712Request {
    pub(crate) typed_data: Value,
    pub(crate) description: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WalletSignatureStepMetadata {
    pub(crate) wallet_tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) signing_primitive: Option<String>,
    pub(crate) callback_field: String,
    pub(crate) requires_user_confirmation_before_call: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Eip712TypeField {
    pub(crate) name: &'static str,
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClobAuthDomain {
    pub(crate) name: &'static str,
    pub(crate) version: &'static str,
    #[serde(rename = "chainId")]
    pub(crate) chain_id: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ExchangeDomain {
    pub(crate) name: &'static str,
    pub(crate) version: &'static str,
    #[serde(rename = "chainId")]
    pub(crate) chain_id: u64,
    #[serde(rename = "verifyingContract")]
    pub(crate) verifying_contract: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClobAuthMessage {
    pub(crate) address: String,
    pub(crate) timestamp: String,
    pub(crate) nonce: String,
    pub(crate) message: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PreparedOrderMessage {
    pub(crate) salt: String,
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
    pub(crate) side: u8,
    #[serde(rename = "signatureType")]
    pub(crate) signature_type: u8,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClobAuthTypedData {
    pub(crate) types: ClobAuthTypes,
    #[serde(rename = "primaryType")]
    pub(crate) primary_type: &'static str,
    pub(crate) domain: ClobAuthDomain,
    pub(crate) message: ClobAuthMessage,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PreparedOrderTypedData {
    pub(crate) types: PreparedOrderTypes,
    #[serde(rename = "primaryType")]
    pub(crate) primary_type: &'static str,
    pub(crate) domain: ExchangeDomain,
    pub(crate) message: PreparedOrderMessage,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ClobAuthTypes {
    #[serde(rename = "EIP712Domain")]
    pub(crate) eip712_domain: Vec<Eip712TypeField>,
    #[serde(rename = "ClobAuth")]
    pub(crate) clob_auth: Vec<Eip712TypeField>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PreparedOrderTypes {
    #[serde(rename = "EIP712Domain")]
    pub(crate) eip712_domain: Vec<Eip712TypeField>,
    #[serde(rename = "Order")]
    pub(crate) order: Vec<Eip712TypeField>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SignedOrderHttpBody {
    pub(crate) owner: String,
    #[serde(rename = "orderType")]
    pub(crate) order_type: String,
    pub(crate) order: SignedOrderHttpPayload,
    #[serde(rename = "postOnly", skip_serializing_if = "Option::is_none")]
    pub(crate) post_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SignedOrderHttpPayload {
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
    pub(crate) side: String,
    #[serde(rename = "signatureType")]
    pub(crate) signature_type: u8,
    pub(crate) signature: String,
}
