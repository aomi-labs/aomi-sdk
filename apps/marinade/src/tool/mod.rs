//! Tool layer for the marinade app.
//!
//! Mirrors byreal's layout: reads + writes split into submodules, shared
//! helpers (`validate_confirmation`, `resolve_address`) live here.

pub(crate) mod reads;
pub(crate) mod writes;

use aomi_sdk::*;

/// Resolve the connected SVM wallet address. Marinade is SVM-only; we
/// don't accept `arg.wallet` overrides because every Marinade flow must
/// stake/unstake to the connected wallet's account.
pub(crate) fn require_svm_wallet(ctx: &DynToolCallCtx) -> Result<String, String> {
    ctx.attribute_string(&["domain", "svm", "address"])
        .ok_or_else(|| {
            "[marinade] no SVM wallet connected — connect a Solana wallet first \
             (Marinade is mainnet-beta only)"
                .to_string()
        })
}

/// Wrap a Marinade ix descriptor for the `host::SvmStageIx` route plan.
/// Shape matches what the host's `svm_stage_ix` tool expects:
///
/// ```json
/// {
///   "program_id": "<base58 program id>",
///   "accounts": [
///     {"pubkey": "<base58>", "is_signer": false, "is_writable": true},
///     …
///   ],
///   "data_base64": "<base64 instruction data starting with 8-byte discriminator>",
///   "description": "Marinade <action>"
/// }
/// ```
///
/// Used by both [`writes::build_stake`] and [`writes::build_liquid_unstake`].
#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct MarinadeIx {
    pub program_id: String,
    pub accounts: Vec<MarinadeAcct>,
    pub data_base64: String,
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct MarinadeAcct {
    pub pubkey: String,
    pub is_signer: bool,
    pub is_writable: bool,
}
