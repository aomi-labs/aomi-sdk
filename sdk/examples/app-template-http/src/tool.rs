use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

pub(crate) struct SearchCoins;

impl DynAomiTool for SearchCoins {
    type App = HttpJsonExampleApp;
    type Args = SearchCoinsArgs;
    const NAME: &'static str = "search_coins";
    const DESCRIPTION: &'static str = "Search CoinGecko for matching assets and return coin ids developers can use in follow-up price calls.";

    fn run(
        _app: &HttpJsonExampleApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let client = CoinGeckoClient::new()?;
        let value = client.get_json("/search", &[("query", args.query.as_str())])?;
        let coins = value
            .get("coins")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let matches = coins
            .into_iter()
            .take(10)
            .map(|coin| {
                json!({
                    "id": coin.get("id"),
                    "symbol": coin.get("symbol"),
                    "name": coin.get("name"),
                    "market_cap_rank": coin.get("market_cap_rank"),
                })
            })
            .collect::<Vec<_>>();

        Ok(json!({
            "query": args.query,
            "matches": matches,
        }))
    }
}

pub(crate) struct GetCoinPrice;

impl DynAomiTool for GetCoinPrice {
    type App = HttpJsonExampleApp;
    type Args = GetCoinPriceArgs;
    const NAME: &'static str = "get_coin_price";
    const DESCRIPTION: &'static str = "Fetch the current USD price for a CoinGecko asset id.";

    fn run(
        _app: &HttpJsonExampleApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let client = CoinGeckoClient::new()?;
        let value = client.get_json(
            "/simple/price",
            &[("ids", args.coin_id.as_str()), ("vs_currencies", "usd")],
        )?;

        let usd = value
            .get(&args.coin_id)
            .and_then(Value::as_object)
            .and_then(|coin| coin.get("usd"))
            .ok_or_else(|| format!("CoinGecko price missing for {}", args.coin_id))?;

        Ok(json!({
            "coin_id": args.coin_id,
            "price_usd": usd,
        }))
    }
}
