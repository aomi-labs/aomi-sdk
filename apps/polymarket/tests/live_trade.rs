use polymarket::testing::{LiveTradeRequest, place_live_order};
use serde_json::Value;
use std::env;

fn env_string(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn env_f64(name: &str) -> Result<Option<f64>, String> {
    env_string(name)
        .map(|raw| {
            raw.parse::<f64>()
                .map_err(|err| format!("{name} must be a number, got `{raw}`: {err}"))
        })
        .transpose()
}

fn env_bool(name: &str) -> Option<bool> {
    env_string(name)
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

#[test]
#[ignore = "places a real live Polymarket order; requires explicit confirmation env vars"]
fn place_live_trade() {
    if env_string("POLYMARKET_LIVE_TEST_CONFIRM").as_deref() != Some("YES_I_UNDERSTAND") {
        eprintln!(
            "Skipping live trade test. Set POLYMARKET_LIVE_TEST_CONFIRM=YES_I_UNDERSTAND to enable."
        );
        return;
    }

    if env_string("POLYMARKET_PRIVATE_KEY").is_none() {
        eprintln!("Skipping live trade test. POLYMARKET_PRIVATE_KEY is not set.");
        return;
    }

    let Some(market_id_or_slug) = env_string("POLYMARKET_TEST_MARKET") else {
        eprintln!("Skipping live trade test. POLYMARKET_TEST_MARKET is not set.");
        return;
    };
    let Some(outcome) = env_string("POLYMARKET_TEST_OUTCOME") else {
        eprintln!("Skipping live trade test. POLYMARKET_TEST_OUTCOME is not set.");
        return;
    };

    let size_usd =
        env_f64("POLYMARKET_TEST_AMOUNT_USDC").expect("invalid POLYMARKET_TEST_AMOUNT_USDC");
    let shares = env_f64("POLYMARKET_TEST_SHARES").expect("invalid POLYMARKET_TEST_SHARES");
    if size_usd.is_none() && shares.is_none() {
        eprintln!(
            "Skipping live trade test. Set POLYMARKET_TEST_AMOUNT_USDC or POLYMARKET_TEST_SHARES."
        );
        return;
    }

    let request = LiveTradeRequest {
        market_id_or_slug,
        outcome,
        side: env_string("POLYMARKET_TEST_SIDE").or_else(|| Some("BUY".to_string())),
        size_usd,
        shares,
        limit_price: env_f64("POLYMARKET_TEST_LIMIT_PRICE")
            .expect("invalid POLYMARKET_TEST_LIMIT_PRICE"),
        order_type: env_string("POLYMARKET_TEST_ORDER_TYPE"),
        post_only: env_bool("POLYMARKET_TEST_POST_ONLY"),
        signature_type: env_string("POLYMARKET_TEST_SIGNATURE_TYPE"),
        funder: env_string("POLYMARKET_TEST_FUNDER"),
        private_key: None,
    };

    let result = place_live_order(request).expect("live Polymarket order submission failed");
    println!(
        "{}",
        serde_json::to_string_pretty(&result).expect("result should serialize")
    );

    assert_eq!(result.get("submitted").and_then(Value::as_bool), Some(true));
}
