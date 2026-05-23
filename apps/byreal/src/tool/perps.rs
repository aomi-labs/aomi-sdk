use crate::client::ByrealApp;
use crate::client::perps::{
    OrderInputs, build_cancel_action, build_exchange_body, build_order_action,
    build_update_leverage_action, parse_signature, perps_client, prepare_l1_action,
};
use crate::tool::{build_evm_signed_routes, ok, resolve_address, validate_confirmation};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_MARKET_SLIPPAGE_PCT: f64 = 5.0;

// ---------------------------------------------------------------------------
// build_order / submit_order
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct BuildOrderArgs {
    /// Asset ticker (e.g. "BTC", "ETH", "SOL"). Must exist in the Hyperliquid universe.
    pub coin: String,
    /// true for long / buy, false for short / sell.
    pub is_buy: bool,
    /// Order kind: "market" or "limit".
    pub order_kind: String,
    /// Size in coin units (NOT USD). Use `usd_notional / mid_price` if you have a USD target.
    pub sz: f64,
    /// Limit price in USD. Required for "limit" orders. Ignored for "market".
    pub limit_px: Option<f64>,
    /// Current mid price in USD. Required for "market" orders so we can apply slippage.
    pub mid_price: Option<f64>,
    /// Slippage tolerance for market orders, in percent. Default 5.0 (matches the Byreal CLI).
    pub slippage_pct: Option<f64>,
    /// Time-in-force for limit orders: "Gtc" (default), "Ioc", or "Alo".
    pub tif: Option<String>,
    /// Reduce-only flag. Set true when closing a position so you don't accidentally flip side.
    pub reduce_only: Option<bool>,
}

pub(crate) struct BuildOrder;

impl DynAomiTool for BuildOrder {
    type App = ByrealApp;
    type Args = BuildOrderArgs;
    const NAME: &'static str = "byreal_perps_build_order";
    const DESCRIPTION: &'static str = "Build and immediately execute a Hyperliquid perpetual order. Returns a preview and routes directly to `commit_eip712` for the host wallet to sign. The matched `byreal_perps_submit_order` continuation runs after the signature comes back.";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        if args.sz <= 0.0 {
            return Err("[byreal] sz must be > 0".to_string());
        }
        let client = perps_client()?;
        let asset_index = client.lookup_asset(&args.coin)?;

        let kind = args.order_kind.to_ascii_lowercase();
        let (limit_px, tif) = match kind.as_str() {
            "limit" => {
                let px = args
                    .limit_px
                    .ok_or_else(|| "[byreal] limit orders require limit_px".to_string())?;
                if px <= 0.0 {
                    return Err("[byreal] limit_px must be > 0".to_string());
                }
                (px, args.tif.clone().unwrap_or_else(|| "Gtc".to_string()))
            }
            "market" => {
                let mid = args.mid_price.ok_or_else(|| {
                    "[byreal] market orders require mid_price (fetch it from `byreal_perps_get_all_mids` first)".to_string()
                })?;
                if mid <= 0.0 {
                    return Err("[byreal] mid_price must be > 0".to_string());
                }
                let slip = args.slippage_pct.unwrap_or(DEFAULT_MARKET_SLIPPAGE_PCT);
                let factor = if args.is_buy {
                    1.0 + slip / 100.0
                } else {
                    1.0 - slip / 100.0
                };
                (mid * factor, "Ioc".to_string())
            }
            other => {
                return Err(format!(
                    "[byreal] unknown order_kind '{other}', expected 'market' or 'limit'"
                ));
            }
        };

        let reduce_only = args.reduce_only.unwrap_or(false);

        let action = build_order_action(
            &OrderInputs {
                coin: &args.coin,
                is_buy: args.is_buy,
                limit_px,
                sz: args.sz,
                reduce_only,
                tif: tif.clone(),
            },
            client.coin_to_asset()?,
        )?;
        let (action_json, nonce, typed_data) = prepare_l1_action(action, None)?;

        // Bundle action+nonce+vault into one opaque hex blob so the LLM
        // can't accidentally rewrite the 13-digit nonce or corrupt the
        // pre-signed action JSON between build and submit. The submit-side
        // tool decodes this verbatim.
        let payload = SubmissionPayload::encode(action_json.clone(), nonce, None)?;
        let submit_template = serde_json::to_value(&SubmitOrderArgs {
            confirmation: Some("confirm".to_string()),
            payload,
            signature: None,
        })
        .map_err(|e| format!("[byreal] submit template serialize: {e}"))?;

        let preview = json!({
            "action_kind": "order",
            "preview": {
                "coin": args.coin,
                "asset_index": asset_index,
                "is_buy": args.is_buy,
                "side": if args.is_buy { "long" } else { "short" },
                "order_kind": kind,
                "sz": args.sz,
                "limit_px": limit_px,
                "tif": tif,
                "reduce_only": reduce_only,
                "slippage_applied_pct": if kind == "market" {
                    Some(args.slippage_pct.unwrap_or(DEFAULT_MARKET_SLIPPAGE_PCT))
                } else { None },
            },
            "nonce": nonce,
            "submit_args_template": submit_template.clone(),
        });

        let description = format!(
            "Hyperliquid {} {} {} {} @ ${:.4}{}",
            if args.is_buy { "BUY" } else { "SELL" },
            args.sz,
            args.coin,
            kind,
            limit_px,
            if reduce_only { " (reduce-only)" } else { "" },
        );

        build_evm_signed_routes::<SubmitOrder>(preview, typed_data, description, submit_template)
    }
}

/// Decoded shape of the opaque `payload` blob carried through the
/// build → sign → submit chain. Kept private to byreal — the LLM never
/// touches the individual fields, it only forwards the hex-encoded blob.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct SubmissionPayload {
    action: Value,
    nonce: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vault_address: Option<String>,
}

/// Length (hex chars) of the keccak256-based checksum suffix appended to
/// every encoded payload. 8 hex chars = first 4 bytes of the digest = 32
/// bits of collision resistance — enough to catch single-character drops or
/// substitutions the LLM occasionally introduces while transcribing a
/// long opaque blob, with a sub-microsecond verify cost.
const PAYLOAD_CHECKSUM_HEX_LEN: usize = 8;

impl SubmissionPayload {
    /// Encode `(action, nonce, vault_address)` into `<hex(json)><checksum>`
    /// so the LLM-transcribed blob has end-to-end integrity protection. If
    /// even one hex char is dropped or substituted, the checksum no longer
    /// matches and `decode` returns a clean "payload was modified" error
    /// instead of letting a corrupted action through to the exchange where
    /// it'd surface as an opaque 422 or a "User does not exist" (signature
    /// recovers to a wrong address).
    fn encode(action: Value, nonce: u64, vault_address: Option<String>) -> Result<String, String> {
        let payload = Self {
            action,
            nonce,
            vault_address,
        };
        let bytes = serde_json::to_vec(&payload)
            .map_err(|e| format!("[byreal] payload encode failed: {e}"))?;
        let mut out = hex::encode(&bytes);
        out.push_str(&payload_checksum_hex(&bytes));
        Ok(out)
    }

    fn decode(s: &str) -> Result<Self, String> {
        let trimmed_owned = s
            .trim()
            .trim_matches('"')
            .chars()
            .filter(|c| !c.is_ascii_whitespace())
            .collect::<String>();
        let trimmed = trimmed_owned
            .strip_prefix("0x")
            .or_else(|| trimmed_owned.strip_prefix("0X"))
            .unwrap_or(&trimmed_owned);
        if trimmed.len() <= PAYLOAD_CHECKSUM_HEX_LEN {
            return Err(format!(
                "[byreal] payload too short ({} chars) — forward the full blob verbatim",
                trimmed.len()
            ));
        }
        let split_at = trimmed.len() - PAYLOAD_CHECKSUM_HEX_LEN;
        let (body_hex, checksum_hex) = trimmed.split_at(split_at);
        let bytes = hex::decode(body_hex)
            .map_err(|e| format!("[byreal] payload not valid hex — forward verbatim: {e}"))?;
        let expected = payload_checksum_hex(&bytes);
        if !checksum_hex.eq_ignore_ascii_case(&expected) {
            return Err(format!(
                "[byreal] payload checksum mismatch — the blob was modified in transit. \
                 Forward the value from the build_* preview CHARACTER BY CHARACTER \
                 without inserting, dropping, or substituting any hex digit. \
                 (expected suffix `{expected}`, got `{checksum_hex}`)"
            ));
        }
        serde_json::from_slice(&bytes)
            .map_err(|e| format!("[byreal] payload decode failed: {e}"))
    }
}

fn payload_checksum_hex(bytes: &[u8]) -> String {
    use ethers::utils::keccak256;
    let digest = keccak256(bytes);
    hex::encode(&digest[..PAYLOAD_CHECKSUM_HEX_LEN / 2])
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SubmitOrderArgs {
    /// Must be the literal string "confirm". Forwarded from the build_* preview.
    pub confirmation: Option<String>,
    /// OPAQUE pre-signed submission payload. Hex-encoded JSON blob from the
    /// `build_*` preview's `submit_args_template.payload`. Forward CHARACTER
    /// BY CHARACTER — DO NOT modify, decode, regenerate, or trim. The EIP-712
    /// signature was computed against the exact bytes inside; mutating this
    /// invalidates the wallet's signature.
    pub payload: String,
    /// EIP-712 signature (65-byte hex). Filled in by the host wallet via `commit_eip712`.
    pub signature: Option<String>,
}

pub(crate) struct SubmitOrder;

impl DynAomiTool for SubmitOrder {
    type App = ByrealApp;
    type Args = SubmitOrderArgs;
    const NAME: &'static str = "byreal_perps_submit_order";
    const DESCRIPTION: &'static str = "Submit a Hyperliquid order that was previously prepared by `byreal_perps_build_order` and signed via `commit_eip712`. Pass the opaque `payload` blob verbatim from the build preview; the `signature` field is filled in by the runtime — never invent one.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        validate_confirmation(args.confirmation.as_deref())?;
        let sig_hex = args.signature.as_deref().ok_or_else(|| {
            "[byreal] signature missing — wait for commit_eip712 callback".to_string()
        })?;
        let sig = parse_signature(sig_hex)?;
        let decoded = SubmissionPayload::decode(&args.payload)?;
        let body = build_exchange_body(
            decoded.action,
            decoded.nonce,
            &sig,
            decoded.vault_address.as_deref(),
        );
        ok(perps_client()?.post_exchange(body)?)
    }
}

// ---------------------------------------------------------------------------
// build_cancel / submit_cancel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct BuildCancelArgs {
    /// Asset ticker for the order being canceled.
    pub coin: String,
    /// Order ID returned when the order was placed (look it up via `byreal_perps_get_open_orders` if unknown).
    pub oid: u64,
}

pub(crate) struct BuildCancel;

impl DynAomiTool for BuildCancel {
    type App = ByrealApp;
    type Args = BuildCancelArgs;
    const NAME: &'static str = "byreal_perps_build_cancel";
    const DESCRIPTION: &'static str = "Build (do not submit) a cancel for a single resting order. Returns a preview and a routed `commit_eip712` step the host wallet signs.";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let client = perps_client()?;
        let asset_index = client.lookup_asset(&args.coin)?;
        let action = build_cancel_action(asset_index, args.oid);
        let (action_json, nonce, typed_data) = prepare_l1_action(action, None)?;

        let payload = SubmissionPayload::encode(action_json.clone(), nonce, None)?;
        let submit_template = serde_json::to_value(&SubmitCancelArgs {
            confirmation: Some("confirm".to_string()),
            payload,
            signature: None,
        })
        .map_err(|e| format!("[byreal] submit template serialize: {e}"))?;

        let preview = json!({
            "action_kind": "cancel",
            "preview": {
                "coin": args.coin,
                "asset_index": asset_index,
                "oid": args.oid,
            },
            "nonce": nonce,
            "submit_args_template": submit_template.clone(),
        });

        let description = format!("Hyperliquid CANCEL {} order #{}", args.coin, args.oid);

        build_evm_signed_routes::<SubmitCancel>(preview, typed_data, description, submit_template)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SubmitCancelArgs {
    pub confirmation: Option<String>,
    /// OPAQUE pre-signed submission payload. Forward verbatim — see
    /// [`SubmitOrderArgs::payload`] for the contract.
    pub payload: String,
    pub signature: Option<String>,
}

pub(crate) struct SubmitCancel;

impl DynAomiTool for SubmitCancel {
    type App = ByrealApp;
    type Args = SubmitCancelArgs;
    const NAME: &'static str = "byreal_perps_submit_cancel";
    const DESCRIPTION: &'static str = "Submit a Hyperliquid cancel that was prepared by `byreal_perps_build_cancel` and signed via `commit_eip712`. Forward the opaque `payload` blob verbatim from the build preview.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        validate_confirmation(args.confirmation.as_deref())?;
        let sig_hex = args
            .signature
            .as_deref()
            .ok_or_else(|| "[byreal] signature missing".to_string())?;
        let sig = parse_signature(sig_hex)?;
        let decoded = SubmissionPayload::decode(&args.payload)?;
        let body = build_exchange_body(
            decoded.action,
            decoded.nonce,
            &sig,
            decoded.vault_address.as_deref(),
        );
        ok(perps_client()?.post_exchange(body)?)
    }
}

// ---------------------------------------------------------------------------
// build_update_leverage / submit_update_leverage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct BuildUpdateLeverageArgs {
    /// Asset ticker.
    pub coin: String,
    /// Target leverage (1..=max for the asset). Caps vary per asset; check `byreal_perps_get_meta`.
    pub leverage: u32,
    /// true = cross margin, false = isolated margin.
    pub is_cross: bool,
}

pub(crate) struct BuildUpdateLeverage;

impl DynAomiTool for BuildUpdateLeverage {
    type App = ByrealApp;
    type Args = BuildUpdateLeverageArgs;
    const NAME: &'static str = "byreal_perps_build_update_leverage";
    const DESCRIPTION: &'static str = "Build (do not submit) a leverage update for one asset. Apply this BEFORE opening a position so the order opens at the intended leverage.";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        if args.leverage == 0 {
            return Err("[byreal] leverage must be >= 1".to_string());
        }
        let client = perps_client()?;
        let asset_index = client.lookup_asset(&args.coin)?;
        let action = build_update_leverage_action(asset_index, args.is_cross, args.leverage);
        let (action_json, nonce, typed_data) = prepare_l1_action(action, None)?;

        let payload = SubmissionPayload::encode(action_json.clone(), nonce, None)?;
        let submit_template = serde_json::to_value(&SubmitUpdateLeverageArgs {
            confirmation: Some("confirm".to_string()),
            payload,
            signature: None,
        })
        .map_err(|e| format!("[byreal] submit template serialize: {e}"))?;

        let preview = json!({
            "action_kind": "update_leverage",
            "preview": {
                "coin": args.coin,
                "asset_index": asset_index,
                "leverage": args.leverage,
                "margin_mode": if args.is_cross { "cross" } else { "isolated" },
            },
            "nonce": nonce,
            "submit_args_template": submit_template.clone(),
        });

        let description = format!(
            "Hyperliquid SET LEVERAGE {} = {}x ({})",
            args.coin,
            args.leverage,
            if args.is_cross { "cross" } else { "isolated" },
        );

        build_evm_signed_routes::<SubmitUpdateLeverage>(
            preview,
            typed_data,
            description,
            submit_template,
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SubmitUpdateLeverageArgs {
    pub confirmation: Option<String>,
    /// OPAQUE pre-signed submission payload. Forward verbatim — see
    /// [`SubmitOrderArgs::payload`] for the contract.
    pub payload: String,
    pub signature: Option<String>,
}

pub(crate) struct SubmitUpdateLeverage;

impl DynAomiTool for SubmitUpdateLeverage {
    type App = ByrealApp;
    type Args = SubmitUpdateLeverageArgs;
    const NAME: &'static str = "byreal_perps_submit_update_leverage";
    const DESCRIPTION: &'static str = "Submit a Hyperliquid leverage update prepared by `byreal_perps_build_update_leverage` and signed via `commit_eip712`. Forward the opaque `payload` blob verbatim from the build preview.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        validate_confirmation(args.confirmation.as_deref())?;
        let sig_hex = args
            .signature
            .as_deref()
            .ok_or_else(|| "[byreal] signature missing".to_string())?;
        let sig = parse_signature(sig_hex)?;
        let decoded = SubmissionPayload::decode(&args.payload)?;
        let body = build_exchange_body(
            decoded.action,
            decoded.nonce,
            &sig,
            decoded.vault_address.as_deref(),
        );
        ok(perps_client()?.post_exchange(body)?)
    }
}

// ===========================================================================
// READ TOOLS — all hit the public /info endpoint, no signing required.
// ===========================================================================

fn resolve_user(arg: Option<String>, ctx: &DynToolCallCtx) -> Result<String, String> {
    resolve_address(arg, ctx, "evm")
}

// -- byreal_get_meta -------------------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetMetaArgs {}

pub(crate) struct GetMeta;

impl DynAomiTool for GetMeta {
    type App = ByrealApp;
    type Args = GetMetaArgs;
    const NAME: &'static str = "byreal_perps_get_meta";
    const DESCRIPTION: &'static str = "List every tradeable Hyperliquid perpetual asset along with its `szDecimals` (size precision) and `maxLeverage`. Call this once per session to discover the asset universe and to look up size precision before placing an order.";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(perps_client()?.get_meta()?)
    }
}

// -- byreal_get_all_mids ---------------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetAllMidsArgs {}

pub(crate) struct GetAllMids;

impl DynAomiTool for GetAllMids {
    type App = ByrealApp;
    type Args = GetAllMidsArgs;
    const NAME: &'static str = "byreal_perps_get_all_mids";
    const DESCRIPTION: &'static str = "Get the current mid-price for every listed asset, returned as a `{coin: price_string}` map. Use this to convert a USD notional into a coin size before calling `byreal_perps_build_order`, or to apply slippage to a market order.";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(perps_client()?.get_all_mids()?)
    }
}

// -- byreal_get_l2_book ----------------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetL2BookArgs {
    /// Asset ticker (e.g. "BTC", "ETH", "SOL").
    pub coin: String,
}

pub(crate) struct GetL2Book;

impl DynAomiTool for GetL2Book {
    type App = ByrealApp;
    type Args = GetL2BookArgs;
    const NAME: &'static str = "byreal_perps_get_l2_book";
    const DESCRIPTION: &'static str = "Snapshot the L2 order book for one asset (top bids and asks with px/sz/n). Use to inspect liquidity depth or pick a limit price near top-of-book.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(perps_client()?.get_l2_book(&args.coin)?)
    }
}

// -- byreal_get_account_state ---------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetAccountStateArgs {
    /// Ethereum-style address (0x...). Optional — falls back to the connected wallet in context.
    pub user: Option<String>,
}

pub(crate) struct GetAccountState;

impl DynAomiTool for GetAccountState {
    type App = ByrealApp;
    type Args = GetAccountStateArgs;
    const NAME: &'static str = "byreal_perps_get_account_state";
    const DESCRIPTION: &'static str = "Get an address's perp account state: margin summary (account value, total margin used, withdrawable), every open position with size/entry/leverage/liquidation/PnL, and cross-margin parameters. Call before opening a position to verify free margin, and after to confirm the new position.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let user = resolve_user(args.user, &ctx)?;
        ok(perps_client()?.get_account_state(&user)?)
    }
}

// -- byreal_get_open_orders -----------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetOpenOrdersArgs {
    /// Ethereum-style address (0x...). Optional — falls back to the connected wallet in context.
    pub user: Option<String>,
}

pub(crate) struct GetOpenOrders;

impl DynAomiTool for GetOpenOrders {
    type App = ByrealApp;
    type Args = GetOpenOrdersArgs;
    const NAME: &'static str = "byreal_perps_get_open_orders";
    const DESCRIPTION: &'static str = "List every resting (unfilled) order for an address: coin, side, size, limit price, order ID, timestamp. Use to find the `oid` for `byreal_perps_build_cancel`, or to confirm a freshly-placed limit order is on the book.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let user = resolve_user(args.user, &ctx)?;
        ok(perps_client()?.get_open_orders(&user)?)
    }
}

// -- byreal_get_user_fills ------------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetUserFillsArgs {
    /// Ethereum-style address (0x...). Optional — falls back to the connected wallet in context.
    pub user: Option<String>,
}

pub(crate) struct GetUserFills;

impl DynAomiTool for GetUserFills {
    type App = ByrealApp;
    type Args = GetUserFillsArgs;
    const NAME: &'static str = "byreal_perps_get_user_fills";
    const DESCRIPTION: &'static str = "Recent trade fill history for an address: each fill's coin, side, px, sz, fee, closedPnl, oid, txHash, timestamp. Use to review what just executed or to compute realised PnL.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let user = resolve_user(args.user, &ctx)?;
        ok(perps_client()?.get_user_fills(&user)?)
    }
}

// -- byreal_get_funding_history -------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetFundingHistoryArgs {
    /// Asset ticker (e.g. "BTC").
    pub coin: String,
    /// Start timestamp in milliseconds (Unix epoch).
    pub start_time: u64,
    /// End timestamp in milliseconds. Optional — defaults to now.
    pub end_time: Option<u64>,
}

pub(crate) struct GetFundingHistory;

impl DynAomiTool for GetFundingHistory {
    type App = ByrealApp;
    type Args = GetFundingHistoryArgs;
    const NAME: &'static str = "byreal_perps_get_funding_history";
    const DESCRIPTION: &'static str = "Historical funding-rate snapshots for an asset over a time window. The `fundingRate` field is the 8-hour rate; settlement happens hourly at 1/8 of the displayed rate. Annualised ≈ rate × 3 × 365.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(perps_client()?.get_funding_history(&args.coin, args.start_time, args.end_time)?)
    }
}

// -- byreal_get_candles ---------------------------------------------------

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetCandlesArgs {
    /// Asset ticker (e.g. "BTC").
    pub coin: String,
    /// Candle interval: "1m", "5m", "15m", "1h", "4h", "1d".
    pub interval: String,
    /// Start timestamp in milliseconds (Unix epoch).
    pub start_time: u64,
    /// End timestamp in milliseconds.
    pub end_time: u64,
}

pub(crate) struct GetCandles;

impl DynAomiTool for GetCandles {
    type App = ByrealApp;
    type Args = GetCandlesArgs;
    const NAME: &'static str = "byreal_perps_get_candles";
    const DESCRIPTION: &'static str = "OHLCV candle data for one asset over a time window at the given interval. Use for charting context or short-horizon technical reads. Supported intervals: 1m, 5m, 15m, 1h, 4h, 1d.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(perps_client()?.get_candles(
            &args.coin,
            &args.interval,
            args.start_time,
            args.end_time,
        )?)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_confirmation_requires_confirm_token() {
        assert!(validate_confirmation(Some("confirm")).is_ok());
        assert!(validate_confirmation(Some("yes")).is_err());
        assert!(validate_confirmation(None).is_err());
    }

    #[test]
    fn submission_payload_round_trip() {
        let encoded = SubmissionPayload::encode(
            json!({"type": "order", "asset": 0}),
            1779522523508_u64,
            None,
        )
        .expect("encode");
        // Hex output, no JSON braces leaking out — that's the whole point.
        assert!(encoded.chars().all(|c| c.is_ascii_hexdigit()));
        let back = SubmissionPayload::decode(&encoded).expect("decode");
        assert_eq!(back.nonce, 1779522523508_u64);
        assert_eq!(back.action["type"], "order");
        assert!(back.vault_address.is_none());
    }

    #[test]
    fn submission_payload_accepts_0x_prefix() {
        let encoded = SubmissionPayload::encode(json!({"a": 1}), 42, None).unwrap();
        let with_prefix = format!("0x{encoded}");
        SubmissionPayload::decode(&with_prefix).expect("0x prefix tolerated");
    }

    #[test]
    fn submission_payload_rejects_garbage() {
        assert!(SubmissionPayload::decode("not-hex-data").is_err());
        // Valid hex but no checksum, and not valid JSON anyway
        assert!(SubmissionPayload::decode("ffeeddcc").is_err());
    }

    #[test]
    fn submission_payload_detects_single_char_mutation() {
        // Regression: the LLM occasionally drops or substitutes ONE hex char
        // while transcribing a long opaque blob (e.g. `tif` → `if`). Without
        // the checksum suffix that would slip through as a corrupted but
        // still-valid-JSON action, and the exchange would reject with an
        // opaque 422 / wrong-signer error. The checksum makes mutations
        // surface as a clean local "payload was modified" error.
        let encoded =
            SubmissionPayload::encode(json!({"type": "order"}), 1779547086366_u64, None).unwrap();
        // Drop a single hex char from the BODY (before the checksum suffix).
        // Choose a position safely inside the body so the suffix length stays
        // intact — the decode then computes the checksum on slightly-different
        // bytes and rejects.
        let mut mutated = encoded.clone();
        mutated.remove(20);
        // Pad back so the checksum suffix lands at the right position again,
        // emulating "LLM kept length but flipped a char somewhere".
        mutated.insert(20, '0');
        let err = SubmissionPayload::decode(&mutated).unwrap_err();
        assert!(
            err.contains("checksum mismatch") || err.contains("not valid hex"),
            "expected checksum/hex error, got: {err}"
        );
    }

    #[test]
    fn submit_order_args_round_trip() {
        let payload =
            SubmissionPayload::encode(json!({"type": "order"}), 12345, None).expect("encode");
        let args = SubmitOrderArgs {
            confirmation: Some("confirm".to_string()),
            payload: payload.clone(),
            signature: Some("0xdead".to_string()),
        };
        let v = serde_json::to_value(&args).unwrap();
        assert_eq!(v["confirmation"], "confirm");
        assert_eq!(v["payload"], payload);
        assert_eq!(v["signature"], "0xdead");

        let back: SubmitOrderArgs = serde_json::from_value(v).unwrap();
        let decoded = SubmissionPayload::decode(&back.payload).unwrap();
        assert_eq!(decoded.nonce, 12345);
    }
}
