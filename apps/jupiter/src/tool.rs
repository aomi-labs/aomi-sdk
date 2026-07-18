//! Tool layer for the Jupiter swap aggregator.
//!
//! Two tools: `jupiter_get_quote` (preview a route) and `jupiter_swap`
//! (execute). The write re-quotes internally, exchanges the fresh quote for the
//! `swapTransaction` blob, and emits the Lane-2 route plan:
//!
//! ```text
//! jupiter_swap → svm_stage_tx({tx}) → svm_commit_tx({tx_id})
//! ```
//!
//! Who signs is kernel policy on the wallet; who broadcasts is the staged
//! broadcaster (default `wallet`, `aomi` for autonomous runs). Commit signs the
//! Jupiter-built tx and broadcasts it — there is no submit step. The app never
//! touches the tx bytes: Jupiter's `swapTransaction` is staged verbatim.

use serde::Deserialize;
use serde_json::{Value, json};

use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;

use crate::client::{JupiterApp, jupiter_client};

const DEFAULT_SLIPPAGE_BPS: u32 = 50;

fn ok<T: serde::Serialize>(value: T) -> Result<Value, String> {
    let v =
        serde_json::to_value(value).map_err(|e| format!("[jupiter] response serialize: {e}"))?;
    Ok(match v {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("jupiter".to_string()));
            Value::Object(map)
        }
        other => json!({ "source": "jupiter", "data": other }),
    })
}

fn resolve_wallet(arg: Option<String>, ctx: &DynToolCallCtx) -> Result<String, String> {
    arg.or_else(|| ctx.attribute_string(&["domain", "svm", "address"]))
        .ok_or_else(|| {
            "[jupiter] no SVM wallet — pass `wallet` explicitly or connect a Solana wallet"
                .to_string()
        })
}

/// `SOL` is native; `wrapAndUnwrapSol` should be on when either side is SOL so
/// Jupiter injects the WSOL wrap/unwrap ixs.
const SOL_MINT: &str = "So11111111111111111111111111111111111111112";

fn wrap_sol(input_mint: &str, output_mint: &str) -> bool {
    input_mint == SOL_MINT || output_mint == SOL_MINT
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct QuoteArgs {
    /// Mint address of the token you're selling.
    pub input_mint: String,
    /// Mint address of the token you're buying.
    pub output_mint: String,
    /// Amount in atomic units. For `swap_mode` "ExactIn" this is the input
    /// amount; for "ExactOut" it's the desired output amount. (USDC has 6
    /// decimals: 10 USDC = "10000000"; SOL has 9.)
    pub amount: String,
    /// Slippage tolerance in bps. Default 50 (0.5%).
    pub slippage_bps: Option<u32>,
    /// "ExactIn" (default — fixed input) or "ExactOut" (fixed output target).
    pub swap_mode: Option<String>,
}

pub(crate) struct GetQuote;

impl DynAomiTool for GetQuote {
    type App = JupiterApp;
    type Args = QuoteArgs;
    const NAME: &'static str = "jupiter_get_quote";
    const DESCRIPTION: &'static str = "Preview the best Jupiter route to swap `amount` of `input_mint` into `output_mint` across all Solana liquidity — returns the estimated output, price impact, and the venues on the route. Use this to show the user numbers before `jupiter_swap`. Amounts are in atomic units.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let mode = args.swap_mode.as_deref().unwrap_or("ExactIn");
        ok(jupiter_client()?.quote(
            &args.input_mint,
            &args.output_mint,
            &args.amount,
            args.slippage_bps.unwrap_or(DEFAULT_SLIPPAGE_BPS),
            mode,
        )?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct SwapArgs {
    /// Mint address of the token you're selling.
    pub input_mint: String,
    /// Mint address of the token you're buying.
    pub output_mint: String,
    /// Amount in atomic units (see `swap_mode`).
    pub amount: String,
    /// Slippage tolerance in bps. Default 50 (0.5%).
    pub slippage_bps: Option<u32>,
    /// "ExactIn" (default) or "ExactOut".
    pub swap_mode: Option<String>,
}

pub(crate) struct Swap;

impl DynAomiTool for Swap {
    type App = JupiterApp;
    type Args = SwapArgs;
    const NAME: &'static str = "jupiter_swap";
    const DESCRIPTION: &'static str = "Execute a best-execution swap via Jupiter across all Solana liquidity. Re-quotes internally for a fresh route, builds the swap transaction, and stages it for signing via svm_stage_tx → svm_commit_tx. Surface a confirmation summary (in/out amounts, price impact, slippage, route) and get explicit go-ahead BEFORE calling — unless the conversation is pre-authorized.";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let wallet = resolve_wallet(None, &ctx)?;
        let mode = args.swap_mode.as_deref().unwrap_or("ExactIn");
        let slippage = args.slippage_bps.unwrap_or(DEFAULT_SLIPPAGE_BPS);
        let client = jupiter_client()?;

        // Fresh route — a stale quote can fail once the referenced slot ages out.
        let quote = client.quote(
            &args.input_mint,
            &args.output_mint,
            &args.amount,
            slippage,
            mode,
        )?;
        let swap = client.swap(
            &quote,
            &wallet,
            wrap_sol(&args.input_mint, &args.output_mint),
        )?;
        let blob = swap
            .get("swapTransaction")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                "[jupiter] /swap returned no swapTransaction — route may have expired, retry"
                    .to_string()
            })?
            .to_string();

        let preview = json!({
            "action_kind": "jupiter_swap",
            "wallet": wallet,
            "input": { "mint": args.input_mint, "amount": args.amount },
            "output_mint": args.output_mint,
            "swap_mode": mode,
            "slippage_bps": slippage,
            "in_amount": quote.get("inAmount").cloned().unwrap_or(Value::Null),
            "out_amount": quote.get("outAmount").cloned().unwrap_or(Value::Null),
            "price_impact_pct": quote.get("priceImpactPct").cloned().unwrap_or(Value::Null),
            "route": route_labels(&quote),
            "source": "jupiter",
        });

        ToolReturn::route(preview)
            .next(|next| {
                next.add::<host::SvmStageTx>(json!({
                    "tx": blob,
                    "description": format!(
                        "jupiter: swap {} of {} → {}",
                        args.amount, args.input_mint, args.output_mint
                    ),
                    "kind": "jupiter.swap",
                }))
                .bind_as("tx_id")
                .note("Stage the Jupiter-built swap transaction.");
            })
            .after::<host::SvmCommitTx>(json!({}))
            .awaits("tx_id")
            .note(
                "MANDATORY: call svm_commit_tx RIGHT NOW with the staged tx_id. \
                 Do NOT generate a text response first. Commit signs under kernel \
                 policy and broadcasts — there is no separate submit step.",
            )
            .try_build()
            .map_err(|e| format!("[jupiter] route build failed: {e}"))
    }
}

/// The venue labels on a quote's route, for a compact confirmation summary.
fn route_labels(quote: &Value) -> Vec<String> {
    quote
        .get("routePlan")
        .and_then(Value::as_array)
        .map(|steps| {
            steps
                .iter()
                .filter_map(|s| s.pointer("/swapInfo/label").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aomi_sdk::testing::TestCtxBuilder;

    #[test]
    fn resolve_wallet_prefers_arg_then_ctx() {
        let ctx = TestCtxBuilder::new("jupiter_swap")
            .attribute(
                "domain",
                json!({ "svm": { "address": "CtxW", "cluster": "solana:mainnet" } }),
            )
            .build();
        assert_eq!(resolve_wallet(Some("ArgW".into()), &ctx).unwrap(), "ArgW");
        assert_eq!(resolve_wallet(None, &ctx).unwrap(), "CtxW");
        let empty = TestCtxBuilder::new("t")
            .attribute("domain", json!({}))
            .build();
        assert!(resolve_wallet(None, &empty).is_err());
    }

    #[test]
    fn wrap_sol_when_either_side_is_native() {
        let usdc = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
        assert!(wrap_sol(SOL_MINT, usdc));
        assert!(wrap_sol(usdc, SOL_MINT));
        assert!(!wrap_sol(usdc, usdc));
    }

    #[test]
    fn route_labels_pulls_venue_names() {
        let quote = json!({
            "routePlan": [
                { "swapInfo": { "label": "Riptide" } },
                { "swapInfo": { "label": "Orca" } }
            ]
        });
        assert_eq!(route_labels(&quote), vec!["Riptide", "Orca"]);
        assert!(route_labels(&json!({})).is_empty());
    }
}
