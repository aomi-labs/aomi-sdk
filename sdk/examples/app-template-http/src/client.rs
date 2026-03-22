use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct HttpJsonExampleApp;

pub(crate) use crate::tool::*;

pub(crate) const API_BASE: &str = "https://api.coingecko.com/api/v3";

#[derive(Clone)]
pub(crate) struct CoinGeckoClient {
    pub(crate) http: reqwest::blocking::Client,
}

impl CoinGeckoClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self { http })
    }

    pub(crate) fn get_json(
        &self,
        path: &str,
        query: &[(&str, &str)],
    ) -> Result<Value, String> {
        let url = format!("{API_BASE}{path}");
        let response = self
            .http
            .get(&url)
            .query(query)
            .send()
            .map_err(|e| format!("CoinGecko request failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("CoinGecko error {status}: {body}"));
        }

        serde_json::from_str(&body).map_err(|e| format!("CoinGecko decode failed: {e}"))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchCoinsArgs {
    /// Free-form search query such as `bitcoin`, `eth`, or `dogecoin`
    pub(crate) query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCoinPriceArgs {
    /// CoinGecko coin id such as `bitcoin` or `ethereum`
    pub(crate) coin_id: String,
}
