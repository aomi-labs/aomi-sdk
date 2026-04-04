use chrono::Local;
use serde_json::{Value, json};
use std::time::Duration;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn build_preamble() -> String {
    let now = Local::now();
    format!(
        r#"## Role
You specialize in Kalshi prediction markets via the Simmer SDK.

## Current Date
Today is {} ({}). Use this exact date when interpreting relative terms like 'today', 'tomorrow', and 'yesterday'.

## Simmer SDK
Simmer SDK (simmer.markets) provides the trading API used by this app.

Venues: 'sim' = sandbox ($SIM, no real money, no KYC). 'kalshi' = live Kalshi trading after the agent is claimed and the user's Kalshi wallet/account setup is complete. Default to sim unless the user explicitly wants a live Kalshi trade.

Setup: simmer_register -> get api_key + claim_url -> user runs /apikey simmer <key> -> user visits claim_url to complete identity verification and link the agent. When presenting the claim link, always tell the user: 'Visit this link to verify your identity and claim your agent. This is where Simmer handles account linking and unlocks live Kalshi trading.'

Discovery flow: search_simmer_markets returns importable Kalshi markets, not Simmer UUIDs. Before trading, call import_kalshi_market with the Kalshi URL. That returns the Simmer market_id UUID required for fetch_simmer_market_context and simmer_place_order.

Trading flow: search_simmer_markets -> import_kalshi_market -> fetch_simmer_market_context -> simmer_place_order. Always check context warnings before trading. The reasoning field is public on the user's Simmer profile, so write a real thesis.

Live Kalshi trading uses venue='kalshi' and currently requires the user's live wallet/account setup through Simmer. Sandbox trading uses venue='sim'.

Compliance: Aomi is only an interface. We do not hold funds. KYC, custody, and compliance are handled by Simmer and the underlying Kalshi integration. Users are responsible for ensuring prediction market trading is legal in their jurisdiction.

IMPORTANT -- show this disclaimer on registration (before claim link), on first live Kalshi trade (venue=kalshi), and in /apikey simmer response:
'Aomi is an interface to Simmer (simmer.markets) -- we do not hold your funds. KYC and compliance are handled by Simmer and the underlying Kalshi integration. You are responsible for ensuring prediction market trading is legal in your jurisdiction. By claiming your agent you agree to Simmer ToS.'
"#,
        now.format("%Y-%m-%d"),
        now.format("%Z")
    )
}

#[derive(Clone, Default)]
pub(crate) struct KalshiApp;

pub(crate) use crate::tool::*;

pub(crate) const SIMMER_API_URL: &str = "https://api.simmer.markets";

#[derive(Clone)]
pub(crate) struct SimmerClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_key: String,
}

impl SimmerClient {
    pub(crate) fn new(api_key: &str) -> Result<Self, String> {
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

    #[cfg(test)]
    pub(crate) fn health_check() -> Result<Value, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        let req = http.get(format!("{}/api/sdk/health", SIMMER_API_URL));
        Self::send_json(req, "health")
    }

    pub(crate) fn get_agent_status(&self) -> Result<Value, String> {
        let url = format!("{SIMMER_API_URL}/api/sdk/agents/me");
        let req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        Self::send_json(req, "get_agent_status")
    }

    pub(crate) fn get_briefing(&self, since: Option<&str>) -> Result<Value, String> {
        let url = format!("{SIMMER_API_URL}/api/sdk/briefing");
        let mut req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        if let Some(since) = since {
            req = req.query(&[("since", since)]);
        }
        Self::send_json(req, "get_briefing")
    }

    pub(crate) fn get_market_context(&self, market_id: &str) -> Result<Value, String> {
        let url = format!("{SIMMER_API_URL}/api/sdk/context/{market_id}");
        let req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        Self::send_json(req, "get_market_context")
    }

    pub(crate) fn trade(&self, body: Value) -> Result<Value, String> {
        let url = format!("{SIMMER_API_URL}/api/sdk/trade");
        let req = self
            .http
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&body);
        Self::send_json(req, "trade")
    }

    pub(crate) fn get_positions(&self, venue: Option<&str>) -> Result<Value, String> {
        let url = format!("{SIMMER_API_URL}/api/sdk/positions");
        let mut req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        if let Some(venue) = venue {
            req = req.query(&[("venue", venue)]);
        }
        Self::send_json(req, "get_positions")
    }

    pub(crate) fn get_portfolio(&self) -> Result<Value, String> {
        let url = format!("{SIMMER_API_URL}/api/sdk/portfolio");
        let req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header());
        Self::send_json(req, "get_portfolio")
    }

    pub(crate) fn list_importable_kalshi_markets(
        &self,
        query: Option<&str>,
        limit: Option<u32>,
        min_volume: Option<f64>,
    ) -> Result<Value, String> {
        let url = format!("{SIMMER_API_URL}/api/sdk/markets/importable");
        let mut req = self
            .http
            .get(&url)
            .header("Authorization", self.auth_header())
            .query(&[("venue", "kalshi")]);
        if let Some(query) = query {
            req = req.query(&[("q", query)]);
        }
        if let Some(limit) = limit {
            req = req.query(&[("limit", limit.to_string())]);
        }
        if let Some(min_volume) = min_volume {
            req = req.query(&[("min_volume", min_volume.to_string())]);
        }
        Self::send_json(req, "list_importable_kalshi_markets")
    }

    pub(crate) fn import_kalshi_market(&self, kalshi_url: &str) -> Result<Value, String> {
        let url = format!("{SIMMER_API_URL}/api/sdk/markets/import/kalshi");
        let req = self
            .http
            .post(&url)
            .header("Authorization", self.auth_header())
            .json(&json!({ "kalshi_url": kalshi_url }));
        Self::send_json(req, "import_kalshi_market")
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
        .post(format!("{SIMMER_API_URL}/api/sdk/agents/register"))
        .json(&body);
    SimmerClient::send_json(req, "register_agent")
}

pub(crate) fn parse_venue(venue: &str) -> Result<String, String> {
    match venue.to_lowercase().as_str() {
        "sim" | "sandbox" | "simmer" => Ok("sim".to_string()),
        "kalshi" => Ok("kalshi".to_string()),
        other => Err(format!("Unknown venue: {other}. Use sim or kalshi.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_name(prefix: &str) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_millis();
        format!("{prefix}-{now}")
    }

    fn register_temp_agent() -> Value {
        simmer_register_agent(
            &unique_name("codex-kalshi-smoke"),
            Some("temporary smoke test"),
        )
        .expect("agent registration should succeed")
    }

    fn temp_api_key() -> String {
        register_temp_agent()["api_key"]
            .as_str()
            .expect("register response should include api_key")
            .to_string()
    }

    #[test]
    fn simmer_health_smoke() {
        let payload = SimmerClient::health_check().expect("health endpoint should respond");
        assert_eq!(payload.get("status").and_then(Value::as_str), Some("ok"));
    }

    #[test]
    fn register_and_status_smoke() {
        let registered = register_temp_agent();
        let api_key = registered["api_key"]
            .as_str()
            .expect("register response should include api_key");
        let client = SimmerClient::new(api_key).expect("client should build");
        let status = client
            .get_agent_status()
            .expect("agent status request should succeed");

        assert_eq!(
            status.get("status").and_then(Value::as_str),
            Some("unclaimed")
        );
    }

    #[test]
    fn list_importable_kalshi_markets_smoke() {
        let client = SimmerClient::new(&temp_api_key()).expect("client should build");
        let payload = client
            .list_importable_kalshi_markets(None, Some(1), None)
            .expect("importable markets request should succeed");
        let markets = payload["markets"]
            .as_array()
            .expect("markets should be an array");

        assert!(
            !markets.is_empty(),
            "expected at least one importable Kalshi market"
        );
        assert_eq!(
            markets[0].get("venue").and_then(Value::as_str),
            Some("kalshi")
        );
        assert!(
            markets[0].get("url").and_then(Value::as_str).is_some(),
            "expected importable market to include a Kalshi URL"
        );
    }

    #[test]
    fn import_kalshi_market_requires_claimed_agent_smoke() {
        let client = SimmerClient::new(&temp_api_key()).expect("client should build");
        let importables = client
            .list_importable_kalshi_markets(None, Some(1), None)
            .expect("importable markets request should succeed");
        let kalshi_url = importables["markets"][0]["url"]
            .as_str()
            .expect("importable market should include url");

        let err = client
            .import_kalshi_market(kalshi_url)
            .expect_err("fresh agents should not be able to import before claiming");

        assert!(
            err.contains("claimed agent") || err.contains("Claim your agent first"),
            "unexpected import error: {err}"
        );
    }
}
