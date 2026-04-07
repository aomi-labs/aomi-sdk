use crate::client::*;
use aomi_sdk::*;
use serde_json::Value;

impl DynAomiTool for GetMorphoMarkets {
    type App = MorphoApp;
    type Args = GetMorphoMarketsArgs;
    const NAME: &'static str = "get_markets";
    const DESCRIPTION: &'static str =
        "List all Morpho lending markets with LTV, supply/borrow APY, and available liquidity.";

    fn run(_app: &MorphoApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = MorphoClient::new()?;
        client.get_markets()
    }
}

impl DynAomiTool for GetMorphoVaults {
    type App = MorphoApp;
    type Args = GetMorphoVaultsArgs;
    const NAME: &'static str = "get_vaults";
    const DESCRIPTION: &'static str =
        "List Morpho vaults with APY, TVL, and allocation strategy details.";

    fn run(_app: &MorphoApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = MorphoClient::new()?;
        client.get_vaults()
    }
}

impl DynAomiTool for GetMorphoUserPositions {
    type App = MorphoApp;
    type Args = GetMorphoUserPositionsArgs;
    const NAME: &'static str = "get_user_positions";
    const DESCRIPTION: &'static str = "Get a user's Morpho positions including deposits, borrows, and vault holdings for a given wallet address.";

    fn run(_app: &MorphoApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = MorphoClient::new()?;
        client.get_user_positions(&args.address)
    }
}
