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
    pub(crate) fn post(
        &self,
        path: &str,
        api_key: &str,
        body: &Value,
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
        // Story: Create a prediction market and take an initial position.
        // We exercise every public read endpoint in the sequence a real
        // create-then-bet workflow would follow, stopping short of the
        // authenticated write calls.

        let client = ManifoldClient::new().expect("build HTTP client");
        println!("[step 1] Searching markets for 'AI' (open, BINARY)...");

        // 1. Search markets for "AI" and pick the first BINARY result
        //    (only binary markets carry a top-level probability field).
        let search = client
            .get("/search-markets?term=AI&filter=open", "search_markets")
            .expect("search markets for AI");
        let results = search.as_array().expect("search returns an array");
        assert!(!results.is_empty(), "search for AI should return results");
        println!("[step 1] Search returned {} results", results.len());

        let first = results
            .iter()
            .find(|m| m.get("outcomeType").and_then(|v| v.as_str()) == Some("BINARY"))
            .expect("at least one BINARY market in AI search results");

        let market_id = first
            .get("id")
            .and_then(|v| v.as_str())
            .expect("first result has an id");

        let first_title = first
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        println!("[step 1] Selected BINARY market: id={market_id}, title=\"{first_title}\"");

        // 2. Get market detail and verify key fields.
        println!("[step 2] Fetching market detail for {market_id}...");
        let detail_path = format!("/market/{market_id}");
        let detail = client
            .get(&detail_path, "get_market")
            .expect("get market detail");

        assert!(
            detail.get("probability").is_some(),
            "market detail should include probability"
        );
        assert!(
            detail.get("volume").is_some(),
            "market detail should include volume"
        );
        assert!(
            detail.get("question").is_some(),
            "market detail should include question"
        );
        println!(
            "[step 2] Market detail — probability: {}, volume: {}, question: \"{}\"",
            detail.get("probability").unwrap(),
            detail.get("volume").unwrap(),
            detail
                .get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>")
        );

        // 3. Get positions for the same market.
        println!("[step 3] Fetching positions for market {market_id}...");
        let positions_path = format!("/market/{market_id}/positions");
        let positions = client
            .get(&positions_path, "get_market_positions")
            .expect("get market positions");

        // Positions endpoint returns an array (possibly empty for low-activity markets).
        assert!(
            positions.is_array(),
            "positions response should be an array"
        );
        let position_count = positions.as_array().map_or(0, |a| a.len());
        println!("[step 3] Positions count: {position_count}");

        // 4. (Skipped) create_market and place_bet require an API key.
        println!("[step 4] Skipped — create_market and place_bet require an API key");

        // 5. Verify we collected all the data a real workflow would need
        //    before calling the authenticated endpoints.
        let probability = detail.get("probability").expect("probability present");
        let volume = detail.get("volume").expect("volume present");
        let question = detail
            .get("question")
            .and_then(|v| v.as_str())
            .expect("question present and is a string");

        assert!(
            probability.is_number(),
            "probability should be a number, got {probability}"
        );
        assert!(
            volume.is_number(),
            "volume should be a number, got {volume}"
        );
        assert!(!question.is_empty(), "question should be non-empty");
        println!(
            "[step 5] Workflow summary — question: \"{question}\", probability: {probability}, volume: {volume}, positions: {position_count}"
        );
    }

    #[test]
    fn bet_against_overpriced_workflow() {
        // Story: Bet against an overpriced market.
        // Fetch the newest markets, find one with a high probability,
        // pull its detail and positions, then assert we have everything
        // needed to decide on a contrarian NO bet.

        let client = ManifoldClient::new().expect("build HTTP client");
        println!("[step 1] Listing newest 10 markets...");

        // 1. List newest markets (limit 10).
        let list = client
            .get("/markets?sort=created-time&limit=10", "list_markets")
            .expect("list newest markets");
        let markets = list.as_array().expect("list returns an array");
        assert!(!markets.is_empty(), "should get at least one market");
        println!("[step 1] Retrieved {} markets", markets.len());

        // Every entry should carry the fields the tool exposes.
        for (i, m) in markets.iter().enumerate() {
            assert!(m.get("id").is_some(), "market missing id");
            assert!(m.get("question").is_some(), "market missing question");
            let title = m
                .get("question")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
            let prob = m.get("probability").and_then(|v| v.as_f64());
            println!("[step 1]   market[{i}]: \"{title}\" (probability: {prob:?})");
        }

        // 2. Pick a BINARY market with probability > 80 %.
        //    Not every batch is guaranteed to contain one, so fall back to
        //    the first BINARY market if none qualifies.
        let binary_markets: Vec<&Value> = markets
            .iter()
            .filter(|m| {
                m.get("outcomeType")
                    .and_then(|v| v.as_str())
                    .map_or(false, |t| t == "BINARY")
            })
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

        let selected_title = high_prob_market
            .get("question")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>");
        let selected_prob = high_prob_market.get("probability").and_then(|v| v.as_f64());
        println!(
            "[step 2] Selected market: id={market_id}, title=\"{selected_title}\", probability: {selected_prob:?}"
        );

        println!("[step 2] Fetching market detail for {market_id}...");
        let detail_path = format!("/market/{market_id}");
        let detail = client
            .get(&detail_path, "get_market")
            .expect("get market detail");

        // 3. Get positions for the market.
        println!("[step 3] Fetching positions for market {market_id}...");
        let positions_path = format!("/market/{market_id}/positions");
        let positions = client
            .get(&positions_path, "get_market_positions")
            .expect("get market positions");

        assert!(
            positions.is_array(),
            "positions response should be an array"
        );
        let position_count = positions.as_array().map_or(0, |a| a.len());
        println!("[step 3] Positions count: {position_count}");

        // 4. (Skipped) place_bet requires an API key.
        println!("[step 4] Skipped — place_bet requires an API key");

        // 5. Assert we have probability, liquidity, and position data to
        //    make a bet decision.
        let probability = detail
            .get("probability")
            .and_then(|v| v.as_f64())
            .expect("detail should have numeric probability");
        assert!(
            probability >= 0.0 && probability <= 1.0,
            "probability should be between 0 and 1, got {probability}"
        );

        let liquidity = detail
            .get("totalLiquidity")
            .and_then(|v| v.as_f64())
            .expect("detail should have numeric totalLiquidity");
        assert!(
            liquidity >= 0.0,
            "liquidity should be non-negative, got {liquidity}"
        );

        let positions_arr = positions.as_array().expect("positions is an array");
        // We verified the structure; an empty array is acceptable for new markets.
        assert!(
            positions_arr.len() < 100_000,
            "positions array should be a reasonable size"
        );
        println!(
            "[step 5] Workflow summary — title: \"{selected_title}\", probability: {probability:.4}, liquidity: {liquidity:.2}, positions: {position_count}"
        );
    }
}
