use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct DeltaApp;

pub(crate) use crate::tool::*;

// ============================================================================
// Client
// ============================================================================

pub(crate) const DEFAULT_API_URL: &str = "http://localhost:3335";

#[derive(Clone)]
pub(crate) struct DeltaClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) base_url: String,
}

impl DeltaClient {
    pub(crate) fn new() -> Result<Self, String> {
        let base_url =
            std::env::var("DELTA_RFQ_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string());
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self { http, base_url })
    }

    pub(crate) fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("API error {status}: {text}"));
        }
        resp.json().map_err(|e| format!("decode failed: {e}"))
    }

    pub(crate) fn post<B: Serialize, T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T, String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(body)
            .send()
            .map_err(|e| format!("request failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("API error {status}: {text}"));
        }
        resp.json().map_err(|e| format!("decode failed: {e}"))
    }
}

// ============================================================================
// Data models
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Quote {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) status: String,
    pub(crate) asset: String,
    pub(crate) direction: String,
    pub(crate) size: f64,
    pub(crate) price_limit: Option<f64>,
    pub(crate) currency: String,
    pub(crate) expires_at: i64,
    pub(crate) created_at: i64,
    pub(crate) maker_owner_id: String,
    pub(crate) maker_shard: u64,
    #[serde(default)]
    pub(crate) local_law: Option<Value>,
    #[serde(default)]
    pub(crate) constraints_summary: Option<String>,
    #[serde(default)]
    pub(crate) message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct FillResponse {
    pub(crate) success: bool,
    pub(crate) fill_id: String,
    pub(crate) quote_id: String,
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) error: Option<Value>,
    #[serde(default)]
    pub(crate) receipt: Option<Value>,
    #[serde(default)]
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
    pub(crate) error_code: Option<String>,
    pub(crate) error_message: Option<String>,
    pub(crate) sdl_hash: Option<String>,
}

// ============================================================================
// Tool 1: CreateQuote
// ============================================================================

pub(crate) struct CreateQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CreateQuoteArgs {
    /// Natural language description of the quote (e.g., 'Buy 10 dETH at most 2000 USDD each, expires in 5 minutes')
    pub(crate) text: String,
    /// Maker's owner ID
    pub(crate) maker_owner_id: String,
    /// Maker's shard number
    pub(crate) maker_shard: u64,
}
