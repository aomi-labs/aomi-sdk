use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcrossRoute {
    pub(crate) origin_chain_id: u64,
    pub(crate) origin_token: String,
    pub(crate) destination_chain_id: u64,
    pub(crate) destination_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) origin_token_symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) destination_token_symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) is_native: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcrossFeeComponent {
    pub(crate) pct: String,
    pub(crate) total: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcrossLimits {
    pub(crate) min_deposit: String,
    pub(crate) max_deposit: String,
    pub(crate) max_deposit_instant: String,
    pub(crate) max_deposit_short_delay: String,
    pub(crate) recommended_deposit_instant: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcrossTokenRef {
    pub(crate) address: String,
    pub(crate) symbol: String,
    pub(crate) decimals: u64,
    pub(crate) chain_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcrossSuggestedFees {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) estimated_fill_time_sec: Option<u64>,
    pub(crate) total_relay_fee: AcrossFeeComponent,
    pub(crate) relayer_capital_fee: AcrossFeeComponent,
    pub(crate) relayer_gas_fee: AcrossFeeComponent,
    pub(crate) lp_fee: AcrossFeeComponent,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) limits: Option<AcrossLimits>,
    pub(crate) output_amount: String,
    pub(crate) input_token: AcrossTokenRef,
    pub(crate) output_token: AcrossTokenRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AcrossDepositStatus {
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AcrossTokenPrice {
    pub(crate) price: f64,
}
