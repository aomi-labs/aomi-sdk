use crate::client::*;
use aomi_sdk::*;
use serde_json::Value;

impl DynAomiTool for GetAllVaults {
    type App = YearnApp;
    type Args = GetAllVaultsArgs;
    const NAME: &'static str = "get_all_vaults";
    const DESCRIPTION: &'static str =
        "List all Yearn vaults on a given chain with TVL, APY, strategies, and fees.";

    fn run(_app: &YearnApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = YearnClient::new()?;
        client.get_all_vaults(args.chain_id)
    }
}

impl DynAomiTool for GetVaultDetail {
    type App = YearnApp;
    type Args = GetVaultDetailArgs;
    const NAME: &'static str = "get_vault_detail";
    const DESCRIPTION: &'static str = "Get detailed info for a specific Yearn vault by address: APY breakdown (net, gross, weekly, monthly, inception), strategies, fees, and TVL.";

    fn run(_app: &YearnApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = YearnClient::new()?;
        client.get_vault_detail(args.chain_id, &args.address)
    }
}

impl DynAomiTool for GetBlacklistedVaults {
    type App = YearnApp;
    type Args = GetBlacklistedVaultsArgs;
    const NAME: &'static str = "get_blacklisted_vaults";
    const DESCRIPTION: &'static str = "Get the list of blacklisted Yearn vaults across all chains.";

    fn run(_app: &YearnApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = YearnClient::new()?;
        client.get_blacklisted_vaults()
    }
}
