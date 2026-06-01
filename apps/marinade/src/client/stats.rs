//! Marinade stats endpoints — APY, TVL, exchange rate, validator scores.
//!
//! Endpoint shapes verified against `api.marinade.finance` as of late 2026.
//! Path constants live here; the HTTP plumbing is in [`super`].

use super::marinade_get;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const PATH_APY: &str = "/msol/apy/30d";
const PATH_TVL: &str = "/tlv";
const PATH_PRICE: &str = "/msol/sol_price";
const PATH_VALIDATORS: &str = "/validators";

/// Lightweight passthrough shape — Marinade's responses vary by endpoint,
/// so we deserialize into `Value` to stay tolerant. Real consumers should
/// access fields via JSON pointer in the tool layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct Passthrough(Value);

pub(crate) fn get_apy_30d() -> Result<Value, String> {
    marinade_get::<Passthrough>(PATH_APY)
}

pub(crate) fn get_tvl() -> Result<Value, String> {
    marinade_get::<Passthrough>(PATH_TVL)
}

pub(crate) fn get_exchange_rate() -> Result<Value, String> {
    marinade_get::<Passthrough>(PATH_PRICE)
}

pub(crate) fn get_validators() -> Result<Value, String> {
    marinade_get::<Passthrough>(PATH_VALIDATORS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_paths_are_stable_strings() {
        // Frozen contract — Marinade endpoint rename means a one-line fix
        // here, no other plumbing changes. The constants are the source
        // of truth for what the read tools call.
        assert_eq!(PATH_APY, "/msol/apy/30d");
        assert_eq!(PATH_TVL, "/tlv");
        assert_eq!(PATH_PRICE, "/msol/sol_price");
        assert_eq!(PATH_VALIDATORS, "/validators");
    }
}
