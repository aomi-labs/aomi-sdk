use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use chrono::Local;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::time::Duration;

pub(crate) fn build_preamble() -> String {
    let now = Local::now();
    format!(
        r#"## Role
You are the Prediction Wizard, an expert in prediction markets. You trade on Polymarket and Kalshi via the Simmer SDK, or directly on Polymarket via wallet signing.

## Current Date
Today is {} ({}). Use this exact date when interpreting relative terms like 'today', 'tomorrow', and 'yesterday'.

## Simmer SDK
Simmer SDK (simmer.markets) -- unified trading API with one key (sk_...).

Venues: 'simmer' = sandbox ($SIM, starts 10k, no real money, no KYC). 'polymarket' = real USDC, non-US only (Polygon). 'kalshi' = real USD, US only (CFTC-regulated). Always default to simmer unless user explicitly asks otherwise.

Setup: simmer_register -> get api_key + claim_url -> user runs /apikey simmer <key> -> user visits claim_url to complete KYC and link their Simmer account. If user already has a key, skip to /apikey. When presenting the claim link, always tell the user: 'Visit this link to verify your identity and claim your agent. This is where Simmer handles KYC -- once complete, real-money trading on Polymarket/Kalshi unlocks.'

Trade flow: search markets -> fetch_simmer_market_context (ALWAYS check pre-trade warnings) -> simmer_place_order with reasoning. The reasoning field is PUBLIC on the user's Simmer profile and builds reputation -- write a real thesis, not 'user asked me to buy'.

Use dry_run=true for >$50 trades. Use simmer_briefing for a full dashboard in one call. Limits: $100/trade, $500/day, auto stop-loss -50%, take-profit +35%.
All Simmer tools require the api_key argument. When the user configures their key via /apikey, you will receive it in a system message. Pass this key to every Simmer tool call.
Auth errors -> remind user: /apikey simmer <key>

Compliance: Aomi is only an interface -- we don't hold funds or verify identity. Simmer holds custody and handles KYC/compliance. The claim flow at simmer.markets is the KYC point. Simmer blocks US users from Polymarket and routes them to Kalshi. Users are responsible for legality in their jurisdiction.

IMPORTANT -- show this disclaimer on registration (before claim link), on first real-money trade (venue=polymarket or kalshi), and in /apikey simmer response:
'Aomi is an interface to Simmer (simmer.markets) -- we do not hold your funds. KYC and compliance are handled by Simmer and the underlying platforms. You are responsible for ensuring prediction market trading is legal in your jurisdiction. US users: Polymarket is NOT available to you; use Kalshi (CFTC-regulated) instead. By claiming your agent you agree to Simmer ToS.'

## Polymarket Direct Path
Polymarket direct path -- ONLY for users who explicitly ask for direct wallet signing via /connect. Do NOT use these tools if the user has a Simmer key -- use simmer_place_order instead. Flow: resolve_polymarket_trade_intent -> build_polymarket_order_preview -> get_polymarket_clob_signature -> ensure_polymarket_clob_credentials -> place_polymarket_order. Requires connected wallet with a real address (not empty). If wallet address is missing, tell user to /connect first.

Capability: you CAN access Polymarket CLOB HTTP APIs through tools. Never claim you cannot call clob.polymarket.com or that API access is unavailable. Use the tools to perform CLOB auth and order submission steps.

Reference -- Chain: Polygon (137). USDC: 0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174. CTF Exchange: 0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E.
Auth: EIP-712 ClobAuth signature (address from USER_STATE, timestamp=now, nonce=0, domain='ClobAuthDomain', chain 137). POST clob.polymarket.com/auth/api-key to derive API creds, then L2 header auth for orders (POST clob.polymarket.com/order).
Optimization: get wallet address from USER_STATE -- skip redundant lookups. ensure_polymarket_clob_credentials caches derived keys per session.

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
pub(crate) struct PredictionApp;

pub(crate) use crate::tool::*;

// ============================================================================
// Simmer Client (blocking)
// ============================================================================

pub(crate) const SIMMER_API_URL: &str = "https://api.simmer.markets";

#[derive(Clone)]
pub(crate) struct SimmerClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_key: String,
}

impl SimmerClient {
    pub(crate) fn new(api_key: &str, _venue: &str) -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_key: api_key.to_string(),
        })
    }

    pub(crate) fn auth_header(&self) -> String {
        format!("Bearer {}", self.api_key)
    }

    pub(crate) fn send_json(
        request: reqwest::blocking::RequestBuilder,
        operation: &str,
    ) -> Result<Value, String> {
        let response = request
            .send()
            .map_err(|e| format!("[simmer] {operation} request failed: {e}"))?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[simmer] {operation} failed: {status} {body}"));
        }
        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[simmer] {operation} decode failed: {e}; body: {body}"))
    }

    pub(crate) fn get_agent_status(&self) -> Result<Value, String> {
        let url = format!("{}/api/sdk/agents/me", SIMMER_API_URL);
        let req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        Self::send_json(req, "get_agent_status")
    }

    pub(crate) fn get_briefing(&self, since: Option<&str>) -> Result<Value, String> {
        let mut url = format!("{}/api/sdk/briefing", SIMMER_API_URL);
        if let Some(since) = since {
            url.push_str(&format!("?since={}", since));
        }
        let req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        Self::send_json(req, "get_briefing")
    }

    pub(crate) fn get_market_context(&self, market_id: &str) -> Result<Value, String> {
        let url = format!("{}/api/sdk/context/{}", SIMMER_API_URL, market_id);
        let req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        Self::send_json(req, "get_market_context")
    }

    pub(crate) fn trade(&self, body: Value) -> Result<Value, String> {
        let url = format!("{}/api/sdk/trade", SIMMER_API_URL);
        let req = self
            .http
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&body);
        Self::send_json(req, "trade")
    }

    pub(crate) fn get_positions(&self) -> Result<Value, String> {
        let url = format!("{}/api/sdk/positions", SIMMER_API_URL);
        let req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        Self::send_json(req, "get_positions")
    }

    pub(crate) fn get_portfolio(&self) -> Result<Value, String> {
        let url = format!("{}/api/sdk/portfolio", SIMMER_API_URL);
        let req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        Self::send_json(req, "get_portfolio")
    }

    pub(crate) fn get_markets(
        &self,
        import_source: Option<&str>,
        status: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Value, String> {
        let mut url = format!("{}/api/sdk/markets", SIMMER_API_URL);
        let mut params = vec![];
        if let Some(src) = import_source {
            params.push(format!("import_source={}", src));
        }
        if let Some(st) = status {
            params.push(format!("status={}", st));
        }
        if let Some(l) = limit {
            params.push(format!("limit={}", l));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
        let req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        Self::send_json(req, "get_markets")
    }
}

pub(crate) fn simmer_register_agent(
    name: &str,
    description: Option<&str>,
) -> Result<Value, String> {
    let http = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    let mut body = json!({ "name": name });
    if let Some(desc) = description {
        body["description"] = Value::String(desc.to_string());
    }

    let req = http
        .post(format!("{}/api/sdk/agents/register", SIMMER_API_URL))
        .json(&body);
    SimmerClient::send_json(req, "register_agent")
}

pub(crate) fn parse_venue(venue: &str) -> Result<String, String> {
    match venue.to_lowercase().as_str() {
        "simmer" | "polymarket" | "kalshi" => Ok(venue.to_lowercase()),
        other => Err(format!(
            "Unknown venue: {}. Use simmer, polymarket, or kalshi.",
            other
        )),
    }
}

// ============================================================================
// Polymarket Client (blocking)
// ============================================================================

pub(crate) const GAMMA_API_BASE: &str = "https://gamma-api.polymarket.com";
pub(crate) const DATA_API_BASE: &str = "https://data-api.polymarket.com";
pub(crate) const CLOB_API_BASE: &str = "https://clob.polymarket.com";

type HmacSha256 = Hmac<sha2::Sha256>;

#[derive(Clone)]
pub(crate) struct PolymarketClient {
    pub(crate) http: reqwest::blocking::Client,
}

impl PolymarketClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self { http })
    }

    pub(crate) fn get_markets(&self, params: &GetMarketsParams) -> Result<Vec<Value>, String> {
        let url = format!("{}/markets", GAMMA_API_BASE);
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
        resp.json::<Vec<Value>>()
            .map_err(|e| format!("Gamma API decode failed: {e}"))
    }

    pub(crate) fn get_market(&self, id_or_slug: &str) -> Result<Value, String> {
        let (path, query) = classify_polymarket_lookup(id_or_slug);
        let url = format!("{}{}", GAMMA_API_BASE, path);
        let resp = self
            .http
            .get(&url)
            .query(&query)
            .send()
            .map_err(|e| format!("Gamma API request failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!(
                "Failed to get market {id_or_slug}: {status} {text}"
            ));
        }
        let text = resp.text().map_err(|e| format!("read body failed: {e}"))?;
        // If condition_id lookup, response is an array
        if id_or_slug.starts_with("0x") {
            let markets: Vec<Value> =
                serde_json::from_str(&text).map_err(|e| format!("decode failed: {e}"))?;
            markets
                .into_iter()
                .next()
                .ok_or_else(|| format!("No market found for {id_or_slug}"))
        } else {
            serde_json::from_str(&text).map_err(|e| format!("decode failed: {e}"))
        }
    }

    pub(crate) fn get_trades(&self, params: &GetTradesParams) -> Result<Vec<Value>, String> {
        let url = format!("{}/trades", DATA_API_BASE);
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
        resp.json::<Vec<Value>>()
            .map_err(|e| format!("Data API decode failed: {e}"))
    }

    pub(crate) fn submit_order(&self, request: SubmitOrderRequest) -> Result<Value, String> {
        if !request.owner.starts_with("0x") || request.owner.len() != 42 {
            return Err("owner must be a 0x-prefixed address".to_string());
        }
        if !request.signature.starts_with("0x") {
            return Err("signature must be a 0x-prefixed hex string".to_string());
        }
        let order_obj = request
            .order
            .as_object()
            .ok_or("order must be a JSON object")?;
        if order_obj.is_empty() {
            return Err("order payload cannot be empty".to_string());
        }

        let owner = request.owner.clone();
        let mut body = Map::new();
        body.insert("owner".to_string(), Value::String(owner.clone()));
        body.insert("signature".to_string(), Value::String(request.signature));
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
            .unwrap_or_else(|| format!("{}/orders", CLOB_API_BASE));

        let mut req_builder = self.http.post(&url).json(&body);
        let body_string =
            serde_json::to_string(&body).map_err(|e| format!("serialize body: {e}"))?;

        if let Some(auth_bundle) = request.clob_auth {
            let creds = match auth_bundle.credentials {
                Some(creds) => creds,
                None => {
                    let l1_auth = auth_bundle
                        .l1_auth
                        .ok_or("CLOB credentials missing and no L1 auth provided for bootstrap")?;
                    self.create_or_derive_api_credentials(&l1_auth)?
                }
            };

            let request_path = extract_request_path(&url)?;
            let timestamp = auth_bundle
                .l2_timestamp
                .unwrap_or_else(now_unix_timestamp);

            let l2_signature = match auth_bundle.l2_signature {
                Some(sig) => sig,
                None => build_l2_signature(
                    &creds.secret,
                    &timestamp,
                    "POST",
                    &request_path,
                    &body_string,
                )?,
            };

            req_builder = req_builder
                .header("POLY_ADDRESS", owner.as_str())
                .header("POLY_API_KEY", creds.key.as_str())
                .header("POLY_PASSPHRASE", creds.passphrase.as_str())
                .header("POLY_TIMESTAMP", &timestamp)
                .header("POLY_SIGNATURE", &l2_signature)
                .header("X-API-KEY", creds.key);
        } else if let Some(api_key) = request.api_key {
            req_builder = req_builder.header("X-API-KEY", api_key);
        }

        let resp = req_builder
            .send()
            .map_err(|e| format!("Order submission failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("Order submission failed {status}: {text}"));
        }
        resp.json::<Value>()
            .map_err(|e| format!("Failed to parse order response: {e}"))
    }

    pub(crate) fn create_or_derive_api_credentials(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        // Try derive first, then create
        match self.derive_api_key(l1_auth) {
            Ok(creds) => Ok(creds),
            Err(_) => self
                .create_api_key(l1_auth)
                .map_err(|e| format!("CLOB API key bootstrap failed: {e}")),
        }
    }

    pub(crate) fn derive_api_key(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        let url = format!("{}/auth/derive-api-key", CLOB_API_BASE);
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
        let url = format!("{}/auth/api-key", CLOB_API_BASE);
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
        let nonce = l1_auth.nonce.as_deref().unwrap_or("0");
        builder
            .header("POLY_ADDRESS", l1_auth.address.as_str())
            .header("POLY_SIGNATURE", l1_auth.signature.as_str())
            .header("POLY_TIMESTAMP", l1_auth.timestamp.as_str())
            .header("POLY_NONCE", nonce)
    }

    pub(crate) fn parse_credentials_response(
        &self,
        operation: &str,
        resp: reqwest::blocking::Response,
    ) -> Result<ClobApiCredentials, String> {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("{operation} failed {status}: {body}"));
        }
        let payload: Value =
            serde_json::from_str(&body).map_err(|e| format!("{operation} decode: {e}"))?;
        extract_credentials(&payload)
            .ok_or_else(|| format!("{operation} response missing key/secret/passphrase"))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClobApiCredentials {
    pub(crate) key: String,
    pub(crate) secret: String,
    pub(crate) passphrase: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ClobL1Auth {
    pub(crate) address: String,
    pub(crate) signature: String,
    pub(crate) timestamp: String,
    pub(crate) nonce: Option<String>,
}

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

pub(crate) struct ClobAuthBundle {
    pub(crate) credentials: Option<ClobApiCredentials>,
    pub(crate) l1_auth: Option<ClobL1Auth>,
    pub(crate) l2_signature: Option<String>,
    pub(crate) l2_timestamp: Option<String>,
}

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

pub(crate) fn classify_polymarket_lookup(raw: &str) -> (String, Vec<(String, String)>) {
    if raw.starts_with("0x") {
        (
            "/markets".to_string(),
            vec![
                ("condition_ids".to_string(), raw.to_string()),
                ("limit".to_string(), "1".to_string()),
            ],
        )
    } else if raw.contains('-') {
        (format!("/markets/slug/{}", raw), vec![])
    } else {
        (format!("/markets/{}", raw), vec![])
    }
}

pub(crate) fn extract_credentials(payload: &Value) -> Option<ClobApiCredentials> {
    pub(crate) fn pick<'a>(obj: &'a Value, names: &[&str]) -> Option<&'a str> {
        names
            .iter()
            .find_map(|k| obj.get(*k).and_then(|v| v.as_str()))
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    pub(crate) fn from_obj(obj: &Value) -> Option<ClobApiCredentials> {
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

pub(crate) fn extract_request_path(url: &str) -> Result<String, String> {
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
    secret: &str,
    timestamp: &str,
    method: &str,
    request_path: &str,
    body: &str,
) -> Result<String, String> {
    use base64::Engine as _;
    let key_bytes = base64::engine::general_purpose::URL_SAFE
        .decode(secret)
        .or_else(|_| base64::engine::general_purpose::STANDARD.decode(secret))
        .unwrap_or_else(|_| secret.as_bytes().to_vec());
    let mut mac = HmacSha256::new_from_slice(&key_bytes).map_err(|e| format!("hmac error: {e}"))?;
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

pub(crate) fn now_unix_timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

// ============================================================================
// Polymarket helper functions for intent resolution and order building
// ============================================================================

pub(crate) fn extract_yes_no_prices(market: &Value) -> (Option<f64>, Option<f64>) {
    let outcomes = market
        .get("outcomes")
        .and_then(parse_json_string_or_array)
        .unwrap_or_default();
    let prices = market
        .get("outcomePrices")
        .or_else(|| market.get("outcome_prices"))
        .and_then(parse_json_string_or_array)
        .unwrap_or_default();

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

pub(crate) fn parse_json_string_or_array(value: &Value) -> Option<Vec<String>> {
    match value {
        Value::Array(arr) => Some(
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect(),
        ),
        Value::String(s) => serde_json::from_str::<Vec<String>>(s).ok(),
        _ => None,
    }
}

pub(crate) fn extract_outcome_token_ids(market: &Value) -> (Option<String>, Option<String>) {
    let token_keys = ["clobTokenIds", "clob_token_ids", "tokenIds", "token_ids"];
    for key in &token_keys {
        if let Some(tokens_val) = market.get(*key) {
            let values = parse_token_id_list(tokens_val);
            if !values.is_empty() {
                let outcomes_val = market.get("outcomes").and_then(parse_json_string_or_array);
                return map_token_ids_by_outcomes(outcomes_val.as_ref(), &values);
            }
        }
    }

    if let Some(tokens) = market.get("tokens").and_then(Value::as_array) {
        return parse_tokens_array(tokens);
    }

    (None, None)
}

pub(crate) fn parse_token_id_list(value: &Value) -> Vec<String> {
    match value {
        Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
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
            match outcome.trim().to_ascii_lowercase().as_str() {
                "yes" => yes = token_id,
                "no" => no = token_id,
                _ => {}
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

pub(crate) fn parse_tokens_array(arr: &[Value]) -> (Option<String>, Option<String>) {
    let mut yes = None;
    let mut no = None;
    for token in arr {
        let obj = match token.as_object() {
            Some(o) => o,
            None => continue,
        };
        let token_id = obj
            .get("token_id")
            .or_else(|| obj.get("tokenId"))
            .or_else(|| obj.get("id"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let outcome = obj
            .get("outcome")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_ascii_lowercase());
        match outcome.as_deref() {
            Some("yes") if yes.is_none() => yes = token_id,
            Some("no") if no.is_none() => no = token_id,
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

// Intent resolution helpers
pub(crate) const DEFAULT_INTENT_SEARCH_MARKET_LIMIT: u32 = 200;
pub(crate) const MAX_INTENT_SEARCH_MARKET_LIMIT: u32 = 1000;
pub(crate) const DEFAULT_INTENT_CANDIDATE_LIMIT: usize = 5;
pub(crate) const DEFAULT_AMBIGUITY_MIN_SCORE: f64 = 0.75;
pub(crate) const DEFAULT_AMBIGUITY_SCORE_GAP: f64 = 0.08;

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

#[derive(Debug, Clone)]
pub(crate) struct ParsedTradeIntent {
    pub(crate) action: Option<String>,
    pub(crate) outcome: Option<String>,
    pub(crate) year: Option<i32>,
    pub(crate) size_usd: Option<f64>,
    pub(crate) search_query: String,
    pub(crate) query_tokens: Vec<String>,
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
    for tok in &tokens {
        if tok.len() <= 1 || stopwords.contains(tok) {
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

pub(crate) fn rank_market_candidates(intent: &ParsedTradeIntent, markets: &[Value]) -> Vec<Value> {
    let mut ranked: Vec<(f64, Value)> = markets
        .iter()
        .filter_map(|m| {
            let question = m.get("question").and_then(Value::as_str)?;
            let question_tokens = tokenize_for_match(question);
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
            if let Some(ref outcome) = intent.outcome
                && question.to_ascii_lowercase().contains(outcome)
            {
                score += 0.05;
            }
            if let Some(volume) = m
                .get("volumeNum")
                .or_else(|| m.get("volume_num"))
                .and_then(Value::as_f64)
            {
                score += (volume.max(1.0).ln() / 20.0).min(0.15);
            }

            let (yes_price, no_price) = extract_yes_no_prices(m);
            let slug = m.get("slug").and_then(Value::as_str);
            let url = slug.map(|s| format!("https://polymarket.com/market/{}", s));

            Some((
                score,
                json!({
                    "market_id": m.get("id"),
                    "condition_id": m.get("conditionId").or_else(|| m.get("condition_id")),
                    "question": question,
                    "slug": slug,
                    "close_time": m.get("endDate").or_else(|| m.get("end_date")),
                    "yes_price": yes_price,
                    "no_price": no_price,
                    "volume": m.get("volumeNum").or_else(|| m.get("volume_num")),
                    "liquidity": m.get("liquidityNum").or_else(|| m.get("liquidity_num")),
                    "score": score,
                    "url": url,
                }),
            ))
        })
        .collect();

    ranked.sort_by(|a, b| b.0.total_cmp(&a.0));
    ranked.into_iter().map(|(_, v)| v).collect()
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
