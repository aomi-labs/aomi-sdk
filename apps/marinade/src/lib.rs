//! Marinade liquid staking — reference aomi-app for the
//! `BuiltinApp::SvmSelfBroadcast` variant.
//!
//! Marinade is the second reference SVM app shipping after byreal. Where
//! byreal demonstrates the cross-chain (string-typed namespaces) +
//! app-broadcast (venue HTTP submit) pattern, Marinade demonstrates:
//!
//! - **Variant-typed declaration**: `variant = AppVariant::SvmSelfBroadcast`
//!   instead of explicit `namespaces = [...]`. The host loader uses
//!   `variant.default_namespaces()` once `DynManifest.variant`
//!   consumption lands (filed against product-mono ralph).
//! - **Self-broadcast pipeline**: build_* tools emit route plans that
//!   drive `host::SvmStageIx → host::SvmCommitIx({mode: "wallet"})`.
//!   The runtime owns the broadcast loop in the `internal-rpc` future;
//!   today wallet mode is the only functional path.
//! - **First consumer of the new SVM `host::*` markers** shipped in
//!   SDK 0.1.23 (`SvmStageIx`, `SvmCommitIx`, etc.).
//!
//! ## Why Marinade
//!
//! Liquid staking is a textbook self-broadcast use case: small fixed set
//! of actions (stake / liquid-unstake / delayed-unstake / claim), no
//! venue-side submit endpoint (unlike Jupiter `/execute` or byreal's
//! AMM/RFQ submit), and a stats API rich enough to make read tools
//! useful (APY, TVL, validator scores, exchange rate).
//!
//! ## Production-readiness today
//!
//! - **Read tools**: fully functional against `api.marinade.finance`.
//! - **Write tools**: route-plan shape end-to-end correct; instruction
//!   composition is a structural scaffold pending Marinade Anchor SDK
//!   integration. See `HANDOFF.md`.

use aomi_sdk::*;

mod client;
mod tool;

const PREAMBLE: &str = r#"You are the **Marinade Agent**, a focused assistant for Marinade Finance's liquid-staking flows on Solana mainnet-beta.

## Three actions, one rate

| Action | Tool pair | Effect |
|---|---|---|
| Stake | `marinade_build_stake` / `marinade_submit_stake` | Deposit SOL, receive mSOL at current rate |
| Liquid unstake | `marinade_build_liquid_unstake` / `marinade_submit_liquid_unstake` | Burn mSOL, receive SOL **instantly** minus the pool fee |
| Delayed unstake | (not yet shipped — see `HANDOFF.md`) | Burn mSOL, receive SOL **after 1-2 epochs** (~3 days), zero fee, ticket NFT |

mSOL is interest-bearing: the token *count* never changes, but the mSOL:SOL exchange rate increases as the underlying validators earn. To value a user's mSOL position in SOL terms, multiply by `marinade_get_exchange_rate`.

## Read tools

- `marinade_get_apy` — 30-day rolling APY (fraction; e.g. 0.071 = 7.1%).
- `marinade_get_tvl` — total SOL staked through Marinade.
- `marinade_get_exchange_rate` — current mSOL:SOL price; always > 1.0.
- `marinade_get_validators` — Marinade's delegation set with per-validator scores.

## Pipeline (write paths)

Every `build_*` returns a routed plan:

    build_* → svm_stage_ix → svm_commit_ix(mode="wallet") → submit_*

The host wallet signs and broadcasts. `submit_*` is a synchronous continuation anchor for the route — by the time it runs, the tx has been submitted (landing is the wallet's responsibility for wallet-broadcast flows).

## Confirmation gates (always)

Before calling any `build_*` tool, emit a one-screen pre-execute summary and stop the turn.

**Stake:**

    Stake: <amount> SOL
    Current rate: <rate> mSOL/SOL
    Expected mSOL: ~<amount/rate>
    30d APY: <apy>%
    Wallet: <svm address>

**Liquid unstake:**

    Burn: <amount> mSOL
    Current rate: <rate> SOL/mSOL
    Expected SOL: ~<amount*rate> (minus pool fee, typically 0.1-0.3%)
    Wallet: <svm address>
    Tip: delayed unstake (1-2 epochs) is zero-fee — mention if not time-sensitive.

Wait for the user to reply with "go" / "confirm" before calling `build_*`.

## Sizing

- `amount_lamports` for stake: SOL has 9 decimals, so 1 SOL = 1_000_000_000.
- `msol_amount` for liquid unstake: mSOL also has 9 decimals.
- Marinade has no minimum stake on-chain, but very small stakes (< ~0.01 SOL) lose >> 1% to transaction fees.

## Out of scope (today)

- **Delayed unstake** (ticket NFT flow). Routes a different ix list + holds a position-NFT the user later claims via `claim`. Tracked in `HANDOFF.md`.
- **Validator selection.** Marinade's delegation algorithm picks validators; the user can't override per-stake. `marinade_get_validators` surfaces the set for transparency.
- **Bonded / TVL-based slashing.** Marinade is not a bonded protocol — slashing risk routes through the delegated validators directly.

## Errors

- `MARINADE_API_URL returned 5xx` → upstream stats outage; retry with backoff.
- `amount_lamports must be > 0` → caller forgot to validate; surface a clear message.
- `no SVM wallet connected` → user is in EVM-only mode; tell them to connect a Solana wallet first.
- Tx simulate fails with `InvalidAccountData` → the on-chain account list is wrong (production gap; see HANDOFF.md). For now, surface a `request retry once accounts list is filled in` hint.
"#;

dyn_aomi_app!(
    app = client::MarinadeApp,
    name = "marinade",
    version = "0.1.0",
    preamble = PREAMBLE,
    tools = [
        // reads (all hit api.marinade.finance)
        tool::reads::GetApy,
        tool::reads::GetTvl,
        tool::reads::GetExchangeRate,
        tool::reads::GetValidators,
        // writes (route plans through host::SvmStageIx + host::SvmCommitIx)
        tool::writes::BuildStake,
        tool::writes::BuildLiquidUnstake,
    ],
    variant = AppVariant::SvmSelfBroadcast,
);

pub mod testing {
    //! Smoke-tests that exercise the variant + route plan end-to-end.
    //! These run against the in-process app trait, not the loaded dylib,
    //! so they don't need a host runtime.

    #[cfg(test)]
    mod tests {
        use super::super::*;

        #[test]
        fn manifest_declares_svm_self_broadcast_variant() {
            let app = client::MarinadeApp;
            let manifest = app.manifest();
            assert_eq!(manifest.name, "marinade");
            assert_eq!(manifest.variant, Some("svm-self-broadcast".to_string()));
            // No explicit `namespaces` — the variant arm of the macro
            // intentionally sets `namespaces() -> None` so the host
            // loader uses `variant.default_namespaces()` as the source
            // of truth.
            assert_eq!(manifest.namespaces, None);
        }

        #[test]
        fn manifest_lists_all_six_tools() {
            let app = client::MarinadeApp;
            let manifest = app.manifest();
            let names: Vec<&str> = manifest.tools.iter().map(|t| t.name.as_str()).collect();
            assert!(names.contains(&"marinade_get_apy"));
            assert!(names.contains(&"marinade_get_tvl"));
            assert!(names.contains(&"marinade_get_exchange_rate"));
            assert!(names.contains(&"marinade_get_validators"));
            assert!(names.contains(&"marinade_build_stake"));
            assert!(names.contains(&"marinade_build_liquid_unstake"));
            assert_eq!(
                names.len(),
                6,
                "marinade exposes 4 reads + 2 writes; no submit_* because \
                 svm_commit_ix(mode=wallet) IS the broadcast",
            );
        }

        #[test]
        fn preamble_calls_out_self_broadcast_pipeline() {
            // The preamble must mention the route plan shape so the LLM
            // knows which tools chain together. Frozen check; if you
            // rewrite the preamble, update this.
            assert!(PREAMBLE.contains("svm_stage_ix"));
            assert!(PREAMBLE.contains("svm_commit_ix"));
            assert!(PREAMBLE.contains("build_"));
            assert!(PREAMBLE.contains("submit_"));
        }

        #[test]
        fn variant_default_namespaces_match_host_self_broadcast() {
            // Catches drift between this app's expectation and the
            // AppVariant::SvmSelfBroadcast composition shipped in SDK
            // 0.1.22. If the host adds a sub-namespace to that variant,
            // this assertion bumps in lockstep.
            let app = client::MarinadeApp;
            let variant = app.variant().expect("marinade declares a variant");
            assert_eq!(variant.as_str(), "svm-self-broadcast");
            assert_eq!(
                variant.default_namespaces(),
                &["svm-reads", "svm-ix-broadcast", "svm-tx-broadcast"]
            );
        }
    }
}
