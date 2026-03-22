use aomi_sdk::*;
use serde_json::{Value, json};
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

pub(crate) use crate::tool::*;

pub(crate) const DEFAULT_PARA_API: &str = "https://api.beta.getpara.com";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 1_000;

static PARA_CLIENT: OnceLock<Result<ParaClient, String>> = OnceLock::new();

pub(crate) fn para_client() -> Result<&'static ParaClient, String> {
    PARA_CLIENT
        .get_or_init(|| ParaClient::new())
        .as_ref()
        .map_err(|e| e.clone())
}

#[derive(Clone, Default)]
pub(crate) struct ParaApp;

#[derive(Clone)]
pub(crate) struct ParaClient {
    http: reqwest::blocking::Client,
    api_endpoint: String,
}

impl ParaClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[para] failed to build HTTP client: {e}"))?;

        Ok(Self {
            http,
            api_endpoint: std::env::var("PARA_REST_BASE_URL")
                .unwrap_or_else(|_| DEFAULT_PARA_API.to_string()),
        })
    }

    fn send_with_retry<F>(&self, api_key: &str, build: F, op: &str) -> Result<Value, String>
    where
        F: Fn(&reqwest::blocking::Client, &str, &str) -> reqwest::blocking::RequestBuilder,
    {
        let mut backoff_ms = INITIAL_BACKOFF_MS;
        let mut attempts = 0u32;

        loop {
            let request_id = Uuid::new_v4().to_string();
            let response = build(&self.http, &self.api_endpoint, api_key)
                .header("X-Request-Id", &request_id)
                .send()
                .map_err(|e| format!("[para] {op} request failed: {e}"))?;

            let status = response.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS && attempts < MAX_RETRIES {
                attempts += 1;
                eprintln!("[para] {op} rate limited (429), retry {attempts}/{MAX_RETRIES} in {backoff_ms}ms");
                std::thread::sleep(Duration::from_millis(backoff_ms));
                backoff_ms *= 2;
                continue;
            }

            let body = response.text().unwrap_or_default();
            if !status.is_success() {
                let hint = match status.as_u16() {
                    401 | 403 => " Hint: Check that the Para API key is valid and IP is allowlisted.",
                    404 => " Hint: The wallet ID was not found.",
                    409 => {
                        " Hint: A wallet with this type + scheme + userIdentifier already exists."
                    }
                    _ => "",
                };
                return Err(format!("[para] {op} failed with HTTP {status}: {body}{hint}"));
            }

            if body.is_empty() {
                return Ok(json!({}));
            }

            return serde_json::from_str(&body)
                .map_err(|e| format!("[para] {op} JSON decode failed: {e}; body: {body}"));
        }
    }

    pub(crate) fn create_wallet(&self, api_key: &str, body: Value) -> Result<Value, String> {
        self.send_with_retry(
            api_key,
            move |http, base_url, key| {
                http.post(format!("{base_url}/v1/wallets"))
                    .header("X-API-Key", key)
                    .header("Content-Type", "application/json")
                    .json(&body)
            },
            "create wallet",
        )
    }

    pub(crate) fn get_wallet(&self, api_key: &str, wallet_id: &str) -> Result<Value, String> {
        let wallet_id = wallet_id.to_string();
        self.send_with_retry(
            api_key,
            move |http, base_url, key| {
                http.get(format!("{base_url}/v1/wallets/{wallet_id}"))
                    .header("X-API-Key", key)
            },
            "get wallet",
        )
    }

    pub(crate) fn sign_raw(
        &self,
        api_key: &str,
        wallet_id: &str,
        data: &str,
    ) -> Result<Value, String> {
        let wallet_id = wallet_id.to_string();
        let payload = json!({ "data": data });
        self.send_with_retry(
            api_key,
            move |http, base_url, key| {
                http.post(format!("{base_url}/v1/wallets/{wallet_id}/sign-raw"))
                    .header("X-API-Key", key)
                    .header("Content-Type", "application/json")
                    .json(&payload)
            },
            "sign raw",
        )
    }
}
