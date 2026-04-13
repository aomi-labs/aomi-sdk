use polymarket::testing::place_live_smoke_order;
use serde_json::Value;
use std::env;

fn env_string(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[test]
#[ignore = "places a real live Polymarket order; requires POLYMARKET_PRIVATE_KEY and explicit --ignored execution"]
fn place_live_trade() {
    if env_string("POLYMARKET_PRIVATE_KEY").is_none() {
        eprintln!("Skipping live trade test. POLYMARKET_PRIVATE_KEY is not set.");
        return;
    }

    let result = place_live_smoke_order(None).expect("live Polymarket smoke order failed");
    println!(
        "{}",
        serde_json::to_string_pretty(&result).expect("result should serialize")
    );

    assert_eq!(
        result.get("reached_submission").and_then(Value::as_bool),
        Some(true)
    );

    if result.get("submitted").and_then(Value::as_bool) != Some(true) {
        let error = result
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("missing error");
        assert!(
            error.contains("not enough balance / allowance")
                || error.contains("balance is not enough")
                || error.contains("allowance"),
            "unexpected submission failure: {error}"
        );
    }
}
