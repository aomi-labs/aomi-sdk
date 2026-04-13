use crate::client::{
    GetMarketsParams, GetTradesParams, Market, Trade, build_market_lookup_request,
    classify_market_lookup_target,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) const GAMMA_API_BASE: &str = "https://gamma-api.polymarket.com";
pub(crate) const DATA_API_BASE: &str = "https://data-api.polymarket.com";
pub(crate) const CLOB_API_BASE: &str = "https://clob.polymarket.com";
pub(crate) const CLOB_AUTH_DERIVE_API_KEY_PATH: &str = "/auth/derive-api-key";
pub(crate) const CLOB_AUTH_CREATE_API_KEY_PATH: &str = "/auth/api-key";

pub(crate) const HEADER_POLY_ADDRESS: &str = "POLY_ADDRESS";
pub(crate) const HEADER_POLY_SIGNATURE: &str = "POLY_SIGNATURE";
pub(crate) const HEADER_POLY_TIMESTAMP: &str = "POLY_TIMESTAMP";
pub(crate) const HEADER_POLY_NONCE: &str = "POLY_NONCE";
pub(crate) const HEADER_POLY_API_KEY: &str = "POLY_API_KEY";
pub(crate) const HEADER_POLY_PASSPHRASE: &str = "POLY_PASSPHRASE";

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClobApiCredentials {
    pub(crate) key: String,
    pub(crate) secret: String,
    pub(crate) passphrase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClobL1Auth {
    pub(crate) address: String,
    pub(crate) signature: String,
    pub(crate) timestamp: String,
    pub(crate) nonce: Option<String>,
}

#[derive(Clone)]
pub(crate) struct PolymarketClient {
    pub(crate) http: reqwest::blocking::Client,
}

impl PolymarketClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self { http })
    }

    pub(crate) fn get_markets(&self, params: &GetMarketsParams) -> Result<Vec<Market>, String> {
        let url = format!("{GAMMA_API_BASE}/markets");
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(limit) = params.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(offset) = params.offset {
            query.push(("offset", offset.to_string()));
        }
        if let Some(active) = params.active {
            query.push(("active", active.to_string()));
        }
        if let Some(closed) = params.closed {
            query.push(("closed", closed.to_string()));
        }
        if let Some(archived) = params.archived {
            query.push(("archived", archived.to_string()));
        }
        if let Some(tag) = params.tag.as_ref() {
            query.push(("tag", tag.clone()));
        }

        let response = self
            .http
            .get(&url)
            .query(&query)
            .send()
            .map_err(|e| format!("Gamma API request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("Gamma API error {status}: {text}"));
        }

        let text = response.text().map_err(|e| format!("read body: {e}"))?;
        serde_json::from_str::<Vec<Market>>(&text).map_err(|e| {
            let preview = if text.len() > 500 {
                &text[..500]
            } else {
                &text
            };
            format!("Failed to parse markets: {e}\nPreview: {preview}")
        })
    }

    pub(crate) fn get_market(&self, id_or_slug: &str) -> Result<Market, String> {
        let target = classify_market_lookup_target(id_or_slug);
        let lookup = build_market_lookup_request(&target);
        let url = format!("{}{}", GAMMA_API_BASE, lookup.path);

        let response = self
            .http
            .get(&url)
            .query(&lookup.query)
            .send()
            .map_err(|e| format!("Gamma API request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!(
                "Failed to get market {id_or_slug}: {status} - {text}"
            ));
        }

        let text = response.text().map_err(|e| format!("read body: {e}"))?;
        if matches!(target, crate::client::MarketLookupTarget::ConditionId(_)) {
            let markets = serde_json::from_str::<Vec<Market>>(&text)
                .map_err(|e| format!("parse markets: {e}"))?;
            return markets
                .into_iter()
                .next()
                .ok_or_else(|| format!("No market found for {id_or_slug}"));
        }

        serde_json::from_str::<Market>(&text).map_err(|e| format!("parse market: {e}"))
    }

    pub(crate) fn get_trades(&self, params: &GetTradesParams) -> Result<Vec<Trade>, String> {
        let url = format!("{DATA_API_BASE}/trades");
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(limit) = params.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(offset) = params.offset {
            query.push(("offset", offset.to_string()));
        }
        if let Some(market) = params.market.as_ref() {
            query.push(("market", market.clone()));
        }
        if let Some(user) = params.user.as_ref() {
            query.push(("user", user.clone()));
        }
        if let Some(side) = params.side.as_ref() {
            query.push(("side", side.clone()));
        }

        let response = self
            .http
            .get(&url)
            .query(&query)
            .send()
            .map_err(|e| format!("Data API request failed: {e}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().unwrap_or_default();
            return Err(format!("Data API error {status}: {text}"));
        }

        let text = response.text().map_err(|e| format!("read body: {e}"))?;
        serde_json::from_str::<Vec<Trade>>(&text).map_err(|e| {
            let preview = if text.len() > 500 {
                &text[..500]
            } else {
                &text
            };
            format!("Failed to parse trades: {e}\nPreview: {preview}")
        })
    }

    pub(crate) fn create_or_derive_api_credentials(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        match self.create_api_key(l1_auth) {
            Ok(creds) => Ok(creds),
            Err(create_err) if create_err.contains("request failed with status") => {
                match self.derive_api_key(l1_auth) {
                    Ok(creds) => Ok(creds),
                    Err(derive_err) => {
                        let combined = format!(
                            "create-api-key failed: {create_err}; derive-api-key failed: {derive_err}"
                        );
                        if self.looks_like_first_time_wallet_error(&combined) {
                            return Err(format!(
                                "Polymarket CLOB API key bootstrap failed: first-time Polymarket trading setup is required. Complete one-time onboarding in Polymarket, then retry. Details: {combined}"
                            ));
                        }
                        Err(format!(
                            "Polymarket CLOB API key bootstrap failed: {combined}"
                        ))
                    }
                }
            }
            Err(create_err) => {
                let combined = format!("create-api-key failed: {create_err}");
                if self.looks_like_first_time_wallet_error(&combined) {
                    return Err(format!(
                        "Polymarket CLOB API key bootstrap failed: first-time Polymarket trading setup is required. Complete one-time onboarding in Polymarket, then retry. Details: {combined}"
                    ));
                }
                Err(format!(
                    "Polymarket CLOB API key bootstrap failed: {combined}"
                ))
            }
        }
    }

    pub(crate) fn derive_api_key(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        let url = format!("{CLOB_API_BASE}{CLOB_AUTH_DERIVE_API_KEY_PATH}");
        let response = self
            .with_l1_headers(self.http.get(&url), l1_auth)
            .send()
            .map_err(|e| format!("derive-api-key request failed: {e}"))?;
        self.parse_credentials_response("derive-api-key", response)
    }

    pub(crate) fn create_api_key(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        let url = format!("{CLOB_API_BASE}{CLOB_AUTH_CREATE_API_KEY_PATH}");
        let response = self
            .with_l1_headers(self.http.post(&url), l1_auth)
            .send()
            .map_err(|e| format!("create-api-key request failed: {e}"))?;
        self.parse_credentials_response("create-api-key", response)
    }

    pub(crate) fn with_l1_headers(
        &self,
        builder: reqwest::blocking::RequestBuilder,
        l1_auth: &ClobL1Auth,
    ) -> reqwest::blocking::RequestBuilder {
        let nonce = l1_auth.nonce.clone().unwrap_or_else(|| "0".to_string());
        builder
            .header(HEADER_POLY_ADDRESS, l1_auth.address.as_str())
            .header(HEADER_POLY_SIGNATURE, l1_auth.signature.as_str())
            .header(HEADER_POLY_TIMESTAMP, l1_auth.timestamp.as_str())
            .header(HEADER_POLY_NONCE, nonce)
    }

    pub(crate) fn parse_credentials_response(
        &self,
        operation: &str,
        response: reqwest::blocking::Response,
    ) -> Result<ClobApiCredentials, String> {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            if body.to_ascii_lowercase().contains("invalid signature") {
                return Err(format!(
                    "{operation} request failed with status {status}: {body}. Polymarket rejected the CLOB L1 auth signature. Common causes: reusing the order signature instead of a dedicated ClobAuth EIP-712 signature, signing ClobAuth with a stale/non-current timestamp, or signing the wrong address/timestamp/nonce/message."
                ));
            }
            return Err(format!(
                "{operation} request failed with status {status}: {body}"
            ));
        }

        let payload: Value = serde_json::from_str(&body).map_err(|e| {
            format!("{operation} succeeded but response was not valid JSON: {e} (body: {body})")
        })?;

        let creds = self
            .extract_credentials(&payload)
            .ok_or_else(|| format!("{operation} response missing key/secret/passphrase"))?;
        self.validate_credentials(&creds)?;
        Ok(creds)
    }

    pub(crate) fn extract_credentials(&self, payload: &Value) -> Option<ClobApiCredentials> {
        fn pick<'a>(obj: &'a Value, names: &[&str]) -> Option<&'a str> {
            names
                .iter()
                .find_map(|key| obj.get(*key).and_then(|v| v.as_str()))
                .map(str::trim)
                .filter(|s| !s.is_empty())
        }

        fn from_obj(obj: &Value) -> Option<ClobApiCredentials> {
            let key = pick(obj, &["apiKey", "api_key", "key"])?;
            let secret = pick(obj, &["secret", "apiSecret", "api_secret"])?;
            let passphrase = pick(obj, &["passphrase", "apiPassphrase", "api_passphrase"])?;
            Some(ClobApiCredentials {
                key: key.to_string(),
                secret: secret.to_string(),
                passphrase: passphrase.to_string(),
            })
        }

        from_obj(payload).or_else(|| payload.get("data").and_then(from_obj))
    }

    pub(crate) fn validate_credentials(&self, creds: &ClobApiCredentials) -> Result<(), String> {
        if creds.key.trim().is_empty() {
            return Err("CLOB api key is empty".to_string());
        }
        if creds.secret.trim().is_empty() {
            return Err("CLOB secret is empty".to_string());
        }
        if creds.passphrase.trim().is_empty() {
            return Err("CLOB passphrase is empty".to_string());
        }
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn validate_l1_auth_for_bootstrap(
        &self,
        l1_auth: &ClobL1Auth,
        order_signature: &str,
    ) -> Result<(), String> {
        if !l1_auth.address.starts_with("0x") || l1_auth.address.len() != 42 {
            return Err("CLOB L1 auth address must be a 0x-prefixed address".to_string());
        }
        if !l1_auth.signature.starts_with("0x") {
            return Err("CLOB L1 auth signature must be a 0x-prefixed hex string".to_string());
        }
        if l1_auth.signature.eq_ignore_ascii_case(order_signature) {
            return Err("CLOB L1 auth signature cannot reuse the signed order signature. Sign the ClobAuth EIP-712 payload from get_polymarket_clob_signature, then pass that wallet signature to ensure_polymarket_clob_credentials or clob_auth.l1_auth.".to_string());
        }
        if !l1_auth.timestamp.chars().all(|c| c.is_ascii_digit()) {
            return Err("CLOB L1 auth timestamp must be a Unix-seconds numeric string".to_string());
        }

        let timestamp = l1_auth
            .timestamp
            .parse::<u64>()
            .map_err(|_| "CLOB L1 auth timestamp must fit in u64".to_string())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(timestamp);
        if now.abs_diff(timestamp) > 86_400 {
            return Err(format!(
                "CLOB L1 auth timestamp is stale or far in the future ({timestamp}). Rebuild the ClobAuth payload with a fresh current/server timestamp, re-sign it, and retry."
            ));
        }

        Ok(())
    }

    pub(crate) fn extract_request_path(&self, url: &str) -> Result<String, String> {
        let parsed = reqwest::Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;
        let mut path = parsed.path().to_string();
        if path.is_empty() {
            path = "/".to_string();
        }
        if let Some(query) = parsed.query() {
            path.push('?');
            path.push_str(query);
        }
        Ok(path)
    }

    pub(crate) fn build_l2_signature(
        &self,
        secret: &str,
        timestamp: &str,
        method: &str,
        request_path: &str,
        body: &str,
    ) -> Result<String, String> {
        use base64::Engine as _;
        use hmac::Mac;

        let key_bytes = self.decode_secret(secret);
        let mut mac =
            HmacSha256::new_from_slice(&key_bytes).map_err(|e| format!("hmac error: {e}"))?;
        mac.update(
            format!(
                "{}{}{}{}",
                timestamp,
                method.to_ascii_uppercase(),
                request_path,
                body
            )
            .as_bytes(),
        );
        let digest = mac.finalize().into_bytes();
        Ok(base64::engine::general_purpose::STANDARD.encode(digest))
    }

    pub(crate) fn decode_secret(&self, secret: &str) -> Vec<u8> {
        use base64::Engine as _;
        base64::engine::general_purpose::URL_SAFE
            .decode(secret)
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(secret))
            .unwrap_or_else(|_| secret.as_bytes().to_vec())
    }

    pub(crate) fn looks_like_first_time_wallet_error(&self, text: &str) -> bool {
        let lower = text.to_ascii_lowercase();
        lower.contains("could not create a new proxy wallet")
            || lower.contains("proxy wallet")
            || lower.contains("funder")
            || lower.contains("profile")
    }

    pub(crate) fn now_unix_timestamp() -> String {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string())
    }
}
