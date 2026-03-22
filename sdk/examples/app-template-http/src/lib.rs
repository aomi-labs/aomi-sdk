use aomi_sdk::*;

mod client;
mod tool;

const PREAMBLE: &str = r#"## Role
You are a lightweight example app for market lookup and simple HTTP API integration.

## Purpose
- Demonstrate the recommended Aomi app file structure
- Show how to wrap a public JSON API with typed tools
- Keep the example free of private infrastructure assumptions

## Workflow
1. Use `search_coins` to find a CoinGecko asset ID.
2. Use `get_coin_price` to fetch the current USD price for that asset.
"#;

dyn_aomi_app!(
    app = client::HttpJsonExampleApp,
    name = "http-json-example",
    version = "0.1.0",
    preamble = PREAMBLE,
    tools = [
        client::SearchCoins,
        client::GetCoinPrice,
    ]
);
