use aomi_sdk::*;
use std::sync::LazyLock;

mod client;
mod tool;
mod types;

static PREAMBLE: LazyLock<String> = LazyLock::new(client::build_preamble);

dyn_aomi_app!(
    app = client::KalshiApp,
    name = "kalshi",
    version = "0.1.0",
    preamble = PREAMBLE.as_str(),
    tools = [
        client::SimmerRegister,
        client::SimmerStatus,
        client::SimmerBriefing,
        client::ImportKalshiMarket,
        client::FetchSimmerMarketContext,
        client::SimmerPlaceOrder,
        client::SimmerGetPositions,
        client::SimmerGetPortfolio,
        client::SearchSimmerMarkets,
    ],
    namespaces = ["common"]
);
