use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateWalletRequest {
    #[serde(rename = "type")]
    pub(crate) wallet_type: String,
    pub(crate) user_identifier: String,
    pub(crate) user_identifier_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scheme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) cosmos_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SignRawRequest {
    pub(crate) data: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct WalletLookupResult {
    pub(crate) wallet_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}
