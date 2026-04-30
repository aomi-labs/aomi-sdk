use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct ManifoldApp;

pub(crate) use crate::tool::*;

pub(crate) const MANIFOLD_API_URL: &str = "https://api.manifold.markets/v0";

/// Shared HTTP helpers for Manifold Markets API.
pub(crate) struct ManifoldClient {
    pub(crate) http: reqwest::blocking::Client,
}

impl ManifoldClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[manifold] failed to build HTTP client: {e}"))?;
        Ok(Self { http })
    }

    /// Public GET request (no authentication required).
    pub(crate) fn get(&self, path: &str, op: &str) -> Result<Value, String> {
        let url = format!("{MANIFOLD_API_URL}{path}");
        let response = self
            .http
            .get(&url)
            .send()
            .map_err(|e| format!("[manifold] {op} failed: {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[manifold] {op} failed: {status} {body}"));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[manifold] {op} decode failed: {e}; body: {body}"))
    }

    /// Authenticated POST request (requires API key).
    pub(crate) fn post<B: Serialize>(
        &self,
        path: &str,
        api_key: &str,
        body: &B,
        op: &str,
    ) -> Result<Value, String> {
        let url = format!("{MANIFOLD_API_URL}{path}");
        let response = self
            .http
            .post(&url)
            .header("Authorization", format!("Key {api_key}"))
            .json(body)
            .send()
            .map_err(|e| format!("[manifold] {op} failed: {e}"))?;

        let status = response.status();
        let resp_body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[manifold] {op} failed: {status} {resp_body}"));
        }

        serde_json::from_str::<Value>(&resp_body)
            .map_err(|e| format!("[manifold] {op} decode failed: {e}; body: {resp_body}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_market_and_bet_workflow() {
        let client = ManifoldClient::new().expect("build HTTP client");

        // Search markets and pick the first BINARY result.
        let search = client
            .get("/search-markets?term=AI&filter=open", "search_markets")
            .expect("search markets for AI");
        let results = search.as_array().expect("search returns an array");
        assert!(!results.is_empty(), "search for AI should return results");

        let first = results
            .iter()
            .find(|m| m.get("outcomeType").and_then(|v| v.as_str()) == Some("BINARY"))
            .expect("at least one BINARY market in AI search results");
        let market_id = first
            .get("id")
            .and_then(|v| v.as_str())
            .expect("first result has an id");

        // Get market detail and verify key fields.
        let detail = client
            .get(&format!("/market/{market_id}"), "get_market")
            .expect("get market detail");
        assert!(detail.get("probability").is_some());
        assert!(detail.get("volume").is_some());
        assert!(detail.get("question").is_some());

        // Get positions for the same market.
        let positions = client
            .get(
                &format!("/market/{market_id}/positions"),
                "get_market_positions",
            )
            .expect("get market positions");
        assert!(positions.is_array());
    }

    #[test]
    fn bet_against_overpriced_workflow() {
        let client = ManifoldClient::new().expect("build HTTP client");

        let list = client
            .get("/markets?sort=created-time&limit=10", "list_markets")
            .expect("list newest markets");
        let markets = list.as_array().expect("list returns an array");
        assert!(!markets.is_empty(), "should get at least one market");
        for m in markets.iter() {
            assert!(m.get("id").is_some(), "market missing id");
            assert!(m.get("question").is_some(), "market missing question");
        }

        let binary_markets: Vec<&Value> = markets
            .iter()
            .filter(|m| m.get("outcomeType").and_then(|v| v.as_str()) == Some("BINARY"))
            .collect();

        let high_prob_market = binary_markets
            .iter()
            .find(|m| {
                m.get("probability")
                    .and_then(|p| p.as_f64())
                    .map_or(false, |p| p > 0.80)
            })
            .copied()
            .or(binary_markets.first().copied())
            .unwrap_or(&markets[0]);

        let market_id = high_prob_market
            .get("id")
            .and_then(|v| v.as_str())
            .expect("selected market has an id");

        let detail = client
            .get(&format!("/market/{market_id}"), "get_market")
            .expect("get market detail");
        let positions = client
            .get(
                &format!("/market/{market_id}/positions"),
                "get_market_positions",
            )
            .expect("get market positions");

        let probability = detail
            .get("probability")
            .and_then(|v| v.as_f64())
            .expect("detail should have numeric probability");
        assert!((0.0..=1.0).contains(&probability));
        let liquidity = detail
            .get("totalLiquidity")
            .and_then(|v| v.as_f64())
            .expect("detail should have numeric totalLiquidity");
        assert!(liquidity >= 0.0);
        assert!(positions.is_array());
    }
}
