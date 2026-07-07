//! Tool layer for the byreal app.
//!
//! Each submodule covers one byreal product line (perps / spot / lp). The
//! `dyn_aomi_app!` registration in `lib.rs` enumerates every public tool
//! struct from these modules.
//!
//! Cross-cutting helpers ([`ok`], [`validate_confirmation`]) live here and
//! are reused by every tool module so error and response shapes stay
//! consistent across product lines.

pub(crate) mod lp;
pub(crate) mod perps;
pub(crate) mod spot;

use aomi_sdk::*;
use serde::Serialize;
use serde_json::{Value, json};

/// Wrap a tool's response value with the `"source": "byreal"` tag so the
/// LLM can disambiguate provider when multiple read tools' outputs are
/// stitched together.
pub(crate) fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let v = serde_json::to_value(value).map_err(|e| format!("[byreal] response serialize: {e}"))?;
    Ok(match v {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("byreal".to_string()));
            Value::Object(map)
        }
        other => json!({ "source": "byreal", "data": other }),
    })
}

/// Resolve a wallet address: prefer explicit arg, fall back to the host's
/// connected wallet under `domain.<chain>.address`.
///
/// `chain_key` is `"evm"` for Hyperliquid / Arbitrum etc., `"svm"` for Solana.
pub(crate) fn resolve_address(
    arg: Option<String>,
    ctx: &DynToolCallCtx,
    chain_key: &str,
) -> Result<String, String> {
    arg.or_else(|| ctx.attribute_string(&["domain", chain_key, "address"]))
        .ok_or_else(|| {
            format!(
                "[byreal] no {chain_key} address provided and none in context — \
                 pass `wallet` explicitly or connect a {chain_key} wallet",
            )
        })
}

/// Gate every `submit_*` tool behind an explicit `"confirm"` token forwarded
/// from the matching `build_*` preview. The runtime splices the host
/// signature into the args automatically; this check ensures the LLM
/// surfaced the preview to the user first.
pub(crate) fn validate_confirmation(token: Option<&str>) -> Result<(), String> {
    match token {
        Some("confirm") => Ok(()),
        _ => Err(
            "submit_* requires `confirmation: \"confirm\"`. Show the user the build_* preview \
             and obtain explicit go-ahead before submitting."
                .to_string(),
        ),
    }
}

/// Build a `evm_commit_message` route plan: the app emits the typed-data and
/// registers the matched `submit_*` as an `.after()` continuation; the host
/// wallet signs and the runtime splices the signature into the `submit_*`
/// tool args under `signature`.
///
/// Used by [`perps`] (Hyperliquid L1 actions). Solana flows use
/// [`build_venue_commit_routes`] instead.
pub(crate) fn build_evm_signed_routes<Submit: RouteTarget>(
    value: Value,
    typed_data: Value,
    description: String,
    submit_template: Value,
) -> Result<ToolReturn, String> {
    ToolReturn::route(value)
        .next(|next| {
            next.add::<host::EvmCommitMessage>(json!({
                "typed_data": typed_data,
                "description": description,
            }))
            .bind_as("signature");
        })
        .after::<Submit>(submit_template)
        .awaits("signature")
        .note(
            "MANDATORY: Wallet just signed. You MUST call this tool RIGHT NOW. \
             Do NOT generate a text response. Do NOT acknowledge or summarize. \
             Make this function call immediately. \
             \
             CRITICAL — copy every arg EXACTLY as shown. In particular: \
             • `payload` is a short server-issued handle of the form `hpl_<hex>` — \
             copy it verbatim. The host resolves it to the real pre-signed action; \
             a mistyped handle will fail to resolve and the order will not place. \
             • `signature` is opaque — copy verbatim. \
             • `confirmation` must remain the literal string `confirm`. \
             Keep opaque continuation state unchanged.",
        )
        .try_build()
        .map_err(|e| format!("[byreal] route build failed: {e}"))
}

/// Build a venue-broadcast commit route plan: the app emits an unsigned
/// Solana tx (base64 versioned bytes), routes `svm_stage_tx` (staged with
/// `broadcaster: "venue"` — byreal's endpoint submits, never the wallet or
/// the Aomi runtime) then `svm_commit_tx` on the minted `pending_tx_id`,
/// and registers the matched `submit_*` as an `.after()` continuation.
///
/// Who signs is kernel policy on the user's wallet — a human-sync wallet
/// signs in the FE, an autonomous-armed wallet is signed server-side with
/// no FE round-trip — either way the runtime splices the signed bytes into
/// the `submit_*` tool args under `signed_tx`. The app code cannot tell
/// the difference, which is what makes these flows schedulable.
///
/// Used by spot and lp (byreal Solana endpoints).
pub(crate) fn build_venue_commit_routes<Submit: RouteTarget>(
    value: Value,
    unsigned_tx_b64: String,
    description: String,
    submit_template: Value,
) -> Result<ToolReturn, String> {
    ToolReturn::route(value)
        .next(|next| {
            next.add::<host::SvmStageTx>(json!({
                "tx": unsigned_tx_b64,
                "description": description,
                "broadcaster": "venue",
            }));
            next.add::<host::SvmCommitTx>(json!({}))
                .note(
                    "Call with { \"tx_id\": <pending_tx_id> } — the pending_tx_id \
                     returned by the svm_stage_tx step above. No other args.",
                )
                .bind_as("signed_tx");
        })
        .after::<Submit>(submit_template)
        .awaits("signed_tx")
        .note(
            "MANDATORY: The Solana transaction is signed. You MUST call this tool \
             RIGHT NOW. Do NOT generate a text response. Do NOT acknowledge or summarize. \
             Make this function call immediately. Pass all args exactly as shown. \
             Keep opaque continuation state unchanged.",
        )
        .try_build()
        .map_err(|e| format!("[byreal] route build failed: {e}"))
}
