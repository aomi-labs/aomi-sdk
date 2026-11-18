//! A keyless reader: current token prices from DefiLlama.

use aomi_sdk::{Ctx, Outcome, Result, Tool, err, http};
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PriceArgs {
    /// DefiLlama coin ids, e.g. `coingecko:ethereum` or `ethereum:0xA0b8…`.
    coins: Vec<String>,
}

pub struct Price;

impl Tool for Price {
    const NAME: &'static str = "price";
    const DESCRIPTION: &'static str = "Current USD price of one or more coins from DefiLlama.";
    type Args = PriceArgs;

    async fn run(args: PriceArgs, _ctx: &Ctx) -> Result<Outcome> {
        if args.coins.is_empty() {
            return Err(err("invalid_args", "name at least one coin"));
        }
        let url = format!(
            "https://coins.llama.fi/prices/current/{}",
            args.coins.join(",")
        );
        let reply = http::get(url).send().await?;
        if reply.status() != 200 {
            return Err(err(
                "upstream_status",
                format!("DefiLlama answered {}", reply.status()),
            ));
        }
        let body: Value = reply.json()?;
        Ok(Outcome::data(trim(&body)))
    }
}

/// Keeps symbol, price and confidence per coin.
fn trim(body: &Value) -> Value {
    let coins: Map<String, Value> = body["coins"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(id, coin)| {
            let coin = json!({
                "symbol": coin["symbol"],
                "price": coin["price"],
                "confidence": coin["confidence"],
            });
            (id.clone(), coin)
        })
        .collect();
    json!({ "coins": coins })
}

aomi_sdk::package!(Price);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trim_keeps_price_fields() {
        let body = json!({ "coins": { "coingecko:ethereum": {
            "symbol": "ETH", "price": 3000.5, "confidence": 0.99, "timestamp": 1, "decimals": 18
        } } });
        assert_eq!(
            trim(&body),
            json!({ "coins": { "coingecko:ethereum": {
                "symbol": "ETH", "price": 3000.5, "confidence": 0.99
            } } })
        );
    }
}
