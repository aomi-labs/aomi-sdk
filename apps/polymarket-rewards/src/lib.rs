mod client;
mod tool;

use aomi_sdk::*;
use std::sync::LazyLock;

static PREAMBLE: LazyLock<String> = LazyLock::new(client::build_preamble);

dyn_aomi_app!(
    app = client::PolymarketRewardsApp,
    name = "polymarket-rewards",
    version = "0.1.0",
    preamble = PREAMBLE.as_str(),
    tools = [
        tool::EnsureRewardClobCredentials,
        tool::FindRewardMarkets,
        tool::RankRewardPlans,
        tool::ResolveRewardDeployment,
        tool::BuildQuotePlan,
        tool::SubmitRewardQuote,
        tool::ExecuteQuotePlan,
        tool::GetQuotePlanStatus,
        tool::WithdrawQuoteLiquidity,
    ],
    namespaces = ["common"]
);
