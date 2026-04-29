use crate::client::*;
use crate::types::{CreateMarketRequest, PlaceBetRequest};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[manifold] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("manifold".to_string()));
            Value::Object(map)
        }
        other => json!({ "source": "manifold", "data": other }),
    })
}

fn resolve_manifold_api_key(api_key: Option<&str>) -> Result<String, String> {
    resolve_secret_value(
        api_key,
        "MANIFOLD_API_KEY",
        "[manifold] missing api_key argument and MANIFOLD_API_KEY environment variable",
    )
}

// ============================================================================
// ListMarkets
// ============================================================================

pub(crate) struct ListMarkets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ListMarketsArgs {
    /// Maximum number of markets to return (default: 20, max: 1000)
    limit: Option<u32>,
    /// Cursor ID for pagination -- return markets before this ID
    before: Option<String>,
    /// Sort order: "newest" or "score"
    sort: Option<String>,
    /// Filter by topic slug(s), comma-separated
    topics: Option<String>,
}

impl DynAomiTool for ListMarkets {
    type App = ManifoldApp;
    type Args = ListMarketsArgs;
    const NAME: &'static str = "list_markets";
    const DESCRIPTION: &'static str = "List prediction markets on Manifold, optionally filtered by topic and sorted by newest or score.";

    fn run(_app: &ManifoldApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let mut query_parts: Vec<String> = Vec::new();
        if let Some(limit) = args.limit {
            query_parts.push(format!("limit={limit}"));
        }
        if let Some(before) = &args.before {
            query_parts.push(format!("before={before}"));
        }
        if let Some(sort) = &args.sort {
            query_parts.push(format!("sort={sort}"));
        }
        if let Some(topics) = &args.topics {
            query_parts.push(format!("topics={topics}"));
        }

        let path = if query_parts.is_empty() {
            "/markets".to_string()
        } else {
            format!("/markets?{}", query_parts.join("&"))
        };

        let markets = ManifoldClient::new()?.get(&path, "list_markets")?;
        let markets_arr = markets.as_array().cloned().unwrap_or_default();

        ok(json!({
            "markets_count": markets_arr.len(),
            "markets": markets_arr.iter().map(|m| json!({
                "id": m.get("id"),
                "question": m.get("question"),
                "url": m.get("url"),
                "probability": m.get("probability"),
                "volume": m.get("volume"),
                "createdTime": m.get("createdTime"),
                "closeTime": m.get("closeTime"),
                "isResolved": m.get("isResolved"),
            })).collect::<Vec<_>>(),
        }))
    }
}

// ============================================================================
// GetMarket
// ============================================================================

pub(crate) struct GetMarket;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMarketArgs {
    /// The market ID or slug to look up
    id: String,
}

impl DynAomiTool for GetMarket {
    type App = ManifoldApp;
    type Args = GetMarketArgs;
    const NAME: &'static str = "get_market";
    const DESCRIPTION: &'static str =
        "Get detailed information about a specific Manifold market by ID or slug.";

    fn run(_app: &ManifoldApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let path = format!("/market/{}", args.id);
        let market = ManifoldClient::new()?.get(&path, "get_market")?;

        ok(json!({
            "id": market.get("id"),
            "question": market.get("question"),
            "description": market.get("textDescription"),
            "url": market.get("url"),
            "creatorName": market.get("creatorName"),
            "probability": market.get("probability"),
            "volume": market.get("volume"),
            "totalLiquidity": market.get("totalLiquidity"),
            "createdTime": market.get("createdTime"),
            "closeTime": market.get("closeTime"),
            "isResolved": market.get("isResolved"),
            "resolution": market.get("resolution"),
            "mechanism": market.get("mechanism"),
            "outcomeType": market.get("outcomeType"),
        }))
    }
}

// ============================================================================
// GetMarketPositions
// ============================================================================

pub(crate) struct GetMarketPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMarketPositionsArgs {
    /// The market ID to get positions for
    id: String,
}

impl DynAomiTool for GetMarketPositions {
    type App = ManifoldApp;
    type Args = GetMarketPositionsArgs;
    const NAME: &'static str = "get_market_positions";
    const DESCRIPTION: &'static str = "Get user positions for a specific Manifold market.";

    fn run(_app: &ManifoldApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let path = format!("/market/{}/positions", args.id);
        let positions = ManifoldClient::new()?.get(&path, "get_market_positions")?;
        ok(json!({ "market_id": args.id, "positions": positions }))
    }
}

// ============================================================================
// SearchMarkets
// ============================================================================

pub(crate) struct SearchMarkets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchMarketsArgs {
    /// Search query term
    term: String,
    /// Sort order: "newest", "score", "liquidity", etc.
    sort: Option<String>,
    /// Filter: "open", "closed", "resolved", "all"
    filter: Option<String>,
}

impl DynAomiTool for SearchMarkets {
    type App = ManifoldApp;
    type Args = SearchMarketsArgs;
    const NAME: &'static str = "search_markets";
    const DESCRIPTION: &'static str =
        "Search Manifold prediction markets by keyword with optional sort and filter.";

    fn run(_app: &ManifoldApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let mut query_parts: Vec<String> = vec![format!("term={}", urlencoded(&args.term))];
        if let Some(sort) = &args.sort {
            query_parts.push(format!("sort={sort}"));
        }
        if let Some(filter) = &args.filter {
            query_parts.push(format!("filter={filter}"));
        }

        let path = format!("/search-markets?{}", query_parts.join("&"));
        let results = ManifoldClient::new()?.get(&path, "search_markets")?;
        let markets_arr = results.as_array().cloned().unwrap_or_default();

        ok(json!({
            "query": args.term,
            "results_count": markets_arr.len(),
            "markets": markets_arr.iter().map(|m| json!({
                "id": m.get("id"),
                "question": m.get("question"),
                "url": m.get("url"),
                "probability": m.get("probability"),
                "volume": m.get("volume"),
                "isResolved": m.get("isResolved"),
            })).collect::<Vec<_>>(),
        }))
    }
}

// ============================================================================
// PlaceBet
// ============================================================================

pub(crate) struct PlaceBet;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceBetArgs {
    /// Manifold API key for authentication
    api_key: Option<String>,
    /// The contract/market ID to bet on
    contract_id: String,
    /// Amount of mana (M$) to bet
    amount: f64,
    /// Outcome to bet on: "YES" or "NO"
    outcome: String,
}

impl DynAomiTool for PlaceBet {
    type App = ManifoldApp;
    type Args = PlaceBetArgs;
    const NAME: &'static str = "place_bet";
    const DESCRIPTION: &'static str =
        "Place a YES or NO bet on a Manifold binary market. Requires a Manifold API key.";

    fn run(_app: &ManifoldApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let outcome = args.outcome.to_uppercase();
        if outcome != "YES" && outcome != "NO" {
            return Err("outcome must be YES or NO".to_string());
        }
        if args.amount <= 0.0 {
            return Err("amount must be greater than 0".to_string());
        }

        let api_key = resolve_manifold_api_key(args.api_key.as_deref())?;
        let body = PlaceBetRequest {
            contract_id: &args.contract_id,
            amount: args.amount,
            outcome: &outcome,
        };

        let result = ManifoldClient::new()?.post("/bet", &api_key, &body, "place_bet")?;

        ok(json!({
            "status": "success",
            "betId": result.get("betId").or_else(|| result.get("id")),
            "contractId": args.contract_id,
            "amount": args.amount,
            "outcome": outcome,
            "result": result,
        }))
    }
}

// ============================================================================
// CreateMarket
// ============================================================================

pub(crate) struct CreateMarket;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CreateMarketArgs {
    /// Manifold API key for authentication
    api_key: Option<String>,
    /// The question for the market (e.g., "Will X happen by Y date?")
    question: String,
    /// Market type (use "BINARY" for yes/no markets)
    #[serde(rename = "type")]
    market_type: Option<String>,
    /// Close time as a Unix timestamp in milliseconds
    close_time: Option<u64>,
    /// Initial probability for binary markets (1-99)
    initial_prob: Option<u32>,
}

impl DynAomiTool for CreateMarket {
    type App = ManifoldApp;
    type Args = CreateMarketArgs;
    const NAME: &'static str = "create_market";
    const DESCRIPTION: &'static str =
        "Create a new prediction market on Manifold. Requires a Manifold API key.";

    fn run(_app: &ManifoldApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let market_type = args.market_type.as_deref().unwrap_or("BINARY");
        let initial_prob = args.initial_prob.unwrap_or(50);

        if !(1..=99).contains(&initial_prob) {
            return Err("initial_prob must be between 1 and 99".to_string());
        }

        let api_key = resolve_manifold_api_key(args.api_key.as_deref())?;
        let body = CreateMarketRequest {
            outcome_type: market_type,
            question: &args.question,
            initial_prob,
            close_time: args.close_time,
        };

        let result = ManifoldClient::new()?.post("/market", &api_key, &body, "create_market")?;

        ok(json!({
            "status": "created",
            "id": result.get("id"),
            "question": args.question,
            "url": result.get("url"),
            "slug": result.get("slug"),
            "result": result,
        }))
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Minimal percent-encoding for query string values.
fn urlencoded(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            ' ' => result.push_str("%20"),
            '&' => result.push_str("%26"),
            '=' => result.push_str("%3D"),
            '+' => result.push_str("%2B"),
            '#' => result.push_str("%23"),
            '?' => result.push_str("%3F"),
            _ => result.push(c),
        }
    }
    result
}
