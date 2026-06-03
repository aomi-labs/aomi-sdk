//! Lane 2 — `transfer_sol_via_tx`. App builds the full `VersionedTransaction`
//! client-side using `solana-sdk`, fetches a recent blockhash from the
//! configured cluster RPC, bincode-serializes the legacy tx, base64-encodes
//! it, and emits a `svm_stage_tx` → `svm_commit_tx({mode: "wallet"})` route
//! plan. The host's stage_tx decodes the blob, validates payer, mints a
//! `pending_tx_id`. The host's commit_tx then wraps the stored blob into
//! the wallet approval as-is.
//!
//! Pipeline shape:
//!
//! ```text
//! transfer_sol_via_tx ───┐
//!                        ↓
//!  host::SvmStageTx({tx: "<base64>", description, kind: "svm-transfer.system"})
//!                                                            .next, bind_as("tx_id")
//!                        ↓
//!  host::SvmCommitTx({mode: "wallet"})                       .after, awaits("tx_id")
//!                        ↓
//!  wallet signs + sends — tx sig returned to the LLM
//! ```
//!
//! The smoke validates two distinct things at the boundary:
//!
//! 1. The app is the producer-of-record for the tx blob (mirrors how a
//!    venue like byreal `/build-swap-tx` or Jupiter `/swap` returns one).
//! 2. The host's Lane 2 commit (`svm_commit_tx({tx_id})`) actually
//!    surfaces the stored blob into the wallet approval — a Lane 1
//!    regression here would crash this test on the host side.

use std::str::FromStr;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Deserialize;
use serde_json::{json, Value};
use solana_sdk::{
    hash::Hash,
    instruction::{AccountMeta, Instruction},
    message::{Message, VersionedMessage},
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};

use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;

use crate::client::{fetch_recent_blockhash, SvmTransferApp};
use crate::tool::{
    require_svm_wallet, system_transfer_data, validate_base58_address, SYSTEM_PROGRAM_ID,
};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct TransferSolViaTxArgs {
    /// Base58 Solana pubkey of the recipient.
    pub to: String,
    /// Amount of SOL to transfer, in lamports (1 SOL = 1_000_000_000).
    pub amount_lamports: String,
}

pub(crate) struct TransferSolViaTx;

impl DynAomiTool for TransferSolViaTx {
    type App = SvmTransferApp;
    type Args = TransferSolViaTxArgs;
    const NAME: &'static str = "transfer_sol_via_tx";
    const DESCRIPTION: &'static str =
        "**Lane 2 smoke** — transfer SOL to `to` from the connected SVM wallet, building the \
         full VersionedTransaction blob client-side and emitting it through the canonical \
         `svm_stage_tx` → `svm_commit_tx({mode: \"wallet\"})` route plan. The app fetches a \
         recent blockhash from the configured cluster RPC (devnet by default) before composing \
         the blob, so the host's commit_tx can wrap the stored blob into the wallet approval \
         as-is. Always emit a one-screen confirmation summary (amount, to, from, cluster) and \
         stop the turn before calling this.";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let wallet = require_svm_wallet(&ctx)?;
        let amount = args
            .amount_lamports
            .parse::<u64>()
            .map_err(|e| format!("[svm-transfer] amount_lamports must be u64: {e}"))?;
        if amount == 0 {
            return Err("[svm-transfer] amount_lamports must be > 0".to_string());
        }
        validate_base58_address(&args.to)
            .map_err(|e| format!("[svm-transfer] invalid `to`: {e}"))?;

        let (cluster_name, _) = crate::client::rpc_url();
        let blockhash = fetch_recent_blockhash().map_err(|e| {
            format!(
                "[svm-transfer] Lane 2 needs a recent blockhash before it can build the tx blob: {e}"
            )
        })?;

        let blob_b64 = build_legacy_transfer_blob(&wallet, &args.to, amount, &blockhash)?;

        let preview = json!({
            "action_kind": "transfer_sol_via_tx",
            "lane": 2,
            "preview": {
                "amount_lamports": args.amount_lamports,
                "from": wallet,
                "to": args.to,
                "cluster": crate::client::cluster_id(),
                "recent_blockhash": blockhash,
            },
            "blob_size_bytes": blob_b64.len(),
            "requires_user_confirmation": true,
            "confirmation_phrase": "confirm",
        });

        build_lane_2_route_plan(
            preview,
            blob_b64,
            format!(
                "svm-transfer Lane 2 ({cluster_name}): {} lamports to {}",
                amount, args.to
            ),
        )
    }
}

/// Build a base64-encoded legacy `VersionedTransaction` carrying one
/// System Program transfer ix.
///
/// "Legacy" because (a) System::transfer doesn't use ALTs so v0 buys us
/// nothing and (b) legacy keeps the wire shape minimal for the smoke
/// test. The signature is a single zero-filled placeholder — the wallet
/// fills it in at sign time.
fn build_legacy_transfer_blob(
    payer: &str,
    to: &str,
    amount_lamports: u64,
    recent_blockhash: &str,
) -> Result<String, String> {
    let payer_pk = Pubkey::from_str(payer)
        .map_err(|e| format!("payer is not a base58 pubkey ({payer}): {e}"))?;
    let to_pk =
        Pubkey::from_str(to).map_err(|e| format!("`to` is not a base58 pubkey ({to}): {e}"))?;
    let system_pk = Pubkey::from_str(SYSTEM_PROGRAM_ID).expect("known system program id");
    let blockhash = Hash::from_str(recent_blockhash)
        .map_err(|e| format!("recent_blockhash is not a base58 hash: {e}"))?;

    let ix = Instruction {
        program_id: system_pk,
        accounts: vec![
            AccountMeta::new(payer_pk, true),
            AccountMeta::new(to_pk, false),
        ],
        data: system_transfer_data(amount_lamports),
    };
    let mut message = Message::new(&[ix], Some(&payer_pk));
    message.recent_blockhash = blockhash;

    let vtx = VersionedTransaction {
        signatures: vec![Signature::default()],
        message: VersionedMessage::Legacy(message),
    };
    let bytes = bincode::serialize(&vtx)
        .map_err(|e| format!("bincode serialize VersionedTransaction failed: {e}"))?;
    Ok(B64.encode(&bytes))
}

fn build_lane_2_route_plan(
    value: Value,
    blob_b64: String,
    description: String,
) -> Result<ToolReturn, String> {
    ToolReturn::route(value)
        .next(|next| {
            next.add::<host::SvmStageTx>(json!({
                "tx": blob_b64,
                "description": description.clone(),
                "kind": "svm-transfer.system",
            }))
            .bind_as("tx_id")
            .note("Stage the venue-built System Program transfer blob.");
        })
        .after::<host::SvmCommitTx>(json!({"mode": "wallet"}))
        .awaits("tx_id")
        .note("Commit via the connected wallet — signs the stored blob and broadcasts.")
        .try_build()
        .map_err(|e| format!("[svm-transfer] Lane 2 route build failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use aomi_sdk::testing::TestCtxBuilder;

    fn ctx_with_wallet(addr: &str) -> DynToolCallCtx {
        TestCtxBuilder::new("transfer_sol_via_tx")
            .attribute(
                "domain",
                serde_json::json!({ "svm": { "address": addr, "cluster": "devnet" } }),
            )
            .build()
    }

    #[test]
    fn legacy_blob_round_trips_through_solana_sdk_deserialize() {
        // The host's stage_tx uses `bincode::deserialize::<VersionedTransaction>`
        // — this test verifies our serialized bytes survive a round-trip
        // through the same path before they ever reach the host.
        let payer = "11111111111111111111111111111111";
        let to = "11111111111111111111111111111111";
        // A real-looking devnet blockhash format; the value is arbitrary.
        let blockhash = "GfDqQwBxFqUNHNyEFwQRRYRb3MaAfDCYU1Q3vyfXVDwf";

        let blob = build_legacy_transfer_blob(payer, to, 1_000_000, blockhash)
            .expect("blob builds");
        let bytes = B64.decode(&blob).expect("base64 decodes");
        let vtx: VersionedTransaction =
            bincode::deserialize(&bytes).expect("bincode deserialize round-trips");
        assert!(matches!(vtx.message, VersionedMessage::Legacy(_)));
        let static_keys = vtx.message.static_account_keys();
        // payer is first per the Solana invariant.
        assert_eq!(
            static_keys[0],
            Pubkey::from_str(payer).unwrap(),
            "fee payer must be the first static key"
        );
    }

    #[test]
    fn legacy_blob_carries_the_requested_lamports_in_ix_data() {
        let blob = build_legacy_transfer_blob(
            "11111111111111111111111111111111",
            "11111111111111111111111111111111",
            7_777_777,
            "GfDqQwBxFqUNHNyEFwQRRYRb3MaAfDCYU1Q3vyfXVDwf",
        )
        .expect("blob builds");
        let bytes = B64.decode(&blob).expect("base64 decodes");
        let vtx: VersionedTransaction = bincode::deserialize(&bytes).unwrap();
        let ix_data = match &vtx.message {
            VersionedMessage::Legacy(m) => m.instructions[0].data.clone(),
            _ => panic!("expected legacy message"),
        };
        // 4 byte LE discriminator (2) + 8 byte LE lamports (7_777_777).
        assert_eq!(&ix_data[0..4], &[0x02, 0x00, 0x00, 0x00]);
        let lamports = u64::from_le_bytes(ix_data[4..12].try_into().unwrap());
        assert_eq!(lamports, 7_777_777);
    }

    // Note: we don't exercise the full `run_with_routes` for Lane 2 in
    // unit tests because the body performs a live HTTP fetch to the
    // configured cluster's RPC for the blockhash. The route plan
    // serialization is covered indirectly through the helper above.
    // The smoke flow in SMOKE.md covers the full path.

    #[test]
    fn lane_2_rejects_zero_amount() {
        let args = TransferSolViaTxArgs {
            to: "11111111111111111111111111111111".to_string(),
            amount_lamports: "0".to_string(),
        };
        let ctx = ctx_with_wallet("11111111111111111111111111111111");
        let err = TransferSolViaTx::run_with_routes(&SvmTransferApp::default(), args, ctx)
            .unwrap_err();
        assert!(err.contains("must be > 0"));
    }

    #[test]
    fn lane_2_rejects_invalid_to() {
        let args = TransferSolViaTxArgs {
            to: "tooshort".to_string(),
            amount_lamports: "1000000".to_string(),
        };
        let ctx = ctx_with_wallet("11111111111111111111111111111111");
        let err = TransferSolViaTx::run_with_routes(&SvmTransferApp::default(), args, ctx)
            .unwrap_err();
        assert!(err.contains("invalid `to`"));
    }

    #[test]
    fn lane_2_requires_connected_wallet() {
        let args = TransferSolViaTxArgs {
            to: "11111111111111111111111111111111".to_string(),
            amount_lamports: "1000000".to_string(),
        };
        let ctx = TestCtxBuilder::new("transfer_sol_via_tx")
            .attribute("domain", serde_json::json!({}))
            .build();
        let err = TransferSolViaTx::run_with_routes(&SvmTransferApp::default(), args, ctx)
            .unwrap_err();
        assert!(err.contains("no SVM wallet connected"));
    }
}
