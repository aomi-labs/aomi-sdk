use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct KaitoApp;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

// ============================================================================
// Kaito API Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_KAITO_API: &str = "https://api.kaito.ai/api/v1";

#[derive(Clone)]
pub(crate) struct KaitoClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_endpoint: String,
}

impl KaitoClient {
    pub(crate) fn new(api_key: &str) -> Result<Self, String> {
        let mut headers = reqwest::header::HeaderMap::new();
        let val = reqwest::header::HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|e| format!("[kaito] invalid api key header: {e}"))?;
        headers.insert(reqwest::header::AUTHORIZATION, val);

        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .map_err(|e| format!("[kaito] failed to build HTTP client: {e}"))?;

        Ok(Self {
            http,
            api_endpoint: std::env::var("KAITO_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_KAITO_API.to_string()),
        })
    }

    fn get_json(&self, url: &str, op: &str) -> Result<Value, String> {
        let response = self
            .http
            .get(url)
            .send()
            .map_err(|e| format!("[kaito] {op} failed: request error ({url}): {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[kaito] {op} failed: {status} ({url}): {body}"));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[kaito] {op} failed: decode error ({url}): {e}; body: {body}"))
    }

    fn with_source(value: Value) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert("source".to_string(), Value::String("kaito".to_string()));
                Value::Object(map)
            }
            other => serde_json::json!({
                "source": "kaito",
                "data": other,
            }),
        }
    }

    /// Semantic search across Web3 corpus.
    pub(crate) fn search(
        &self,
        query: &str,
        limit: Option<u32>,
        source_type: Option<&str>,
    ) -> Result<Value, String> {
        let mut request = self.http.get(format!("{}/search", self.api_endpoint));
        request = request.query(&[("q", query)]);
        if let Some(l) = limit {
            request = request.query(&[("limit", l.to_string())]);
        }
        if let Some(st) = source_type {
            request = request.query(&[("source_type", st.to_string())]);
        }
        let response = request
            .send()
            .map_err(|e| format!("[kaito] search failed: request error: {e}"))?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[kaito] search failed: {status}: {body}"));
        }
        let value: Value = serde_json::from_str(&body)
            .map_err(|e| format!("[kaito] search failed: decode error: {e}; body: {body}"))?;
        Ok(Self::with_source(value))
    }

    /// Get trending topics / narratives.
    pub(crate) fn get_trending(&self, limit: Option<u32>) -> Result<Value, String> {
        let url = format!("{}/trending", self.api_endpoint);
        let mut request = self.http.get(&url);
        if let Some(l) = limit {
            request = request.query(&[("limit", l.to_string())]);
        }
        let response = request
            .send()
            .map_err(|e| format!("[kaito] get_trending failed: request error ({url}): {e}"))?;
        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "[kaito] get_trending failed: {status} ({url}): {body}"
            ));
        }
        let value: Value = serde_json::from_str(&body).map_err(|e| {
            format!("[kaito] get_trending failed: decode error ({url}): {e}; body: {body}")
        })?;
        Ok(Self::with_source(value))
    }

    /// Get token attention / mindshare metrics.
    pub(crate) fn get_mindshare(&self, token: &str) -> Result<Value, String> {
        let url = format!("{}/mindshare/{}", self.api_endpoint, token);
        let value = self.get_json(&url, "get_mindshare")?;
        Ok(Self::with_source(value))
    }
}

// ============================================================================
// Tool structs and arg definitions
// ============================================================================

pub(crate) struct KaitoSearch;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct KaitoSearchArgs {
    /// Kaito API key for authentication.
    pub(crate) api_key: String,
    /// Search query string for semantic search across Web3 sources (Twitter, Discord, Telegram, governance forums, Farcaster, podcasts, etc.).
    pub(crate) query: String,
    /// Maximum number of results to return. Optional.
    #[serde(default)]
    pub(crate) limit: Option<u32>,
    /// Filter by source type (e.g. "twitter", "discord", "telegram", "farcaster", "governance"). Optional.
    #[serde(default)]
    pub(crate) source_type: Option<String>,
}

pub(crate) struct KaitoGetTrending;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct KaitoGetTrendingArgs {
    /// Kaito API key for authentication.
    pub(crate) api_key: String,
    /// Maximum number of trending topics to return. Optional.
    #[serde(default)]
    pub(crate) limit: Option<u32>,
}

pub(crate) struct KaitoGetMindshare;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct KaitoGetMindshareArgs {
    /// Kaito API key for authentication.
    pub(crate) api_key: String,
    /// Token symbol or name to get attention/mindshare metrics for (e.g. "BTC", "ETH", "SOL").
    pub(crate) token: String,
}

// ============================================================================
// Integration tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn client_or_skip() -> Option<KaitoClient> {
        match std::env::var("KAITO_API_KEY") {
            Ok(key) if !key.is_empty() => {
                Some(KaitoClient::new(&key).expect("failed to build KaitoClient"))
            }
            _ => {
                println!("KAITO_API_KEY not set, skipping test");
                None
            }
        }
    }

    #[test]
    fn narrative_discovery_workflow() {
        // Story: "Find narratives gaining momentum so I can position early"
        let client = match client_or_skip() {
            Some(c) => c,
            None => return,
        };

        // 1. Get trending topics/narratives
        println!("=== Step 1: Fetching trending topics/narratives (limit=10) ===");
        let trending = client
            .get_trending(Some(10))
            .expect("get_trending should return trending narratives");
        println!(
            "Trending response: {}",
            serde_json::to_string_pretty(&trending).unwrap_or_default()
        );
        assert!(!trending.is_null(), "trending response must not be null");

        // Extract a keyword from the trending response to use in the next step.
        // The response is wrapped with `source`, so look for an array in the data.
        let keyword = trending
            .as_object()
            .and_then(|obj| {
                // Try common shapes: look for an array field that holds topic entries
                obj.iter()
                    .filter(|(k, _)| *k != "source")
                    .find_map(|(_, v)| {
                        if let Some(arr) = v.as_array() {
                            arr.first().and_then(|entry| {
                                entry
                                    .get("name")
                                    .or_else(|| entry.get("topic"))
                                    .or_else(|| entry.get("keyword"))
                                    .or_else(|| entry.get("title"))
                                    .and_then(|v| v.as_str())
                                    .map(String::from)
                            })
                        } else {
                            None
                        }
                    })
            })
            .unwrap_or_else(|| "DeFi".to_string());
        println!("Extracted keyword from trending data: {keyword:?}");

        // 2. Search for the top narrative keyword
        println!("=== Step 2: Searching for trending keyword '{keyword}' (limit=5) ===");
        let search_results = client
            .search(&keyword, Some(5), None)
            .expect("search should return results for the trending keyword");
        println!(
            "Search results: {}",
            serde_json::to_string_pretty(&search_results).unwrap_or_default()
        );
        assert!(!search_results.is_null(), "search results must not be null");

        // 3. Get mindshare for a major token
        println!("=== Step 3: Fetching mindshare metrics for ETH ===");
        let mindshare = client
            .get_mindshare("ETH")
            .expect("get_mindshare should return attention metrics for ETH");
        println!(
            "ETH mindshare: {}",
            serde_json::to_string_pretty(&mindshare).unwrap_or_default()
        );
        assert!(!mindshare.is_null(), "mindshare response must not be null");

        // 4. Assert we have enough data to identify actionable narratives
        println!("=== Step 4: Validating all responses are actionable JSON objects ===");
        println!(
            "trending is_object={}, search_results is_object={}, mindshare is_object={}",
            trending.is_object(),
            search_results.is_object(),
            mindshare.is_object()
        );
        assert!(
            trending.is_object() && search_results.is_object() && mindshare.is_object(),
            "all three responses should be valid JSON objects providing actionable narrative data"
        );
        println!("=== Narrative discovery workflow complete ===");
    }

    #[test]
    fn portfolio_sentiment_monitor_workflow() {
        // Story: "Monitor sentiment around my portfolio tokens"
        let client = match client_or_skip() {
            Some(c) => c,
            None => return,
        };

        // 1. Get mindshare for BTC
        println!("=== Step 1: Fetching mindshare metrics for BTC ===");
        let btc_mindshare = client
            .get_mindshare("BTC")
            .expect("get_mindshare should return metrics for BTC");
        println!(
            "BTC mindshare: {}",
            serde_json::to_string_pretty(&btc_mindshare).unwrap_or_default()
        );
        assert!(
            !btc_mindshare.is_null(),
            "BTC mindshare response must not be null"
        );

        // 2. Get mindshare for ETH
        println!("=== Step 2: Fetching mindshare metrics for ETH ===");
        let eth_mindshare = client
            .get_mindshare("ETH")
            .expect("get_mindshare should return metrics for ETH");
        println!(
            "ETH mindshare: {}",
            serde_json::to_string_pretty(&eth_mindshare).unwrap_or_default()
        );
        assert!(
            !eth_mindshare.is_null(),
            "ETH mindshare response must not be null"
        );

        // 3. Search for recent mentions of ETH
        println!("=== Step 3: Searching for recent ETH mentions (limit=10) ===");
        let eth_mentions = client
            .search("ETH", Some(10), None)
            .expect("search should return recent mentions of ETH");
        println!(
            "ETH mentions: {}",
            serde_json::to_string_pretty(&eth_mentions).unwrap_or_default()
        );
        assert!(
            !eth_mentions.is_null(),
            "ETH mentions search must not be null"
        );

        // 4. Assert we can compare mindshare trends across tokens
        println!("=== Step 4: Comparing mindshare trends across BTC and ETH ===");
        println!(
            "BTC is_object={}, ETH is_object={}",
            btc_mindshare.is_object(),
            eth_mindshare.is_object()
        );
        println!(
            "BTC has source={}, ETH has source={}",
            btc_mindshare.get("source").is_some(),
            eth_mindshare.get("source").is_some()
        );
        assert!(
            btc_mindshare.is_object() && eth_mindshare.is_object(),
            "both mindshare responses should be objects so we can compare trends"
        );
        assert!(
            btc_mindshare.get("source").is_some() && eth_mindshare.get("source").is_some(),
            "mindshare responses should carry source attribution for comparison"
        );
        println!("=== Portfolio sentiment monitor workflow complete ===");
    }
}
