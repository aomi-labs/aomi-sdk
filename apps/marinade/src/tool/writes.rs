//! Marinade write tools — build/submit pairs for `stake` and `liquid_unstake`.
//!
//! These are the first tools in any aomi-apps plugin to use the new
//! `host::SvmStageIx` + `host::SvmCommitIx` route-target chain (ADR 0003
//! § Decision A + Decision B; markers shipped in SDK 0.1.23).
//!
//! ## Pipeline shape
//!
//! Both `build_*` tools follow the **wallet-broadcast pattern** of the
//! `SvmSelfBroadcast` variant (ADR 0004 § Decision B):
//!
//! ```text
//! build_*       ───┐
//!                  ↓
//!  host::SvmStageIx({instructions: [...]})    .next, bind_as("ix_ids")
//!                  ↓
//!  host::SvmCommitIx({ix_ids, mode: "wallet"}) .after, awaits("ix_ids")
//!                  ↓
//!  wallet signs+sends — sig returned to the LLM
//! ```
//!
//! Two-node route chain: stage as `.next`, commit as `.after`. No
//! separate `submit_*` continuation — `svm_commit_ix({mode: "wallet"})`
//! IS the broadcast for the wallet path. byreal-style apps add a
//! third `submit_*` step because they post to a venue endpoint; this
//! shape skips that since Marinade has no venue submit endpoint.
//!
//! The `internal-rpc` mode of `SvmCommitIx` (runtime-broadcast +
//! `WalletCallback::Tx*`) is blocked on host #38-pipeline-c; once it
//! lands, the `mode` arg flips without restructuring.
//!
//! ## Instruction-composition status: STUB
//!
//! Marinade's stake/liquid_unstake ixs are Anchor-shaped, with ~12 input
//! accounts each (state PDA, mSOL mint, liq-pool PDAs, reserve PDA,
//! user wallet, user mSOL ATA, msol_mint_authority PDA, system/token
//! programs). Resolving those accounts correctly requires:
//!
//! 1. **The Marinade Anchor IDL** to look up account ordering / PDA seeds.
//! 2. **A typed Rust client** (e.g. `marinade-anchor-common` crate) or
//!    a hand-rolled equivalent that derives the user's mSOL ATA, fetches
//!    the live state account, and packs the ix data with the correct
//!    8-byte discriminator + Borsh-encoded args.
//!
//! This module pins the **program ID + 8-byte discriminators** (computed
//! `sha256("global:<method>")[..8]` per Anchor convention, verified
//! against the Marinade IDL) and emits a **placeholder accounts list**
//! that's structurally correct but not on-chain valid. The route plan
//! shape, args validation, and confirmation gates work end-to-end against
//! the host; the staged ix will fail at simulate / commit time until the
//! accounts list is filled in.
//!
//! `HANDOFF.md` tracks the production-readiness gap.

use crate::client::MarinadeApp;
use crate::client::stats;
use crate::tool::{MarinadeAcct, MarinadeIx, require_svm_wallet};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

// ===========================================================================
// Marinade mainnet-beta constants (pinned)
// ===========================================================================

/// Marinade-Anchor program ID on mainnet-beta. Source: Marinade docs +
/// IDL `address` field. Production code should cross-check against
/// `protocol_data::svm::marinade::PROGRAM_ID` when the host-side skill
/// manifest is added (still TBD per ADR 0004 § Decision A — apps stay
/// the default home for SVM; skill option remains open).
pub(crate) const MARINADE_PROGRAM_ID: &str = "MarBmsSgKXdrN1egZf5sqe1TMThiunzMr5sJC4U6gZ7e";

/// mSOL SPL mint on mainnet-beta.
pub(crate) const MSOL_MINT: &str = "mSoLzYCxHdYgdzU16g5QSh3i5K3z3KZK7ytfqcJm7So";

/// Marinade global state PDA on mainnet-beta.
pub(crate) const MARINADE_STATE: &str = "8szGkuLTAux9XMgZ2vtY39jVSowEcpBfFfD8hXSEqdGC";

/// SPL Token program.
pub(crate) const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";

/// System program.
pub(crate) const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";

/// Anchor discriminator: `sha256("global:<method>")[0..8]`. Computed at
/// runtime once and cached implicitly via the pinning tests below; the
/// constants live as functions so a const-fn-incompatible hasher
/// (`sha2`) can still be used.
pub(crate) fn deposit_discriminator() -> [u8; 8] {
    anchor_discriminator("global:deposit")
}

pub(crate) fn liquid_unstake_discriminator() -> [u8; 8] {
    anchor_discriminator("global:liquid_unstake")
}

fn anchor_discriminator(seed: &str) -> [u8; 8] {
    let mut h = Sha256::new();
    h.update(seed.as_bytes());
    let out = h.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&out[..8]);
    disc
}

// ===========================================================================
// build_stake — deposit SOL, mint mSOL
// ===========================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct BuildStakeArgs {
    /// Amount of SOL to stake, in lamports (1 SOL = 1_000_000_000). Pass as
    /// string to avoid JSON-number precision loss on large stakes.
    pub amount_lamports: String,
}

pub(crate) struct BuildStake;

impl DynAomiTool for BuildStake {
    type App = MarinadeApp;
    type Args = BuildStakeArgs;
    const NAME: &'static str = "marinade_build_stake";
    const DESCRIPTION: &'static str = "Build (do not submit) a Marinade stake: deposit SOL, receive mSOL at the current exchange rate. Returns a preview + a routed `svm_stage_ix` → `svm_commit_ix` plan the host wallet drives. Always emit a one-screen confirmation summary (amount, current APY, expected mSOL output) and stop the turn before calling this.";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let wallet = require_svm_wallet(&ctx)?;
        let amount = args
            .amount_lamports
            .parse::<u64>()
            .map_err(|e| format!("[marinade] amount_lamports must be u64: {e}"))?;
        if amount == 0 {
            return Err("[marinade] amount_lamports must be > 0".to_string());
        }

        // Best-effort surface of the current rate so the preview is useful
        // even when the network is slow. Failure is non-fatal — the
        // preview just omits the rate.
        let rate_hint = stats::get_exchange_rate()
            .ok()
            .and_then(|v| v.get("price").cloned());

        let ix = build_deposit_ix(&wallet, amount);
        let preview = json!({
            "action_kind": "stake",
            "preview": {
                "amount_lamports": args.amount_lamports,
                "wallet": wallet,
                "exchange_rate_hint": rate_hint,
            },
            "requires_user_confirmation": true,
            "confirmation_phrase": "confirm",
        });

        build_marinade_route_plan(
            preview,
            vec![ix],
            format!(
                "Marinade stake: {} lamports SOL → mSOL",
                args.amount_lamports
            ),
        )
    }
}

// ===========================================================================
// build_liquid_unstake — burn mSOL, receive SOL (instant, with fee)
// ===========================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct BuildLiquidUnstakeArgs {
    /// Amount of mSOL to burn, in token-units (mSOL has 9 decimals).
    pub msol_amount: String,
}

pub(crate) struct BuildLiquidUnstake;

impl DynAomiTool for BuildLiquidUnstake {
    type App = MarinadeApp;
    type Args = BuildLiquidUnstakeArgs;
    const NAME: &'static str = "marinade_build_liquid_unstake";
    const DESCRIPTION: &'static str = "Build (do not submit) a Marinade **liquid** (instant) unstake: burn mSOL, receive SOL at the current rate minus the liquidity-pool fee (typically 0.1–0.3%, scales with pool utilization). Returns a preview + a routed `svm_stage_ix` → `svm_commit_ix` plan. For zero-fee unstake (1-2 epoch wait + ticket NFT), use `marinade_build_delayed_unstake` (not yet implemented; see HANDOFF.md).";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let wallet = require_svm_wallet(&ctx)?;
        let amount = args
            .msol_amount
            .parse::<u64>()
            .map_err(|e| format!("[marinade] msol_amount must be u64: {e}"))?;
        if amount == 0 {
            return Err("[marinade] msol_amount must be > 0".to_string());
        }

        let ix = build_liquid_unstake_ix(&wallet, amount);
        let preview = json!({
            "action_kind": "liquid_unstake",
            "preview": {
                "msol_amount": args.msol_amount,
                "wallet": wallet,
                "fee_note": "instant unstake carries a small (~0.1-0.3%) liquidity pool fee. \
                             Use delayed_unstake for zero-fee + 1-2 epoch wait.",
            },
            "requires_user_confirmation": true,
            "confirmation_phrase": "confirm",
        });

        build_marinade_route_plan(
            preview,
            vec![ix],
            format!("Marinade liquid unstake: {} mSOL → SOL", args.msol_amount),
        )
    }
}

// ===========================================================================
// Ix composition (STUB — see module docstring)
// ===========================================================================

/// Placeholder Marinade `deposit` ix. Program ID + discriminator are real;
/// the accounts list is a structural scaffold (only the user-derivable
/// pubkeys are filled in — state PDA + program-derived pubkeys need the
/// real Marinade SDK to resolve correctly).
fn build_deposit_ix(user: &str, amount_lamports: u64) -> MarinadeIx {
    let mut data = deposit_discriminator().to_vec();
    data.extend_from_slice(&amount_lamports.to_le_bytes());
    MarinadeIx {
        program_id: MARINADE_PROGRAM_ID.to_string(),
        accounts: deposit_accounts_stub(user),
        data_base64: B64.encode(&data),
        description: "Marinade deposit (stake SOL → mint mSOL)".to_string(),
    }
}

fn build_liquid_unstake_ix(user: &str, msol_amount: u64) -> MarinadeIx {
    let mut data = liquid_unstake_discriminator().to_vec();
    data.extend_from_slice(&msol_amount.to_le_bytes());
    MarinadeIx {
        program_id: MARINADE_PROGRAM_ID.to_string(),
        accounts: liquid_unstake_accounts_stub(user),
        data_base64: B64.encode(&data),
        description: "Marinade liquid unstake (burn mSOL → return SOL minus pool fee)".to_string(),
    }
}

/// STUB. Returns the obviously-user-derivable accounts + placeholder
/// strings for the PDAs that need the real Marinade SDK to resolve.
/// Production: replace with proper account resolution.
fn deposit_accounts_stub(user: &str) -> Vec<MarinadeAcct> {
    vec![
        acct(MARINADE_STATE, false, true),
        acct(MSOL_MINT, false, true),
        acct("__TODO_liq_pool_sol_leg_pda", false, true),
        acct("__TODO_liq_pool_msol_leg", false, true),
        acct("__TODO_liq_pool_msol_leg_authority", false, false),
        acct("__TODO_reserve_pda", false, true),
        acct(user, true, true),                    // transfer_from
        acct("__TODO_user_msol_ata", false, true), // mint_to (derive via ATA)
        acct("__TODO_msol_mint_authority", false, false),
        acct(SYSTEM_PROGRAM, false, false),
        acct(TOKEN_PROGRAM, false, false),
    ]
}

/// STUB. Same shape as `deposit_accounts_stub`.
fn liquid_unstake_accounts_stub(user: &str) -> Vec<MarinadeAcct> {
    vec![
        acct(MARINADE_STATE, false, true),
        acct(MSOL_MINT, false, true),
        acct("__TODO_liq_pool_sol_leg_pda", false, true),
        acct("__TODO_liq_pool_msol_leg", false, true),
        acct("__TODO_treasury_msol_account", false, true),
        acct("__TODO_user_msol_ata", false, true),
        acct(user, true, true),
        acct(SYSTEM_PROGRAM, false, false),
        acct(TOKEN_PROGRAM, false, false),
    ]
}

fn acct(pubkey: &str, is_signer: bool, is_writable: bool) -> MarinadeAcct {
    MarinadeAcct {
        pubkey: pubkey.to_string(),
        is_signer,
        is_writable,
    }
}

// ===========================================================================
// Route plan helper — host::SvmStageIx → host::SvmCommitIx({mode: "wallet"})
// ===========================================================================

/// Build the canonical Marinade route plan: stage ixs on the host, then
/// commit via wallet-broadcast. This is the first aomi-apps consumer of
/// the `host::SvmStageIx` + `host::SvmCommitIx` markers shipped in SDK
/// 0.1.23.
///
/// Two-node chain: `SvmStageIx` as `.next` (binds `ix_ids`), `SvmCommitIx`
/// as `.after` (awaits `ix_ids`). No separate submit_* step — wallet-mode
/// commit IS the broadcast.
///
/// When host #38-pipeline-c (runtime broadcast loop) lands, the `mode`
/// arg can flip from `"wallet"` to `"internal-rpc"` here and the host
/// fires `WalletCallback::TxLanded` instead of synchronously returning
/// — no other change needed.
fn build_marinade_route_plan(
    value: Value,
    instructions: Vec<MarinadeIx>,
    description: String,
) -> Result<ToolReturn, String> {
    ToolReturn::route(value)
        .next(|next| {
            next.add::<host::SvmStageIx>(json!({
                "instructions": instructions,
                "description": description.clone(),
            }))
            .bind_as("ix_ids")
            .note("Stage Marinade instructions for assembly.");
        })
        .after::<host::SvmCommitIx>(json!({
            "mode": "wallet",
            "version": "v0",
        }))
        .awaits("ix_ids")
        .note("Commit via the connected wallet — signs and broadcasts; tx sig returned.")
        .try_build()
        .map_err(|e| format!("[marinade] route build failed: {e}"))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn program_id_is_marinade_canonical() {
        assert_eq!(
            MARINADE_PROGRAM_ID,
            "MarBmsSgKXdrN1egZf5sqe1TMThiunzMr5sJC4U6gZ7e"
        );
    }

    #[test]
    fn discriminators_match_anchor_global_seeding() {
        // Anchor convention: sha256("global:<method>")[..8]. These bytes
        // are the source-of-truth for the on-chain instruction tags;
        // changing them is a hard fork on Marinade's side, not ours.
        // Pin the computation so a refactor of `anchor_discriminator`
        // doesn't accidentally swap hashes.
        let dep = deposit_discriminator();
        let liq = liquid_unstake_discriminator();

        // Recompute inline to assert determinism.
        let mut h = Sha256::new();
        h.update(b"global:deposit");
        let want_dep: [u8; 8] = h.finalize()[..8].try_into().unwrap();
        let mut h = Sha256::new();
        h.update(b"global:liquid_unstake");
        let want_liq: [u8; 8] = h.finalize()[..8].try_into().unwrap();

        assert_eq!(dep, want_dep, "deposit discriminator drift");
        assert_eq!(liq, want_liq, "liquid_unstake discriminator drift");
    }

    #[test]
    fn deposit_ix_data_starts_with_discriminator() {
        let ix = build_deposit_ix("So11111111111111111111111111111111111111112", 1_000_000_000);
        let data = B64.decode(ix.data_base64).expect("data is base64");
        assert_eq!(&data[..8], &deposit_discriminator());
        // Amount is little-endian u64 right after the discriminator.
        let amount = u64::from_le_bytes(data[8..16].try_into().unwrap());
        assert_eq!(amount, 1_000_000_000);
    }

    #[test]
    fn liquid_unstake_ix_data_starts_with_discriminator() {
        let ix =
            build_liquid_unstake_ix("So11111111111111111111111111111111111111112", 500_000_000);
        let data = B64.decode(ix.data_base64).expect("data is base64");
        assert_eq!(&data[..8], &liquid_unstake_discriminator());
        let amount = u64::from_le_bytes(data[8..16].try_into().unwrap());
        assert_eq!(amount, 500_000_000);
    }

    #[test]
    fn deposit_accounts_include_user_as_writable_signer() {
        let accs = deposit_accounts_stub("Pubkey...");
        let user = accs
            .iter()
            .find(|a| a.pubkey == "Pubkey...")
            .expect("user account present");
        assert!(user.is_signer, "user pays + signs");
        assert!(user.is_writable, "user balance changes");
    }

    #[test]
    fn build_stake_args_round_trip() {
        let v: serde_json::Value = serde_json::from_str(r#"{"amount_lamports": "1000000000"}"#)
            .expect("BuildStakeArgs JSON parses");
        let args: BuildStakeArgs = serde_json::from_value(v).expect("deserialize");
        assert_eq!(args.amount_lamports, "1000000000");
    }

    #[test]
    fn build_liquid_unstake_args_round_trip() {
        let v: serde_json::Value = serde_json::from_str(r#"{"msol_amount": "500000000"}"#)
            .expect("BuildLiquidUnstakeArgs JSON parses");
        let args: BuildLiquidUnstakeArgs = serde_json::from_value(v).expect("deserialize");
        assert_eq!(args.msol_amount, "500000000");
    }

    #[test]
    fn route_plan_serializes_with_stage_then_commit() {
        // Smoke-test the route shape end-to-end: build a plan, serialize
        // the ToolReturn, assert the route chain is
        // [SvmStageIx, SvmCommitIx] in that order with the right
        // bind/awaits aliases.
        let plan = build_marinade_route_plan(
            json!({"action_kind": "stake", "preview": {}}),
            vec![build_deposit_ix(
                "So11111111111111111111111111111111111111112",
                1_000_000_000,
            )],
            "Marinade stake test".to_string(),
        )
        .expect("plan builds");

        let serialized = serde_json::to_value(&plan).expect("serialize");
        let routes = serialized
            .get("__aomi_tool_routes")
            .and_then(Value::as_array)
            .expect("routes present");
        let tools: Vec<&str> = routes
            .iter()
            .filter_map(|r| r.get("tool").and_then(Value::as_str))
            .collect();
        assert_eq!(tools, vec!["svm_stage_ix", "svm_commit_ix"]);

        // The commit step awaits the stage step's bound alias.
        let commit = &routes[1];
        assert_eq!(
            commit.pointer("/trigger/type").and_then(Value::as_str),
            Some("on_bound_event")
        );
        assert_eq!(
            commit.pointer("/trigger/alias").and_then(Value::as_str),
            Some("ix_ids")
        );

        // The stage step binds the alias the commit step awaits.
        let stage = &routes[0];
        assert_eq!(stage.get("bind_as").and_then(Value::as_str), Some("ix_ids"));

        // The commit step carries wallet mode (today; flips to
        // internal-rpc when host #38-pipeline-c lands).
        assert_eq!(
            commit.pointer("/args/mode").and_then(Value::as_str),
            Some("wallet")
        );
    }
}

// Test of route-plan shape lives at the app root (testing.rs) so it can
// exercise BuildStake/BuildLiquidUnstake through the full DynAomiTool
// pathway without poking module-private functions.
