use crate::client::*;
use aomi_sdk::*;
use serde_json::Value;

pub(crate) struct Venues;
impl DynAomiTool for Venues {
    type App = SolanaDefiApp;
    type Args = EmptyArgs;
    const NAME: &'static str = "solana_defi_venues";
    const DESCRIPTION: &'static str = "List supported Solana DeFi protocol families and example market addresses. Read-only; examples are not recommendations.";
    fn run(_: &Self::App, args: Self::Args, _: DynToolCallCtx) -> Result<Value, String> {
        Client::new()?.call("venues", &args)
    }
}

pub(crate) struct Market;
impl DynAomiTool for Market {
    type App = SolanaDefiApp;
    type Args = MarketArgs;
    const NAME: &'static str = "solana_defi_market";
    const DESCRIPTION: &'static str = "Inspect an exact market or vault from chain state, including assets, program ownership, wallet position and available fee metadata. No signing.";
    fn run(_: &Self::App, args: Self::Args, _: DynToolCallCtx) -> Result<Value, String> {
        Client::new()?.call("market", &args)
    }
}

pub(crate) struct Prepare;
impl DynAomiTool for Prepare {
    type App = SolanaDefiApp;
    type Args = PrepareArgs;
    const NAME: &'static str = "solana_defi_prepare";
    const DESCRIPTION: &'static str = "Prepare unsigned deposit/withdraw instructions using current market state and official SDK recipes. Does not sign or submit. Preserve returned batches for the shared executor's preview and confirmed execution.";
    fn run(_: &Self::App, args: Self::Args, _: DynToolCallCtx) -> Result<Value, String> {
        Client::new()?.call("prepare", &args)
    }
}

pub(crate) struct Position;
impl DynAomiTool for Position {
    type App = SolanaDefiApp;
    type Args = MarketArgs;
    const NAME: &'static str = "solana_defi_position";
    const DESCRIPTION: &'static str = "Read the wallet's current Jupiter lending, Kamino vault/farm or PumpSwap LP position. Includes externally acquired holdings; not controller attribution or profit.";
    fn run(_: &Self::App, args: Self::Args, _: DynToolCallCtx) -> Result<Value, String> {
        Client::new()?.call("position", &args)
    }
}
