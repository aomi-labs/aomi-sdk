use crate::client::{Market, TOKIO_RT};
use serde_json::{Value, json};
use std::collections::HashMap;

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
        let normalized = outcome.to_ascii_lowercase();
        if normalized == "yes" {
            yes_price = price;
        } else if normalized == "no" {
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
            let mapped = map_token_ids_by_outcomes(market.outcomes.as_ref(), &values);
            if mapped.0.is_some() || mapped.1.is_some() {
                return mapped;
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
            .map(str::to_string);
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
            .map(str::to_string);
    }

    (yes, no)
}

pub(crate) fn fetch_clob_outcome_token_ids(
    condition_id: &str,
) -> Result<(Option<String>, Option<String>, Option<Value>), String> {
    let market = TOKIO_RT
        .block_on(polymarket_client_sdk::clob::Client::default().market(condition_id))
        .map_err(|e| format!("failed to fetch CLOB market for {condition_id}: {e}"))?;

    let mut yes_token_id = None;
    let mut no_token_id = None;
    let tokens = market
        .tokens
        .iter()
        .map(|token| {
            let token_id = token.token_id.to_string();
            let outcome = token.outcome.trim().to_ascii_lowercase();
            if outcome == "yes" && yes_token_id.is_none() {
                yes_token_id = Some(token_id.clone());
            } else if outcome == "no" && no_token_id.is_none() {
                no_token_id = Some(token_id.clone());
            }

            json!({
                "token_id": token.token_id.to_string(),
                "outcome": token.outcome,
                "price": token.price.to_string(),
            })
        })
        .collect::<Vec<_>>();

    Ok((yes_token_id, no_token_id, Some(Value::Array(tokens))))
}
