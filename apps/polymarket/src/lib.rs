use aomi_sdk::*;
use std::sync::LazyLock;

mod client;
pub mod testing;
mod tool;
mod types;

static PREAMBLE: LazyLock<String> = LazyLock::new(client::build_preamble);

dyn_aomi_app!(
    app = client::PolymarketApp,
    name = "polymarket",
    version = "0.1.0",
    preamble = PREAMBLE.as_str(),
    tools = [
        client::SearchPolymarket,
        client::GetPolymarketDetails,
        client::GetPolymarketTrades,
        client::ResolvePolymarketTradeIntent,
        client::BuildPolymarketOrder,
        client::SubmitPolymarketOrder,
    ],
    namespaces = ["common"]
);
