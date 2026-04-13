use crate::client::{Market, extract_yes_no_prices};
use serde::Serialize;
use std::collections::HashSet;

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
    for token in tokens {
        if token.len() <= 1 || stopwords.contains(&token) {
            continue;
        }
        query_tokens.push(token.to_string());
    }

    if query_tokens.is_empty() {
        return Err("Unable to parse request into a searchable market query".to_string());
    }

    Ok(ParsedTradeIntent {
        action,
        outcome,
        year,
        size_usd,
        search_query: query_tokens.join(" "),
        query_tokens,
    })
}

pub(crate) fn rank_market_candidates(
    intent: &ParsedTradeIntent,
    markets: &[Market],
) -> Vec<RankedMarketCandidate> {
    let mut ranked: Vec<RankedMarketCandidate> = markets
        .iter()
        .filter_map(|market| {
            let question = market.question.clone()?;
            let overlap = token_overlap_ratio(&intent.query_tokens, &tokenize_for_match(&question));
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
            if let Some(volume) = market.volume_num {
                score += (volume.max(1.0).ln() / 20.0).min(0.15);
            }

            let (yes_price, no_price) = extract_yes_no_prices(market);
            Some(RankedMarketCandidate {
                market_id: market.id.clone(),
                condition_id: market.condition_id.clone(),
                question: Some(question),
                slug: market.slug.clone(),
                close_time: market.end_date.clone(),
                yes_price,
                no_price,
                volume: market.volume_num,
                liquidity: market.liquidity_num,
                score,
                url: market
                    .slug
                    .as_ref()
                    .map(|slug| format!("https://polymarket.com/market/{slug}")),
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
        .filter(|token| token.len() > 1)
        .map(str::to_string)
        .collect()
}

pub(crate) fn token_overlap_ratio(query_tokens: &[String], question_tokens: &[String]) -> f64 {
    if query_tokens.is_empty() || question_tokens.is_empty() {
        return 0.0;
    }
    let question_set: HashSet<&str> = question_tokens.iter().map(String::as_str).collect();
    let matches = query_tokens
        .iter()
        .filter(|query| question_set.contains(query.as_str()))
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
            } else {zx
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
            && let Ok(value) = num.parse::<f64>()
        {
            return Some(value);
        }
    }
    None
}
