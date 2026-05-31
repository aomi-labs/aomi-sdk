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

/// Build a `commit_eip712` route plan: app emits the typed-data + a
/// continuation, host wallet signs, runtime splices the signature into the
/// `submit_*` tool args under `master_signature`.
///
/// Used by [`perps`] (Hyperliquid L1 actions). Solana flows use
/// [`build_solana_signed_routes`] instead.
pub(crate) fn build_evm_signed_routes<Submit: RouteTarget>(
    value: Value,
    typed_data: Value,
    description: String,
    submit_template: Value,
) -> Result<ToolReturn, String> {
    ToolReturn::route(value)
        .next(|next| {
            next.add::<host::CommitEip712>(json!({
                "typed_data": typed_data,
                "description": description,
            }))
            .bind_as("master_signature")
            .note("Wait for explicit user confirmation, then sign this Hyperliquid action with the master wallet.");
        })
        .after::<Submit>(submit_template)
        .awaits("master_signature")
        .note("Wallet signed — submit the action to Hyperliquid.")
        .try_build()
        .map_err(|e| format!("[byreal] route build failed: {e}"))
}

/// Build a `svm_sign_tx` route plan: app emits an unsigned Solana tx
/// (base64 versioned bytes) + a continuation, host wallet signs, runtime
/// splices the signed bytes into the `submit_*` tool args under
/// `signed_tx`.
///
/// Used by spot and lp (byreal Solana endpoints).
#[allow(dead_code)] // wired up in Stage 3; kept here so perps + spot share the same shape.
pub(crate) fn build_solana_signed_routes<Submit: RouteTarget>(
    value: Value,
    unsigned_tx_b64: String,
    description: String,
    submit_template: Value,
) -> Result<ToolReturn, String> {
    ToolReturn::route(value)
        .next(|next| {
            next.add::<host::SvmSignTx>(json!({
                "unsigned_tx": unsigned_tx_b64,
                "description": description,
            }))
            .bind_as("signed_tx")
            .note("Wait for explicit user confirmation, then sign this Solana transaction with the connected wallet.");
        })
        .after::<Submit>(submit_template)
        .awaits("signed_tx")
        .note("Wallet signed — submit the signed transaction to byreal.")
        .try_build()
        .map_err(|e| format!("[byreal] route build failed: {e}"))
}
