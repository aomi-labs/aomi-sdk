//! Read-only Marinade tools. All four hit `api.marinade.finance` directly;
//! no signing or chain interaction.

use crate::client::stats;
use crate::client::MarinadeApp;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::Value;

// ==========================================================================
// marinade_get_apy — 30-day mSOL APY
// ==========================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct NoArgs {}

pub(crate) struct GetApy;

impl DynAomiTool for GetApy {
    type App = MarinadeApp;
    type Args = NoArgs;
    const NAME: &'static str = "marinade_get_apy";
    const DESCRIPTION: &'static str = "30-day rolling APY for mSOL (Marinade liquid staking). Returned as a fraction (e.g. 0.071 = 7.1%) plus the start/end window. Use to set user expectations before `marinade_build_stake`.";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        stats::get_apy_30d()
    }
}

// ==========================================================================
// marinade_get_tvl — total value locked
// ==========================================================================

pub(crate) struct GetTvl;

impl DynAomiTool for GetTvl {
    type App = MarinadeApp;
    type Args = NoArgs;
    const NAME: &'static str = "marinade_get_tvl";
    const DESCRIPTION: &'static str = "Total SOL staked through Marinade across all delegated validators. Returned in lamports + a human-readable SOL number. Use as a sanity check before large stakes (avoid >5% of TVL in a single tx).";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        stats::get_tvl()
    }
}

// ==========================================================================
// marinade_get_exchange_rate — mSOL/SOL price
// ==========================================================================

pub(crate) struct GetExchangeRate;

impl DynAomiTool for GetExchangeRate {
    type App = MarinadeApp;
    type Args = NoArgs;
    const NAME: &'static str = "marinade_get_exchange_rate";
    const DESCRIPTION: &'static str = "Current mSOL:SOL exchange rate. Always > 1.0 and monotonically increasing — rewards accrue inside mSOL, the token count never changes. Multiply user's mSOL balance by this rate to get SOL-equivalent value.";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        stats::get_exchange_rate()
    }
}

// ==========================================================================
// marinade_get_validators — delegation set scoring
// ==========================================================================

pub(crate) struct GetValidators;

impl DynAomiTool for GetValidators {
    type App = MarinadeApp;
    type Args = NoArgs;
    const NAME: &'static str = "marinade_get_validators";
    const DESCRIPTION: &'static str = "Marinade's current delegation set with per-validator scores (commission, performance, decentralization). Useful for users curious WHERE their stake goes. The agent does not pick validators — Marinade's algorithm does — but surfacing the top N gives a useful answer to \"who am I delegating to?\".";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        stats::get_validators()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_tool_names_are_stable() {
        // Tool-name strings are the LLM contract; renaming them is an
        // app-version bump. Pin to catch accidental drift.
        assert_eq!(GetApy::NAME, "marinade_get_apy");
        assert_eq!(GetTvl::NAME, "marinade_get_tvl");
        assert_eq!(GetExchangeRate::NAME, "marinade_get_exchange_rate");
        assert_eq!(GetValidators::NAME, "marinade_get_validators");
    }

    #[test]
    fn no_args_deserializes_from_empty_object() {
        let _: NoArgs = serde_json::from_str("{}").expect("NoArgs accepts empty");
    }
}
