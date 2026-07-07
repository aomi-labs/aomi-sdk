//! `byreal_spot_*` tools — byreal CLMM / RFQ swap surface on Solana.
//!
//! Reads hit byreal's HTTP API directly, no signing. The single write pair
//! (`build_swap` / `submit_swap`) routes through `host::SvmStageTx` →
//! `host::SvmCommitTx` (staged `broadcaster: "venue"`): the quote response
//! carries the unsigned base64 versioned tx, the kernel routes who signs
//! from the wallet's authorization, and `submit_swap` forwards the signed
//! bytes to byreal's AMM-or-RFQ submission endpoint depending on the
//! quote's `routerType`.

use crate::client::ByrealApp;
use crate::client::spot::spot_client;
use crate::tool::{build_venue_commit_routes, ok, resolve_address, validate_confirmation};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_PAGE_SIZE: u32 = 20;
const DEFAULT_SLIPPAGE_BPS: u32 = 100; // 1% — matches byreal's frontend default

// ===========================================================================
// READ TOOLS
// ===========================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetPoolsArgs {
    /// Page number (1-indexed). Default 1.
    pub page: Option<u32>,
    /// Items per page. Default 20.
    pub page_size: Option<u32>,
    /// Sort field: "tvl" (default), "volumeUsd24h", "feeApr24h", "incentiveApr".
    pub sort_field: Option<String>,
    /// Sort order: "desc" (default) or "asc".
    pub sort_type: Option<String>,
    /// Filter to a specific category, e.g. "concentrated".
    pub category: Option<String>,
    /// Filter by status (numeric code; check byreal docs for current values).
    pub status: Option<String>,
    /// Look up one specific pool by address.
    pub pool_address: Option<String>,
}

pub(crate) struct GetPools;

impl DynAomiTool for GetPools {
    type App = ByrealApp;
    type Args = GetPoolsArgs;
    const NAME: &'static str = "byreal_spot_get_pools";
    const DESCRIPTION: &'static str = "List byreal CLMM pools on Solana with TVL, 24h volume, fee/incentive APR, and current price ranges. Use to discover trading pairs or find candidates for LP deployment. Default sort is by TVL desc.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(spot_client()?.list_pools(
            args.page.unwrap_or(1),
            args.page_size.unwrap_or(DEFAULT_PAGE_SIZE),
            args.sort_field.as_deref().unwrap_or("tvl"),
            args.sort_type.as_deref().unwrap_or("desc"),
            args.category.as_deref(),
            args.status.as_deref(),
            args.pool_address.as_deref(),
        )?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetPoolArgs {
    /// Pool address (Solana public key).
    pub pool_address: String,
}

pub(crate) struct GetPool;

impl DynAomiTool for GetPool {
    type App = ByrealApp;
    type Args = GetPoolArgs;
    const NAME: &'static str = "byreal_spot_get_pool";
    const DESCRIPTION: &'static str = "Detailed view of one byreal pool: token pair, current tick, fee tier, TVL, volume, full APR breakdown (trading + incentive), reward schedule. Use after `byreal_spot_get_pools` to inspect a candidate.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(spot_client()?.get_pool(&args.pool_address)?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetKlinesArgs {
    /// Pool address.
    pub pool_address: String,
    /// Candle interval: "1m", "5m", "15m", "1h", "4h", "1d".
    pub kline_type: String,
    /// Start timestamp in milliseconds.
    pub start_time: u64,
    /// End timestamp in milliseconds.
    pub end_time: u64,
    /// Optional: anchor klines to one side of the pair (token mint).
    pub token_address: Option<String>,
}

pub(crate) struct GetKlines;

impl DynAomiTool for GetKlines {
    type App = ByrealApp;
    type Args = GetKlinesArgs;
    const NAME: &'static str = "byreal_spot_get_klines";
    const DESCRIPTION: &'static str = "OHLCV candle data for a byreal pool. Use to backtest LP-strategy hypotheses (e.g. \"would tick range X have stayed in-range over the last 7 days?\") or for short-horizon technical reads.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(spot_client()?.get_klines(
            &args.pool_address,
            &args.kline_type,
            args.start_time,
            args.end_time,
            args.token_address.as_deref(),
        )?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetTokensArgs {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    /// Sort field: "volumeUsd24h" (default), "marketCap", "priceChange24h".
    pub sort_field: Option<String>,
    /// Sort order: "desc" (default) or "asc".
    pub sort: Option<String>,
    /// Free-text search over token symbol / name.
    pub search_key: Option<String>,
    pub category: Option<String>,
    pub status: Option<String>,
}

pub(crate) struct GetTokens;

impl DynAomiTool for GetTokens {
    type App = ByrealApp;
    type Args = GetTokensArgs;
    const NAME: &'static str = "byreal_spot_get_tokens";
    const DESCRIPTION: &'static str = "Browse byreal-listed tokens with current price, 24h volume, market cap, price change. Use for token discovery or to look up the mint address before quoting a swap.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(spot_client()?.list_tokens(
            args.page.unwrap_or(1),
            args.page_size.unwrap_or(DEFAULT_PAGE_SIZE),
            args.sort_field.as_deref().unwrap_or("volumeUsd24h"),
            args.sort.as_deref().unwrap_or("desc"),
            args.search_key.as_deref(),
            args.category.as_deref(),
            args.status.as_deref(),
        )?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetTokenPricesArgs {
    /// One or more Solana mint addresses to look up.
    pub mints: Vec<String>,
}

pub(crate) struct GetTokenPrices;

impl DynAomiTool for GetTokenPrices {
    type App = ByrealApp;
    type Args = GetTokenPricesArgs;
    const NAME: &'static str = "byreal_spot_get_token_prices";
    const DESCRIPTION: &'static str = "Current spot prices (USD) for a list of Solana mint addresses. Cheaper than `byreal_spot_get_tokens` when you already know the mints.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        if args.mints.is_empty() {
            return Err("[byreal] mints must be a non-empty list".to_string());
        }
        ok(spot_client()?.get_token_prices(&args.mints)?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetGlobalOverviewArgs {}

pub(crate) struct GetGlobalOverview;

impl DynAomiTool for GetGlobalOverview {
    type App = ByrealApp;
    type Args = GetGlobalOverviewArgs;
    const NAME: &'static str = "byreal_spot_get_global_overview";
    const DESCRIPTION: &'static str = "byreal DEX-wide stats: total TVL, 24h volume, fees, active LPs, current epoch info. Use to gauge venue depth before sizing a position.";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(spot_client()?.get_global_overview()?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetSwapQuoteArgs {
    /// Solana mint address of the input token.
    pub input_mint: String,
    /// Solana mint address of the output token.
    pub output_mint: String,
    /// Amount as a stringified integer in the *input* token's atomic units
    /// (e.g. for 1 USDC, pass "1000000" since USDC has 6 decimals).
    pub amount: String,
    /// "in" (caller specifies input, gets output estimate) or "out".
    pub swap_mode: String,
    /// Slippage tolerance in basis points. Default 100 (= 1%).
    pub slippage_bps: Option<u32>,
    /// Wallet address to quote for. Optional — falls back to the connected SVM wallet.
    pub wallet: Option<String>,
}

pub(crate) struct GetSwapQuote;

impl DynAomiTool for GetSwapQuote {
    type App = ByrealApp;
    type Args = GetSwapQuoteArgs;
    const NAME: &'static str = "byreal_spot_get_swap_quote";
    const DESCRIPTION: &'static str = "Quote a swap via byreal's hybrid AMM+RFQ router. Returns expected output, price impact, the chosen `routerType` (AMM or RFQ), and an unsigned base64 transaction. Read-only — does NOT submit. To execute, call `byreal_spot_build_swap` (which runs this internally and stages the signing route).";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let wallet = resolve_address(args.wallet, &ctx, "svm")?;
        ok(spot_client()?.get_swap_quote(
            &args.input_mint,
            &args.output_mint,
            &args.amount,
            &args.swap_mode,
            args.slippage_bps.unwrap_or(DEFAULT_SLIPPAGE_BPS),
            &wallet,
        )?)
    }
}

// ===========================================================================
// WRITE TOOLS — build/submit pair routed via host::SvmStageTx → SvmCommitTx
// ===========================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct BuildSwapArgs {
    pub input_mint: String,
    pub output_mint: String,
    /// Amount as a stringified integer in the input token's atomic units.
    pub amount: String,
    /// "in" or "out".
    pub swap_mode: String,
    pub slippage_bps: Option<u32>,
    /// Wallet address to swap from. Optional — falls back to the connected SVM wallet.
    pub wallet: Option<String>,
}

pub(crate) struct BuildSwap;

impl DynAomiTool for BuildSwap {
    type App = ByrealApp;
    type Args = BuildSwapArgs;
    const NAME: &'static str = "byreal_spot_build_swap";
    const DESCRIPTION: &'static str = "Build (do not submit) a byreal swap. Internally fetches a router quote and returns a preview + routed `svm_stage_tx` → `svm_commit_tx` steps (staged venue-broadcast; the kernel routes who signs from the wallet's authorization). The matched `byreal_spot_submit_swap` continuation runs once the signed bytes come back. Always emit a one-screen confirmation summary (in/out amount, slippage, router type) and stop the turn before calling this.";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let wallet = resolve_address(args.wallet, &ctx, "svm")?;
        let slippage_bps = args.slippage_bps.unwrap_or(DEFAULT_SLIPPAGE_BPS);

        let quote = spot_client()?.get_swap_quote(
            &args.input_mint,
            &args.output_mint,
            &args.amount,
            &args.swap_mode,
            slippage_bps,
            &wallet,
        )?;

        let unsigned_tx = quote
            .get("transaction")
            .and_then(Value::as_str)
            .ok_or_else(|| "[byreal] swap quote missing `transaction` field".to_string())?
            .to_string();
        let router_type = quote
            .get("routerType")
            .and_then(Value::as_str)
            .unwrap_or("AMM")
            .to_string();
        let quote_id = quote
            .get("quoteId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let order_id = quote
            .get("orderId")
            .and_then(Value::as_str)
            .map(str::to_string);
        let out_amount = quote
            .get("outAmount")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let in_amount = quote
            .get("inAmount")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();
        let price_impact = quote
            .get("priceImpactPct")
            .and_then(Value::as_str)
            .unwrap_or("?")
            .to_string();

        let submit_template = serde_json::to_value(&SubmitSwapArgs {
            confirmation: Some("confirm".to_string()),
            router_type: router_type.clone(),
            unsigned_tx: unsigned_tx.clone(),
            quote_id: quote_id.clone(),
            request_id: order_id.clone(),
            signed_tx: None,
        })
        .map_err(|e| format!("[byreal] submit template serialize: {e}"))?;

        let preview = json!({
            "action_kind": "swap",
            "preview": {
                "input_mint": args.input_mint,
                "output_mint": args.output_mint,
                "in_amount": in_amount,
                "out_amount": out_amount,
                "price_impact_pct": price_impact,
                "slippage_bps": slippage_bps,
                "router_type": router_type,
                "wallet": wallet,
            },
            "submit_args_template": submit_template.clone(),
        });

        let description = format!(
            "byreal {router_type} swap: {in_amount} {} -> {out_amount} {}",
            short_mint(&args.input_mint),
            short_mint(&args.output_mint),
        );

        build_venue_commit_routes::<SubmitSwap>(preview, unsigned_tx, description, submit_template)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SubmitSwapArgs {
    /// Must be `"confirm"`. Forwarded from the build_* preview.
    pub confirmation: Option<String>,
    /// "AMM" or "RFQ" — picks the submission endpoint. From the build preview.
    pub router_type: String,
    /// The original unsigned tx returned by the quote (needed by the AMM
    /// endpoint as `preData`). From the build preview.
    pub unsigned_tx: String,
    /// RFQ-only: opaque quote handle from the router.
    pub quote_id: Option<String>,
    /// RFQ-only: the `orderId` from the quote, sent back as `requestId`.
    pub request_id: Option<String>,
    /// Base64 signed versioned Solana tx. Filled in by the runtime from the
    /// `svm_commit_tx` result — never invent one.
    pub signed_tx: Option<String>,
}

pub(crate) struct SubmitSwap;

impl DynAomiTool for SubmitSwap {
    type App = ByrealApp;
    type Args = SubmitSwapArgs;
    const NAME: &'static str = "byreal_spot_submit_swap";
    const DESCRIPTION: &'static str = "Submit a byreal swap that was previously prepared by `byreal_spot_build_swap` and signed via `svm_commit_tx`. Routes to the AMM or RFQ submission endpoint based on `router_type`. The `signed_tx` field is filled in automatically by the runtime.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        validate_confirmation(args.confirmation.as_deref())?;
        let signed = args.signed_tx.as_deref().ok_or_else(|| {
            "[byreal] signed_tx missing — wait for the svm_commit_tx result".to_string()
        })?;
        let client = spot_client()?;
        let resp = match args.router_type.as_str() {
            "RFQ" => {
                let qid = args
                    .quote_id
                    .as_deref()
                    .ok_or_else(|| "[byreal] RFQ submit requires quote_id".to_string())?;
                let rid = args
                    .request_id
                    .as_deref()
                    .ok_or_else(|| "[byreal] RFQ submit requires request_id".to_string())?;
                client.execute_swap_rfq(qid, rid, signed)?
            }
            _ => {
                // AMM (default): server expects arrays of pre/signed pairs.
                client.execute_swap_amm(
                    std::slice::from_ref(&args.unsigned_tx),
                    &[signed.to_string()],
                )?
            }
        };
        ok(resp)
    }
}

// Solana mints are 44-char base58; truncate for human-readable descriptions.
fn short_mint(mint: &str) -> String {
    if mint.len() <= 8 {
        mint.to_string()
    } else {
        format!("{}…{}", &mint[..4], &mint[mint.len() - 4..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_mint_truncates_long_addresses() {
        assert_eq!(
            short_mint("So11111111111111111111111111111111111111112"),
            "So11…1112"
        );
        assert_eq!(short_mint("ABCD"), "ABCD");
    }

    #[test]
    fn submit_swap_args_round_trip() {
        let args = SubmitSwapArgs {
            confirmation: Some("confirm".to_string()),
            router_type: "RFQ".to_string(),
            unsigned_tx: "AAA=".to_string(),
            quote_id: Some("q".to_string()),
            request_id: Some("r".to_string()),
            signed_tx: Some("BBB=".to_string()),
        };
        let v = serde_json::to_value(&args).unwrap();
        assert_eq!(v["router_type"], "RFQ");
        let back: SubmitSwapArgs = serde_json::from_value(v).unwrap();
        assert_eq!(back.quote_id.as_deref(), Some("q"));
    }

    /// End-to-end route-binding test for byreal's 3-node Lane-2
    /// venue-broadcast pipeline:
    /// `build_swap → host::SvmStageTx → host::SvmCommitTx → submit_swap`.
    ///
    /// `BuildSwap::run_with_routes` hits byreal's HTTP quote endpoint
    /// before calling `build_venue_commit_routes`. This test bypasses
    /// the HTTP step and exercises the route builder directly with
    /// synthetic inputs — hermetic and fast, but covers the same
    /// bug class smoke tests would surface only after a live wallet
    /// dispatch:
    ///   - wrong `bind_as` / `awaits` alias (the signed bytes wouldn't
    ///     land in `submit_swap`'s `signed_tx`)
    ///   - wrong tool name in `next.add` (host marker references the
    ///     wrong verb)
    ///   - missing `broadcaster: "venue"` on the stage step (the host
    ///     would fall back to the manifest default — fine for byreal,
    ///     but the pin is the artifact constraint for RFQ)
    ///   - missing `confirmation` / `unsigned_tx` / `router_type`
    ///     in the submit-args template
    ///   - trigger types flipped (commit on `on_bound_event`,
    ///     submit on `on_sync_return`)
    #[test]
    fn build_swap_route_plan_binds_sign_to_submit() {
        use crate::tool::build_venue_commit_routes;

        let preview = json!({
            "action_kind": "swap",
            "preview": {
                "router_type": "AMM",
                "in_amount": "1000000",
                "out_amount": "950000",
            },
        });
        let unsigned_tx = "AQID".to_string(); // base64 stub — not deserialized in this layer
        let description = "byreal AMM swap: 1.0 IN -> 0.95 OUT".to_string();
        let submit_template = serde_json::to_value(&SubmitSwapArgs {
            confirmation: Some("confirm".to_string()),
            router_type: "AMM".to_string(),
            unsigned_tx: unsigned_tx.clone(),
            quote_id: None,
            request_id: None,
            signed_tx: None,
        })
        .unwrap();

        let ret = build_venue_commit_routes::<SubmitSwap>(
            preview.clone(),
            unsigned_tx.clone(),
            description.clone(),
            submit_template.clone(),
        )
        .expect("route plan should build");
        let env = serde_json::to_value(&ret).expect("envelope serializes");

        // Envelope shape: `__aomi_tool_return` + `__aomi_tool_value` +
        // `__aomi_tool_routes`. The value mirrors the preview the LLM
        // sees; the routes are what the runtime executes after the
        // sync return.
        assert_eq!(env["__aomi_tool_return"], json!(true));
        assert_eq!(env["__aomi_tool_value"], preview);

        let routes = env["__aomi_tool_routes"].as_array().expect("routes array");
        assert_eq!(
            routes.len(),
            3,
            "byreal swap = 3-node (stage → commit → submit). \
             A different count means the route builder grew or shrank \
             a step — update this test if the pipeline shape changed."
        );

        // Node 1 — host::SvmStageTx: stage the venue blob with the
        // venue-broadcast pin. No bind — the artifact that matters
        // downstream is the commit result.
        let stage_step = &routes[0];
        assert_eq!(stage_step["tool"], json!("svm_stage_tx"));
        assert_eq!(
            stage_step["trigger"],
            json!({ "type": "on_sync_return" }),
            "stage fires immediately after build_swap returns"
        );
        let stage_args = &stage_step["args"];
        assert_eq!(
            stage_args["tx"],
            json!(unsigned_tx),
            "the venue-supplied tx blob is what gets staged"
        );
        assert_eq!(stage_args["description"], json!(description));
        assert_eq!(
            stage_args["broadcaster"],
            json!("venue"),
            "byreal's endpoint is the broadcaster — the artifact pin, not a model choice"
        );
        assert!(
            stage_step.get("bind_as").is_none(),
            "stage binds nothing; the signed bytes come from the commit step"
        );

        // Node 2 — host::SvmCommitTx: execute under kernel policy. The
        // model fills `tx_id` from the stage result (see the step note);
        // the signed bytes bind to `signed_tx`.
        let commit_step = &routes[1];
        assert_eq!(commit_step["tool"], json!("svm_commit_tx"));
        assert_eq!(
            commit_step["trigger"],
            json!({ "type": "on_sync_return" }),
            "commit is model-driven right after stage returns the tx_id"
        );
        assert_eq!(
            commit_step["bind_as"],
            json!("signed_tx"),
            "commit result must bind to `signed_tx` — the alias submit_swap awaits"
        );
        assert!(
            commit_step["prompt"]
                .as_str()
                .is_some_and(|p| p.contains("tx_id")),
            "commit step must carry the note telling the model to pass the staged tx_id"
        );

        // Node 3 — submit_swap. Triggered by the bound `signed_tx`
        // event the commit step emits.
        let submit_step = &routes[2];
        assert_eq!(submit_step["tool"], json!(SubmitSwap::NAME));
        assert_eq!(
            submit_step["trigger"],
            json!({ "type": "on_bound_event", "alias": "signed_tx" }),
            "submit waits on the `signed_tx` event the sign step binds"
        );
        let submit_args = &submit_step["args"];
        assert_eq!(submit_args["confirmation"], json!("confirm"));
        assert_eq!(submit_args["router_type"], json!("AMM"));
        assert_eq!(
            submit_args["unsigned_tx"],
            json!(unsigned_tx),
            "AMM endpoint needs the original unsigned tx as `preData`"
        );
        assert!(
            submit_args
                .get("signed_tx")
                .map(Value::is_null)
                .unwrap_or(true),
            "signed_tx must be null at route-emit time; the runtime fills it from the bind"
        );
    }

    /// `submit_swap`'s args template must carry exactly the keys the
    /// runtime expects to splice into. If a future refactor renames
    /// `signed_tx` or drops `unsigned_tx` from the AMM submit payload,
    /// this test fails before any live request fires.
    #[test]
    fn submit_swap_template_carries_runtime_splice_keys() {
        let template = serde_json::to_value(&SubmitSwapArgs {
            confirmation: Some("confirm".to_string()),
            router_type: "AMM".to_string(),
            unsigned_tx: "AQID".to_string(),
            quote_id: None,
            request_id: None,
            signed_tx: None,
        })
        .expect("template serializes");

        // The runtime expects to find `signed_tx` (target of splice)
        // and `unsigned_tx` (AMM endpoint dependency) on the template.
        assert!(
            template.get("signed_tx").is_some(),
            "template missing `signed_tx`"
        );
        assert!(
            template.get("unsigned_tx").is_some(),
            "template missing `unsigned_tx`"
        );
        assert!(
            template.get("router_type").is_some(),
            "template missing `router_type`"
        );
        // confirmation is what gates the submit tool's local validation.
        assert_eq!(template["confirmation"], json!("confirm"));
    }
}
