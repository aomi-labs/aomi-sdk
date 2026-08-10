use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use std::time::Duration;

const BASE_URL: &str = "https://api.circle.com";

#[derive(Clone)]
pub(crate) struct StableFxClient {
    http: reqwest::Client,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CurrencyAmount {
    pub currency: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuoteRequest {
    pub from: CurrencyAmount,
    pub to: CurrencyAmount,
    pub tenor: String,
    #[serde(rename = "type")]
    pub quote_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recipient_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct QuoteResponse {
    pub id: String,
    pub from: Value,
    pub to: Value,
    pub rate: Value,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub fee: Value,
    #[serde(default)]
    pub collateral: Value,
    #[serde(default)]
    pub typed_data: Option<Value>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CreateTradeRequest {
    pub idempotency_key: String,
    pub quote_id: String,
    pub address: String,
    pub message: Value,
    pub signature: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TradeResponse {
    pub id: String,
    #[serde(default)]
    pub contract_trade_id: Option<String>,
    pub status: String,
    #[serde(default)]
    pub rate: Value,
    #[serde(default)]
    pub from: Value,
    #[serde(default)]
    pub to: Value,
    #[serde(default)]
    pub quote_id: Option<String>,
    #[serde(default)]
    pub settlement_transaction_hash: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FundingPresignRequest {
    pub contract_trade_ids: Vec<String>,
    #[serde(rename = "type")]
    pub trader_type: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FundingPresignResponse {
    pub deliverables: Vec<Value>,
    pub receivables: Vec<Value>,
    pub typed_data: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FundRequest {
    #[serde(rename = "type")]
    pub trader_type: String,
    pub signature: String,
    pub permit2: Value,
}

impl StableFxClient {
    pub(crate) fn from_env() -> Result<Self, String> {
        let api_key = std::env::var("STABLEFX_API_KEY")
            .map_err(|_| "[stablefx] STABLEFX_API_KEY is not configured".to_string())?;
        if api_key.trim().is_empty() {
            return Err("[stablefx] STABLEFX_API_KEY is empty".to_string());
        }

        let mut headers = HeaderMap::new();
        let mut authorization = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| "[stablefx] STABLEFX_API_KEY is not a valid header value".to_string())?;
        authorization.set_sensitive(true);
        headers.insert(AUTHORIZATION, authorization);
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("[stablefx] failed to build HTTP client: {e}"))?;
        Ok(Self { http })
    }

    pub(crate) async fn quote(&self, request: &QuoteRequest) -> Result<QuoteResponse, String> {
        self.json(
            self.http
                .post(format!("{BASE_URL}/v1/exchange/stablefx/quotes"))
                .json(request),
            "create quote",
        )
        .await
    }

    pub(crate) async fn create_trade(
        &self,
        request: &CreateTradeRequest,
    ) -> Result<TradeResponse, String> {
        self.json(
            self.http
                .post(format!("{BASE_URL}/v1/exchange/stablefx/trades"))
                .json(request),
            "create trade",
        )
        .await
    }

    pub(crate) async fn trade(&self, trade_id: &str) -> Result<TradeResponse, String> {
        self.json(
            self.http
                .get(format!("{BASE_URL}/v1/exchange/stablefx/trades/{trade_id}")),
            "get trade",
        )
        .await
    }

    pub(crate) async fn funding_presign(
        &self,
        request: &FundingPresignRequest,
    ) -> Result<FundingPresignResponse, String> {
        self.json(
            self.http
                .post(format!(
                    "{BASE_URL}/v1/exchange/stablefx/signatures/funding/presign"
                ))
                .json(request),
            "generate funding signature data",
        )
        .await
    }

    pub(crate) async fn fund(&self, request: &FundRequest) -> Result<(), String> {
        let response = self
            .http
            .post(format!("{BASE_URL}/v1/exchange/stablefx/fund"))
            .json(request)
            .send()
            .await
            .map_err(|e| format!("[stablefx] fund trade request failed: {e}"))?;
        Self::expect_success(response, "fund trade").await?;
        Ok(())
    }

    async fn json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        operation: &str,
    ) -> Result<T, String> {
        let response = request
            .send()
            .await
            .map_err(|e| format!("[stablefx] {operation} request failed: {e}"))?;
        let body = Self::expect_success(response, operation).await?;
        let value: Value = serde_json::from_slice(&body)
            .map_err(|e| format!("[stablefx] {operation} returned invalid JSON: {e}"))?;
        let payload = value.get("data").cloned().unwrap_or(value);
        serde_json::from_value(payload)
            .map_err(|e| format!("[stablefx] {operation} response had an unexpected shape: {e}"))
    }

    async fn expect_success(
        response: reqwest::Response,
        operation: &str,
    ) -> Result<Vec<u8>, String> {
        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|e| format!("[stablefx] {operation} response read failed: {e}"))?;
        if status.is_success() {
            return Ok(body.to_vec());
        }
        let detail = String::from_utf8_lossy(&body);
        let detail: String = detail.chars().take(800).collect();
        Err(format!(
            "[stablefx] {operation} failed with HTTP {status}: {detail}"
        ))
    }
}
