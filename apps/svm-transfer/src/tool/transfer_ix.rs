//! Lane 1 — `transfer_sol_via_ix`. App composes the System Program
//! transfer ix as JSON; host's `svm_stage_ix` accepts the list and
//! composes the VersionedTransaction at commit time.
//!
//! Pipeline shape (same as Marinade):
//!
//! ```text
//! transfer_sol_via_ix ───┐
//!                        ↓
//!  host::SvmStageIx({instructions: [transfer_ix], description})  .next, bind_as("ix_ids")
//!                        ↓
//!  host::SvmCommitIx({mode: "wallet", version: "legacy"})         .after, awaits("ix_ids")
//!                        ↓
//!  wallet signs + sends — tx sig returned to the LLM
//! ```

use crate::client::SvmTransferApp;
use crate::tool::{
    SYSTEM_PROGRAM_ID, TransferAcct, TransferIx, require_svm_wallet, system_transfer_data,
    validate_base58_address,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use base64::{Engine, engine::general_purpose::STANDARD as B64};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct TransferSolViaIxArgs {
    /// Base58 Solana pubkey of the recipient.
    pub to: String,
    /// Amount of SOL to transfer, in lamports (1 SOL = 1_000_000_000).
    /// Pass as a string to avoid JSON-number precision loss on the
    /// large values some smoke flows might use.
    pub amount_lamports: String,
}

pub(crate) struct TransferSolViaIx;

impl DynAomiTool for TransferSolViaIx {
    type App = SvmTransferApp;
    type Args = TransferSolViaIxArgs;
    const NAME: &'static str = "transfer_sol_via_ix";
    const DESCRIPTION: &'static str = "**Lane 1 smoke** — transfer SOL to `to` from the connected SVM wallet, composing the \
         System Program transfer instruction client-side and emitting it through the canonical \
         `svm_stage_ix` → `svm_commit_ix({mode: \"wallet\"})` route plan. The host composes the \
         VersionedTransaction at commit time. Always emit a one-screen confirmation summary \
         (amount, to, from, cluster) and stop the turn before calling this.";

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

        let ix = TransferIx {
            program_id: SYSTEM_PROGRAM_ID.to_string(),
            accounts: vec![
                TransferAcct {
                    pubkey: wallet.clone(),
                    is_signer: true,
                    is_writable: true,
                },
                TransferAcct {
                    pubkey: args.to.clone(),
                    is_signer: false,
                    is_writable: true,
                },
            ],
            data_base64: B64.encode(system_transfer_data(amount)),
            description: format!(
                "System::transfer {amount} lamports {wallet} → {to}",
                wallet = wallet,
                to = args.to
            ),
        };

        let preview = json!({
            "action_kind": "transfer_sol_via_ix",
            "lane": 1,
            "preview": {
                "amount_lamports": args.amount_lamports,
                "from": wallet,
                "to": args.to,
                "cluster": crate::client::cluster_id(),
            },
            "requires_user_confirmation": true,
            "confirmation_phrase": "confirm",
        });

        build_lane_1_route_plan(
            preview,
            vec![ix],
            format!("svm-transfer Lane 1: {} lamports to {}", amount, args.to),
        )
    }
}

/// Two-node Lane 1 route plan: stage_ix → commit_ix({mode: "wallet"}).
/// Mirrors Marinade's `build_marinade_route_plan` exactly — the only
/// difference is the ix payload kind.
fn build_lane_1_route_plan(
    value: Value,
    instructions: Vec<TransferIx>,
    description: String,
) -> Result<ToolReturn, String> {
    ToolReturn::route(value)
        .next(|next| {
            next.add::<host::SvmStageIx>(json!({
                "instructions": instructions,
                "description": description.clone(),
            }))
            .bind_as("ix_ids")
            .note("Stage the System Program transfer ix for assembly.");
        })
        .after::<host::SvmCommitIx>(json!({
            "mode": "wallet",
            "version": "legacy",
        }))
        .awaits("ix_ids")
        .note("Commit via the connected wallet — signs and broadcasts; tx sig returned.")
        .try_build()
        .map_err(|e| format!("[svm-transfer] Lane 1 route build failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use aomi_sdk::testing::TestCtxBuilder;

    fn ctx_with_wallet(addr: &str) -> DynToolCallCtx {
        TestCtxBuilder::new("transfer_sol_via_ix")
            .attribute(
                "domain",
                serde_json::json!({ "svm": { "address": addr, "cluster": "devnet" } }),
            )
            .build()
    }

    #[test]
    fn lane_1_emits_stage_ix_then_commit_ix() {
        let args = TransferSolViaIxArgs {
            to: "11111111111111111111111111111111".to_string(),
            amount_lamports: "1000000".to_string(),
        };
        let ctx = ctx_with_wallet("11111111111111111111111111111111");
        let ret = TransferSolViaIx::run_with_routes(&SvmTransferApp::default(), args, ctx)
            .expect("route plan builds");
        let serialized = serde_json::to_value(&ret).expect("serialize");
        let routes = serialized
            .get("__aomi_tool_routes")
            .and_then(Value::as_array)
            .expect("routes present");
        let tools: Vec<&str> = routes
            .iter()
            .filter_map(|r| r.get("tool").and_then(Value::as_str))
            .collect();
        assert_eq!(tools, vec!["svm_stage_ix", "svm_commit_ix"]);
    }

    #[test]
    fn lane_1_rejects_zero_amount() {
        let args = TransferSolViaIxArgs {
            to: "11111111111111111111111111111111".to_string(),
            amount_lamports: "0".to_string(),
        };
        let ctx = ctx_with_wallet("11111111111111111111111111111111");
        let err =
            TransferSolViaIx::run_with_routes(&SvmTransferApp::default(), args, ctx).unwrap_err();
        assert!(err.contains("must be > 0"));
    }

    #[test]
    fn lane_1_rejects_invalid_to() {
        let args = TransferSolViaIxArgs {
            to: "tooshort".to_string(),
            amount_lamports: "1000000".to_string(),
        };
        let ctx = ctx_with_wallet("11111111111111111111111111111111");
        let err =
            TransferSolViaIx::run_with_routes(&SvmTransferApp::default(), args, ctx).unwrap_err();
        assert!(err.contains("invalid `to`"));
    }

    #[test]
    fn lane_1_requires_connected_wallet() {
        let args = TransferSolViaIxArgs {
            to: "11111111111111111111111111111111".to_string(),
            amount_lamports: "1000000".to_string(),
        };
        // ctx without the domain.svm.address key.
        let ctx = TestCtxBuilder::new("transfer_sol_via_ix")
            .attribute("domain", serde_json::json!({}))
            .build();
        let err =
            TransferSolViaIx::run_with_routes(&SvmTransferApp::default(), args, ctx).unwrap_err();
        assert!(err.contains("no SVM wallet connected"));
    }
}
