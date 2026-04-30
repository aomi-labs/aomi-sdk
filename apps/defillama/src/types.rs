use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ExcludedChartsQuery {
    #[serde(rename = "excludeTotalDataChart")]
    pub(crate) exclude_total_data_chart: bool,
    #[serde(rename = "excludeTotalDataChartBreakdown")]
    pub(crate) exclude_total_data_chart_breakdown: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct FeesOverviewQuery {
    #[serde(rename = "excludeTotalDataChart")]
    pub(crate) exclude_total_data_chart: bool,
    #[serde(rename = "excludeTotalDataChartBreakdown")]
    pub(crate) exclude_total_data_chart_breakdown: bool,
    #[serde(rename = "dataType", skip_serializing_if = "Option::is_none")]
    pub(crate) data_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProtocolFeesQuery {
    #[serde(rename = "dataType", skip_serializing_if = "Option::is_none")]
    pub(crate) data_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct IncludePricesQuery {
    #[serde(rename = "includePrices")]
    pub(crate) include_prices: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct HistoricalTokenPriceQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) start: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) end: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) span: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) period: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct TokenPriceChangeQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) timestamp: Option<u64>,
    #[serde(rename = "lookForward", skip_serializing_if = "Option::is_none")]
    pub(crate) look_forward: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) period: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct StablecoinHistoryQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stablecoin: Option<u64>,
}
