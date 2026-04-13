use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) fn build_preamble() -> String {
    let now = Local::now();
    format!(
        r#"## Role
You specialize in Polymarket prediction markets — discovering markets, analyzing trends, and placing trades.

## Current Date
Today is {} ({}). Use this exact date when interpreting relative terms like 'today', 'tomorrow', and 'yesterday'.

## Popular Tags
- Politics & Elections: election 2026, donald trump, kamala harris, electoral votes
- Crypto & Web3: Bitcoin Conference, Stablecoins, DJT, blast, celestia, eigenlayer
- Sports: EPL, MLS Cup, NCAA, CFB, Cricket, Wimbledon
- International: European Union, ukraine, russia, china
- Economics: stock market, crude oil, recession, gdp
- Technology: ai technology, anthropic

## Polymarket Basics
- Prices are probabilities (0.65 = 65%). Markets resolve to $1 (Yes) or $0 (No).
- Higher volume/liquidity = more reliable markets.

## Trading Flow
1. resolve_polymarket_trade_intent — match request to candidate markets; if ambiguous, ask user to pick
2. build_polymarket_order_preview — resolve token_id/price/size; show preview, require confirmation
3. get_polymarket_clob_signature — returns SYSTEM_NEXT_ACTION for the wallet ClobAuth EIP-712 signature flow
4. ensure_polymarket_clob_credentials — use the exact address/timestamp/nonce from step 3 plus the wallet callback signature; returns clob_auth credentials
5. place_polymarket_order — submit with clob_auth credentials; requires confirmation='confirm'

## Guidelines
- Never skip the preview step or place orders without explicit user confirmation
- When a tool returns SYSTEM_NEXT_ACTION, follow those exact steps and preserve args exactly
- get_polymarket_clob_signature already returns the wallet step and the follow-up args template; do not rebuild them manually
- L1 auth values (address, timestamp, nonce) in step 4 must be identical to what was signed in step 3
- Pass the returned clob_auth object from step 4 directly into place_polymarket_order
- The order signature and the CLOB L1 auth signature are different signatures for different payloads; never reuse the order signature as POLY_SIGNATURE for /auth/derive-api-key
- The CLOB L1 auth signature must be created with a fresh current/server timestamp, then reused unchanged only for the immediate create/derive call
- You have tool access to Polymarket CLOB HTTP APIs; never claim clob.polymarket.com is inaccessible

## Account Context
{}"#,
        now.format("%Y-%m-%d"),
        now.format("%Z"),
        build_account_context()
    )
}

pub(crate) fn build_account_context() -> String {
    let mut context = String::from("Available test accounts:\n");
    context.push_str("- Alice: 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\n");
    context.push_str("- Bob: 0x70997970C51812dc3A010C7d01b50e0d17dc79C8 ⚠️  WARNING: This address is a contract on mainnet forks and will forward ETH - use Charlie, Dave, or Eve for testing ETH transfers!\n");
    context.push_str(
        "\nYou can refer to these accounts by their names (Alice, Bob, Charlie, Dave, Eve).",
    );
    context.push_str("\n\nIMPORTANT: If the user has not connected a wallet, do not assume any hidden fallback network. Ask the host or user to provide an explicit wallet or sandbox/test network before placing orders.");
    context
}

#[derive(Clone, Default)]
pub(crate) struct PolymarketApp;

pub(crate) use crate::tool::*;

// ============================================================================
// Client (blocking)
// ============================================================================

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
pub(crate) const HEADER_X_API_KEY: &str = "X-API-KEY";

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ClobL2Auth {
    pub(crate) signature: Option<String>,
    pub(crate) timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(crate) struct ClobAuthBundle {
    pub(crate) credentials: Option<ClobApiCredentials>,
    pub(crate) l1_auth: Option<ClobL1Auth>,
    pub(crate) l2: Option<ClobL2Auth>,
    pub(crate) auto_create_or_derive: Option<bool>,
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

    // ── Market APIs ──────────────────────────────────────────────────────

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
        if let Some(ref tag) = params.tag {
            query.push(("tag", tag.clone()));
        }

        let resp = self
            .http
            .get(&url)
            .query(&query)
            .send()
            .map_err(|e| format!("Gamma API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("Gamma API error {status}: {text}"));
        }

        let text = resp.text().map_err(|e| format!("read body: {e}"))?;
        let markets: Vec<Market> = serde_json::from_str(&text).map_err(|e| {
            let preview = if text.len() > 500 {
                &text[..500]
            } else {
                &text
            };
            format!("Failed to parse markets: {e}\nPreview: {preview}")
        })?;
        Ok(markets)
    }

    pub(crate) fn get_market(&self, id_or_slug: &str) -> Result<Market, String> {
        let target = classify_market_lookup_target(id_or_slug);
        let lookup = build_market_lookup_request(&target);
        let url = format!("{}{}", GAMMA_API_BASE, lookup.path);

        let resp = self
            .http
            .get(&url)
            .query(&lookup.query)
            .send()
            .map_err(|e| format!("Gamma API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!(
                "Failed to get market {id_or_slug}: {status} - {text}"
            ));
        }

        let text = resp.text().map_err(|e| format!("read body: {e}"))?;

        if matches!(target, MarketLookupTarget::ConditionId(_)) {
            let markets: Vec<Market> =
                serde_json::from_str(&text).map_err(|e| format!("parse markets: {e}"))?;
            return markets
                .into_iter()
                .next()
                .ok_or_else(|| format!("No market found for {id_or_slug}"));
        }

        serde_json::from_str(&text).map_err(|e| format!("parse market: {e}"))
    }

    // ── Trades API ───────────────────────────────────────────────────────

    pub(crate) fn get_trades(&self, params: &GetTradesParams) -> Result<Vec<Trade>, String> {
        let url = format!("{DATA_API_BASE}/trades");
        let mut query: Vec<(&str, String)> = Vec::new();
        if let Some(limit) = params.limit {
            query.push(("limit", limit.to_string()));
        }
        if let Some(offset) = params.offset {
            query.push(("offset", offset.to_string()));
        }
        if let Some(ref market) = params.market {
            query.push(("market", market.clone()));
        }
        if let Some(ref user) = params.user {
            query.push(("user", user.clone()));
        }
        if let Some(ref side) = params.side {
            query.push(("side", side.clone()));
        }

        let resp = self
            .http
            .get(&url)
            .query(&query)
            .send()
            .map_err(|e| format!("Data API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("Data API error {status}: {text}"));
        }

        let text = resp.text().map_err(|e| format!("read body: {e}"))?;
        let trades: Vec<Trade> = serde_json::from_str(&text).map_err(|e| {
            let preview = if text.len() > 500 {
                &text[..500]
            } else {
                &text
            };
            format!("Failed to parse trades: {e}\nPreview: {preview}")
        })?;
        Ok(trades)
    }

    // ── Order submission ─────────────────────────────────────────────────

    pub(crate) fn submit_order(&self, request: SubmitOrderRequest) -> Result<Value, String> {
        if !request.owner.starts_with("0x") || request.owner.len() != 42 {
            return Err("owner must be a 0x-prefixed address".to_string());
        }
        let owner = request.owner.clone();

        if !request.signature.starts_with("0x") {
            return Err("signature must be a 0x-prefixed hex string".to_string());
        }

        let order_obj = request
            .order
            .as_object()
            .ok_or("order must be a JSON object containing the signed order payload")?;
        if order_obj.is_empty() {
            return Err("order payload cannot be empty".to_string());
        }

        let order_signature = request.signature.clone();

        let mut body = Map::new();
        body.insert("owner".to_string(), Value::String(owner.clone()));
        body.insert(
            "signature".to_string(),
            Value::String(order_signature.clone()),
        );
        body.insert("order".to_string(), Value::Object(order_obj.clone()));

        if let Some(client_id) = request.client_id {
            body.insert("clientId".to_string(), Value::String(client_id));
        }
        if let Some(extra) = request.extra_fields
            && let Some(extra_obj) = extra.as_object()
        {
            for (key, value) in extra_obj {
                body.entry(key.clone()).or_insert(value.clone());
            }
        }

        let url = request
            .endpoint
            .unwrap_or_else(|| format!("{CLOB_API_BASE}/order"));

        let body_string =
            serde_json::to_string(&body).map_err(|e| format!("serialize body: {e}"))?;
        let mut req_builder = self.http.post(&url).json(&body);

        if let Some(auth_bundle) = request.clob_auth {
            let auto_bootstrap = auth_bundle.auto_create_or_derive.unwrap_or(true);
            let credentials = match auth_bundle.credentials {
                Some(creds) => {
                    self.validate_credentials(&creds)?;
                    creds
                }
                None if auto_bootstrap => {
                    let l1_auth = auth_bundle.l1_auth.as_ref().ok_or(
                        "CLOB credentials are missing and no L1 auth payload was provided for create/derive.",
                    )?;
                    self.validate_l1_auth_for_bootstrap(l1_auth, &order_signature)?;
                    self.create_or_derive_api_credentials(l1_auth)?
                }
                None => {
                    return Err(
                        "CLOB credentials are required when auto_create_or_derive is disabled."
                            .to_string(),
                    );
                }
            };

            let request_path = self.extract_request_path(&url)?;
            let timestamp = auth_bundle
                .l2
                .as_ref()
                .and_then(|l2| l2.timestamp.clone())
                .unwrap_or_else(Self::now_unix_timestamp);

            let l2_signature = auth_bundle
                .l2
                .as_ref()
                .and_then(|l2| l2.signature.clone())
                .map(Ok)
                .unwrap_or_else(|| {
                    self.build_l2_signature(
                        &credentials.secret,
                        &timestamp,
                        "POST",
                        &request_path,
                        &body_string,
                    )
                })?;

            req_builder = req_builder
                .header(HEADER_POLY_ADDRESS, owner.as_str())
                .header(HEADER_POLY_API_KEY, credentials.key.as_str())
                .header(HEADER_POLY_PASSPHRASE, credentials.passphrase.as_str())
                .header(HEADER_POLY_TIMESTAMP, &timestamp)
                .header(HEADER_POLY_SIGNATURE, &l2_signature)
                .header(HEADER_X_API_KEY, &credentials.key);
        } else if let Some(api_key) = request.api_key {
            req_builder = req_builder.header(HEADER_X_API_KEY, api_key);
        }

        let resp = req_builder
            .send()
            .map_err(|e| format!("Order submission request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("Order submission failed {status}: {text}"));
        }

        resp.json::<Value>()
            .map_err(|e| format!("Failed to parse order response: {e}"))
    }

    // ── CLOB credential management ──────────────────────────────────────

    pub(crate) fn create_or_derive_api_credentials(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        match self.derive_api_key(l1_auth) {
            Ok(creds) => Ok(creds),
            Err(derive_err) => match self.create_api_key(l1_auth) {
                Ok(creds) => Ok(creds),
                Err(create_err) => {
                    let combined = format!(
                        "derive-api-key failed: {derive_err}; create-api-key failed: {create_err}"
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
            },
        }
    }

    pub(crate) fn derive_api_key(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        let url = format!("{CLOB_API_BASE}{CLOB_AUTH_DERIVE_API_KEY_PATH}");
        let resp = self
            .with_l1_headers(self.http.get(&url), l1_auth)
            .send()
            .map_err(|e| format!("derive-api-key request failed: {e}"))?;
        self.parse_credentials_response("derive-api-key", resp)
    }

    pub(crate) fn create_api_key(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        let url = format!("{CLOB_API_BASE}{CLOB_AUTH_CREATE_API_KEY_PATH}");
        let resp = self
            .with_l1_headers(self.http.post(&url), l1_auth)
            .send()
            .map_err(|e| format!("create-api-key request failed: {e}"))?;
        self.parse_credentials_response("create-api-key", resp)
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
                .find_map(|k| obj.get(*k).and_then(|v| v.as_str()))
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
        let skew = now.abs_diff(timestamp);
        if skew > 86_400 {
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
        let prehash = format!(
            "{}{}{}{}",
            timestamp,
            method.to_ascii_uppercase(),
            request_path,
            body
        );
        mac.update(prehash.as_bytes());
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

// ============================================================================
// Data models
// ============================================================================

#[derive(Debug, Default)]
pub(crate) struct GetMarketsParams {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    pub(crate) active: Option<bool>,
    pub(crate) closed: Option<bool>,
    pub(crate) archived: Option<bool>,
    pub(crate) tag: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct GetTradesParams {
    pub(crate) limit: Option<u32>,
    pub(crate) offset: Option<u32>,
    pub(crate) market: Option<String>,
    pub(crate) user: Option<String>,
    pub(crate) side: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Market {
    pub(crate) id: Option<String>,
    pub(crate) question: Option<String>,
    pub(crate) slug: Option<String>,
    pub(crate) condition_id: Option<String>,
    pub(crate) description: Option<String>,
    #[serde(deserialize_with = "deserialize_string_or_array", default)]
    pub(crate) outcomes: Option<Vec<String>>,
    #[serde(deserialize_with = "deserialize_string_or_array", default)]
    pub(crate) outcome_prices: Option<Vec<String>>,
    pub(crate) volume: Option<String>,
    pub(crate) volume_num: Option<f64>,
    pub(crate) liquidity: Option<String>,
    pub(crate) liquidity_num: Option<f64>,
    pub(crate) start_date: Option<String>,
    pub(crate) end_date: Option<String>,
    pub(crate) image: Option<String>,
    pub(crate) active: Option<bool>,
    pub(crate) closed: Option<bool>,
    pub(crate) archived: Option<bool>,
    pub(crate) category: Option<String>,
    pub(crate) market_type: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

pub(crate) fn deserialize_string_or_array<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct StringOrArrayVisitor;

    impl<'de> Visitor<'de> for StringOrArrayVisitor {
        type Value = Option<Vec<String>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or array of strings")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D: serde::Deserializer<'de>>(
            self,
            deserializer: D,
        ) -> Result<Self::Value, D::Error> {
            deserializer.deserialize_any(self)
        }

        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            match serde_json::from_str::<Vec<String>>(v) {
                Ok(arr) => Ok(Some(arr)),
                Err(_) => Ok(Some(vec![v.to_string()])),
            }
        }

        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut vec = Vec::new();
            while let Some(elem) = seq.next_element()? {
                vec.push(elem);
            }
            Ok(Some(vec))
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(None)
        }
    }

    deserializer.deserialize_option(StringOrArrayVisitor)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Trade {
    pub(crate) id: Option<String>,
    pub(crate) market: Option<String>,
    pub(crate) asset: Option<String>,
    pub(crate) side: Option<String>,
    pub(crate) size: Option<f64>,
    pub(crate) price: Option<f64>,
    pub(crate) timestamp: Option<i64>,
    pub(crate) transaction_hash: Option<String>,
    pub(crate) outcome: Option<String>,
    pub(crate) proxy_wallet: Option<String>,
    pub(crate) condition_id: Option<String>,
    pub(crate) title: Option<String>,
    pub(crate) slug: Option<String>,
    pub(crate) icon: Option<String>,
    #[serde(flatten)]
    pub(crate) extra: HashMap<String, Value>,
}

#[derive(Debug, Clone)]
pub(crate) struct SubmitOrderRequest {
    pub(crate) owner: String,
    pub(crate) signature: String,
    pub(crate) order: Value,
    pub(crate) client_id: Option<String>,
    pub(crate) endpoint: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) clob_auth: Option<ClobAuthBundle>,
    pub(crate) extra_fields: Option<Value>,
}

// ============================================================================
// Market lookup helpers
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MarketLookupTarget {
    MarketId(String),
    Slug(String),
    ConditionId(String),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MarketLookupRequest {
    pub(crate) path: String,
    pub(crate) query: HashMap<String, String>,
}

pub(crate) fn classify_market_lookup_target(raw: &str) -> MarketLookupTarget {
    if raw.starts_with("0x") {
        return MarketLookupTarget::ConditionId(raw.to_string());
    }
    if raw.chars().all(|ch| ch.is_ascii_digit()) {
        return MarketLookupTarget::MarketId(raw.to_string());
    }
    if raw.contains('-') {
        return MarketLookupTarget::Slug(raw.to_string());
    }
    MarketLookupTarget::MarketId(raw.to_string())
}

pub(crate) fn build_market_lookup_request(target: &MarketLookupTarget) -> MarketLookupRequest {
    match target {
        MarketLookupTarget::MarketId(id) => MarketLookupRequest {
            path: format!("/markets/{id}"),
            query: HashMap::new(),
        },
        MarketLookupTarget::Slug(slug) => MarketLookupRequest {
            path: format!("/markets/slug/{slug}"),
            query: HashMap::new(),
        },
        MarketLookupTarget::ConditionId(condition_id) => {
            let mut query = HashMap::new();
            query.insert("condition_ids".to_string(), condition_id.clone());
            query.insert("limit".to_string(), "1".to_string());
            MarketLookupRequest {
                path: "/markets".to_string(),
                query,
            }
        }
    }
}

// ============================================================================
// Intent parsing and ranking helpers
// ============================================================================

pub(crate) const DEFAULT_INTENT_SEARCH_MARKET_LIMIT: u32 = 200;
pub(crate) const MAX_INTENT_SEARCH_MARKET_LIMIT: u32 = 1000;
pub(crate) const DEFAULT_INTENT_CANDIDATE_LIMIT: usize = 5;
pub(crate) const DEFAULT_AMBIGUITY_MIN_SCORE: f64 = 0.75;
pub(crate) const DEFAULT_AMBIGUITY_SCORE_GAP: f64 = 0.08;

#[derive(Debug, Clone)]
pub(crate) struct ParsedTradeIntent {
    pub(crate) action: Option<String>,
    pub(crate) outcome: Option<String>,
    pub(crate) year: Option<i32>,
    pub(crate) size_usd: Option<f64>,
    pub(crate) search_query: String,
    pub(crate) query_tokens: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RankedMarketCandidate {
    pub(crate) market_id: Option<String>,
    pub(crate) condition_id: Option<String>,
    pub(crate) question: Option<String>,
    pub(crate) slug: Option<String>,
    pub(crate) close_time: Option<String>,
    pub(crate) yes_price: Option<f64>,
    pub(crate) no_price: Option<f64>,
    pub(crate) volume: Option<f64>,
    pub(crate) liquidity: Option<f64>,
    pub(crate) score: f64,
    pub(crate) url: Option<String>,
}

pub(crate) fn parse_trade_intent(input: &str) -> Result<ParsedTradeIntent, String> {
    let normalized = normalize_text(input);
    let tokens: Vec<&str> = normalized.split_whitespace().collect();

    let action = if tokens.contains(&"buy") {
        Some("buy".to_string())
    } else if tokens.contains(&"sell") {
        Some("sell".to_string())
    } else {
        None
    };

    let outcome = if tokens.contains(&"yes") {
        Some("yes".to_string())
    } else if tokens.contains(&"no") {
        Some("no".to_string())
    } else {
        None
    };

    let year = tokens
        .iter()
        .filter_map(|t| t.parse::<i32>().ok())
        .find(|y| *y >= 2024 && *y <= 2100);

    let size_usd = extract_size_usd(input);

    let stopwords = [
        "buy", "sell", "yes", "no", "on", "in", "for", "to", "at", "by", "with", "bet", "trade",
        "place", "will", "the", "a", "an", "of",
    ];
    let mut query_tokens = Vec::new();
    for tok in tokens {
        if tok.len() <= 1 {
            continue;
        }
        if stopwords.contains(&tok) {
            continue;
        }
        query_tokens.push(tok.to_string());
    }

    if query_tokens.is_empty() {
        return Err("Unable to parse request into a searchable market query".to_string());
    }

    let search_query = query_tokens.join(" ");
    Ok(ParsedTradeIntent {
        action,
        outcome,
        year,
        size_usd,
        search_query,
        query_tokens,
    })
}

pub(crate) fn rank_market_candidates(
    intent: &ParsedTradeIntent,
    markets: &[Market],
) -> Vec<RankedMarketCandidate> {
    let mut ranked: Vec<RankedMarketCandidate> = markets
        .iter()
        .filter_map(|m| {
            let question = m.question.clone()?;
            let question_tokens = tokenize_for_match(&question);
            let overlap = token_overlap_ratio(&intent.query_tokens, &question_tokens);
            if overlap <= 0.0 {
                return None;
            }

            let mut score = overlap;

            if let Some(year) = intent.year
                && question.contains(&year.to_string())
            {
                score += 0.25;
            }

            if let Some(outcome) = &intent.outcome
                && question.to_ascii_lowercase().contains(outcome)
            {
                score += 0.05;
            }

            if let Some(volume) = m.volume_num {
                score += (volume.max(1.0).ln() / 20.0).min(0.15);
            }

            let (yes_price, no_price) = extract_yes_no_prices(m);
            let url = m
                .slug
                .as_ref()
                .map(|slug| format!("https://polymarket.com/market/{slug}"));

            Some(RankedMarketCandidate {
                market_id: m.id.clone(),
                condition_id: m.condition_id.clone(),
                question: Some(question),
                slug: m.slug.clone(),
                close_time: m.end_date.clone(),
                yes_price,
                no_price,
                volume: m.volume_num,
                liquidity: m.liquidity_num,
                score,
                url,
            })
        })
        .collect();

    ranked.sort_by(|a, b| b.score.total_cmp(&a.score));
    ranked
}

pub(crate) fn requires_selection(top1_score: f64, top2_score: Option<f64>) -> bool {
    if top1_score < DEFAULT_AMBIGUITY_MIN_SCORE {
        return true;
    }
    if let Some(second) = top2_score {
        return (top1_score - second) < DEFAULT_AMBIGUITY_SCORE_GAP;
    }
    false
}

pub(crate) fn normalize_text(input: &str) -> String {
    input
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '$' || c == '.' || c.is_ascii_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect()
}

pub(crate) fn tokenize_for_match(input: &str) -> Vec<String> {
    normalize_text(input)
        .split_whitespace()
        .filter(|t| t.len() > 1)
        .map(|t| t.to_string())
        .collect()
}

pub(crate) fn token_overlap_ratio(query_tokens: &[String], question_tokens: &[String]) -> f64 {
    if query_tokens.is_empty() || question_tokens.is_empty() {
        return 0.0;
    }
    let question_set: std::collections::HashSet<&str> =
        question_tokens.iter().map(String::as_str).collect();
    let matches = query_tokens
        .iter()
        .filter(|q| question_set.contains(q.as_str()))
        .count();
    matches as f64 / query_tokens.len() as f64
}

pub(crate) fn extract_size_usd(raw_input: &str) -> Option<f64> {
    let lower = raw_input.to_ascii_lowercase();
    if let Some(idx) = lower.find('$') {
        let slice = &lower[idx + 1..];
        let mut number = String::new();
        for ch in slice.chars() {
            if ch.is_ascii_digit() || ch == '.' {
                number.push(ch);
            } else {
                break;
            }
        }
        if !number.is_empty() {
            return number.parse::<f64>().ok();
        }
    }

    let normalized = normalize_text(&lower);
    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    for window in tokens.windows(2) {
        if let [num, unit] = window
            && ["usd", "usdc", "dollars", "dollar"].contains(unit)
            && let Ok(v) = num.parse::<f64>()
        {
            return Some(v);
        }
    }
    None
}

pub(crate) fn extract_yes_no_prices(market: &Market) -> (Option<f64>, Option<f64>) {
    let outcomes = market.outcomes.clone().unwrap_or_default();
    let prices = market.outcome_prices.clone().unwrap_or_default();
    if outcomes.is_empty() || prices.is_empty() {
        return (None, None);
    }

    let mut yes_price = None;
    let mut no_price = None;
    for (idx, outcome) in outcomes.iter().enumerate() {
        let price = prices.get(idx).and_then(|v| v.parse::<f64>().ok());
        let o = outcome.to_ascii_lowercase();
        if o == "yes" {
            yes_price = price;
        } else if o == "no" {
            no_price = price;
        }
    }

    if yes_price.is_none() && !prices.is_empty() {
        yes_price = prices.first().and_then(|v| v.parse::<f64>().ok());
    }
    if no_price.is_none() && prices.len() > 1 {
        no_price = prices.get(1).and_then(|v| v.parse::<f64>().ok());
    }

    (yes_price, no_price)
}

pub(crate) fn extract_outcome_token_ids(market: &Market) -> (Option<String>, Option<String>) {
    if let Some(tokens) = market
        .extra
        .get("clobTokenIds")
        .or_else(|| market.extra.get("clob_token_ids"))
        .or_else(|| market.extra.get("tokenIds"))
        .or_else(|| market.extra.get("token_ids"))
    {
        let values = parse_token_id_list(tokens);
        if !values.is_empty() {
            let yes_no = map_token_ids_by_outcomes(market.outcomes.as_ref(), &values);
            if yes_no.0.is_some() || yes_no.1.is_some() {
                return yes_no;
            }
        }
    }

    if let Some(tokens) = market.extra.get("tokens") {
        let parsed = parse_tokens_array(tokens);
        if parsed.0.is_some() || parsed.1.is_some() {
            return parsed;
        }
    }

    (None, None)
}

pub(crate) fn parse_token_id_list(value: &Value) -> Vec<String> {
    match value {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(str::trim))
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect(),
        Value::String(raw) => {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                return Vec::new();
            }
            if let Ok(parsed) = serde_json::from_str::<Vec<String>>(trimmed) {
                return parsed
                    .into_iter()
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty())
                    .collect();
            }
            trimmed
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect()
        }
        _ => Vec::new(),
    }
}

pub(crate) fn map_token_ids_by_outcomes(
    outcomes: Option<&Vec<String>>,
    token_ids: &[String],
) -> (Option<String>, Option<String>) {
    let mut yes = None;
    let mut no = None;

    if let Some(outcomes) = outcomes {
        for (idx, outcome) in outcomes.iter().enumerate() {
            let token_id = token_ids.get(idx).cloned();
            let outcome_normalized = outcome.trim().to_ascii_lowercase();
            if outcome_normalized == "yes" {
                yes = token_id;
            } else if outcome_normalized == "no" {
                no = token_id;
            }
        }
    }

    if yes.is_none() && !token_ids.is_empty() {
        yes = token_ids.first().cloned();
    }
    if no.is_none() && token_ids.len() > 1 {
        no = token_ids.get(1).cloned();
    }

    (yes, no)
}

pub(crate) fn parse_tokens_array(value: &Value) -> (Option<String>, Option<String>) {
    let Some(arr) = value.as_array() else {
        return (None, None);
    };

    let mut yes = None;
    let mut no = None;
    for token in arr {
        let Some(obj) = token.as_object() else {
            continue;
        };
        let token_id = obj
            .get("token_id")
            .or_else(|| obj.get("tokenId"))
            .or_else(|| obj.get("id"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let outcome = obj
            .get("outcome")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_ascii_lowercase());

        match outcome.as_deref() {
            Some("yes") if yes.is_none() => yes = token_id.clone(),
            Some("no") if no.is_none() => no = token_id.clone(),
            _ => {}
        }
    }

    if yes.is_none() {
        yes = arr
            .first()
            .and_then(|v| {
                v.get("token_id")
                    .or_else(|| v.get("tokenId"))
                    .or_else(|| v.get("id"))
            })
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
    }
    if no.is_none() {
        no = arr
            .get(1)
            .and_then(|v| {
                v.get("token_id")
                    .or_else(|| v.get("tokenId"))
                    .or_else(|| v.get("id"))
            })
            .and_then(|v| v.as_str())
            .map(|v| v.to_string());
    }

    (yes, no)
}

// ============================================================================
// Validation helpers
// ============================================================================

pub(crate) fn normalize_yes_no(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "y" => Ok("YES".to_string()),
        "no" | "n" => Ok("NO".to_string()),
        _ => Err("outcome must be YES or NO".to_string()),
    }
}

pub(crate) fn normalize_side(value: Option<&str>) -> Result<String, String> {
    match value {
        None => Ok("BUY".to_string()),
        Some(v) => match v.trim().to_ascii_uppercase().as_str() {
            "BUY" => Ok("BUY".to_string()),
            "SELL" => Ok("SELL".to_string()),
            _ => Err("side must be BUY or SELL".to_string()),
        },
    }
}

pub(crate) fn validate_confirmation_token(value: Option<&str>) -> Result<(), String> {
    let Some(raw) = value else {
        return Err(
            "Missing explicit confirmation. Require confirmation='confirm' before order submission."
                .to_string(),
        );
    };
    if raw.trim().eq_ignore_ascii_case("confirm") {
        return Ok(());
    }
    Err("Invalid confirmation token. Expected confirmation='confirm'.".to_string())
}

// ============================================================================
// Tool 1: SearchPolymarket
// ============================================================================

pub(crate) struct SearchPolymarket;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchPolymarketArgs {
    /// Maximum number of markets to return (default: 100, max: 1000)
    pub(crate) limit: Option<u32>,
    /// Pagination offset (default: 0)
    pub(crate) offset: Option<u32>,
    /// Filter for active markets
    pub(crate) active: Option<bool>,
    /// Filter for closed markets
    pub(crate) closed: Option<bool>,
    /// Filter for archived markets
    pub(crate) archived: Option<bool>,
    /// Filter by tag/category (e.g., 'crypto', 'sports', 'politics')
    pub(crate) tag: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn public_market_apis_smoke() {
        let client = PolymarketClient::new().expect("client should build");
        let markets = client
            .get_markets(&GetMarketsParams {
                limit: Some(1),
                offset: Some(0),
                active: Some(true),
                closed: Some(false),
                archived: Some(false),
                tag: None,
            })
            .expect("market list request should succeed");

        assert!(!markets.is_empty(), "expected at least one active market");

        let first = &markets[0];
        let slug = first
            .slug
            .as_deref()
            .expect("market list response should include slug");
        let condition_id = first
            .condition_id
            .as_deref()
            .expect("market list response should include condition_id");

        let by_slug = client
            .get_market(slug)
            .expect("market lookup by slug should succeed");
        assert_eq!(by_slug.slug.as_deref(), Some(slug));

        let by_condition_id = client
            .get_market(condition_id)
            .expect("market lookup by condition_id should succeed");
        assert_eq!(by_condition_id.condition_id.as_deref(), Some(condition_id));

        let trades = client
            .get_trades(&GetTradesParams {
                limit: Some(1),
                offset: Some(0),
                market: None,
                user: None,
                side: None,
            })
            .expect("trades request should succeed");
        assert!(!trades.is_empty(), "expected at least one trade");
    }

    #[test]
    fn default_order_path_matches_docs() {
        let client = PolymarketClient::new().expect("client should build");
        let request = SubmitOrderRequest {
            owner: "0x1234567890123456789012345678901234567890".to_string(),
            signature: "0xdeadbeef".to_string(),
            order: json!({"maker":"0x1234567890123456789012345678901234567890"}),
            client_id: None,
            endpoint: None,
            api_key: None,
            clob_auth: None,
            extra_fields: None,
        };

        let url = request
            .endpoint
            .unwrap_or_else(|| format!("{}/order", CLOB_API_BASE));
        let path = client
            .extract_request_path(&url)
            .expect("request path extraction should succeed");

        assert_eq!(path, "/order");
    }

    #[test]
    fn rejects_reusing_order_signature_for_clob_l1_auth() {
        let client = PolymarketClient::new().expect("client should build");
        let shared_signature = "0x4f5ebd67f345143fe72b896c26bc11cc69c44fc8e75f2c4bfa2aa6b51316cf84552633fe49c00e9e43bd3d16d1a7c993095f0f7c8d35e04e72993f2d93c122741c";
        let err = client
            .validate_l1_auth_for_bootstrap(
                &ClobL1Auth {
                    address: "0x5D907BEa404e6F821d467314a9cA07663CF64c9B".to_string(),
                    signature: shared_signature.to_string(),
                    timestamp: PolymarketClient::now_unix_timestamp(),
                    nonce: Some("0".to_string()),
                },
                shared_signature,
            )
            .expect_err("should reject reused signature");

        assert!(
            err.contains("cannot reuse the signed order signature"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn rejects_stale_clob_l1_timestamp() {
        let client = PolymarketClient::new().expect("client should build");
        let err = client
            .validate_l1_auth_for_bootstrap(
                &ClobL1Auth {
                    address: "0x5D907BEa404e6F821d467314a9cA07663CF64c9B".to_string(),
                    signature: "0xdeadbeef".to_string(),
                    timestamp: "1744329600".to_string(),
                    nonce: Some("0".to_string()),
                },
                "0xbeadfeed",
            )
            .expect_err("should reject stale timestamp");

        assert!(
            err.contains("fresh current/server timestamp"),
            "unexpected error: {err}"
        );
    }
}
