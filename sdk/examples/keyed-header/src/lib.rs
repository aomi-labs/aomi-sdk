//! A keyed reader: Jupiter prices. The host injects the builder's
//! `JUPITER_API_KEY` as the `x-api-key` header; the package never sees it.

use aomi_sdk::{Ctx, Outcome, Result, Tool, err, http, secrets};
use serde::Deserialize;
use serde_json::{Value, json};

const KEY: &str = "JUPITER_API_KEY";

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PriceArgs {
    /// Solana token mint addresses.
    ids: Vec<String>,
}

pub struct Price;

impl Tool for Price {
    const NAME: &'static str = "price";
    const DESCRIPTION: &'static str = "Current USD price of Solana tokens from Jupiter.";
    type Args = PriceArgs;

    async fn run(args: PriceArgs, _ctx: &Ctx) -> Result<Outcome> {
        if !secrets::has(KEY) {
            return Err(err(
                "secret_missing",
                "the package's Jupiter API key (JUPITER_API_KEY) is not set",
            ));
        }
        if args.ids.is_empty() {
            return Err(err("invalid_args", "name at least one mint"));
        }
        let reply = http::get("https://api.jup.ag/price/v3")
            .query(&[("ids", args.ids.join(","))])
            .send()
            .await?;
        let record = reply.record()?;
        if reply.status() != 200 {
            return Err(err(
                "upstream_status",
                format!("Jupiter answered {} (record {record})", reply.status()),
            ));
        }
        let prices: Value = reply.json()?;
        Ok(Outcome::data(json!({ "prices": prices, "record": record })))
    }
}

aomi_sdk::package!(Price);
