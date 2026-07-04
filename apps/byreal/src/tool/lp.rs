//! `byreal_lp_*` tools — Copy Farming intelligence + LP position management
//! on byreal Solana.
//!
//! Reads expose the byreal-unique social-LP dataset (top performers,
//! per-provider strategy histories, current incentive epoch). Writes cover
//! reward claiming via the `host::SvmStageTx` → `host::SvmCommitTx` route
//! (staged `broadcaster: "venue"`).
//!
//! v1 explicitly handles the single-tx reward-claim case. byreal's
//! `encode-v2` endpoint may return multiple unsigned txs per call (one per
//! pool cluster); when that happens, this tool errors with a hint asking the
//! LLM to claim positions in smaller groups (one position at a time is
//! always safe). Multi-tx orchestration can be added later once the
//! happy-path is validated against live byreal state.

use crate::client::ByrealApp;
use crate::client::lp::lp_client;
use crate::tool::{build_venue_commit_routes, ok, resolve_address, validate_confirmation};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const DEFAULT_PAGE_SIZE: u32 = 20;

// ===========================================================================
// READ TOOLS
// ===========================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetTopLpsArgs {
    /// Optional: restrict to one pool. Omit to get the venue-wide leaderboard.
    pub pool_address: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    /// Sort field: "liquidity" (default), "earnedUsd", "pnlUsd", "copies".
    pub sort_field: Option<String>,
    /// Sort order: "desc" (default) or "asc".
    pub sort_type: Option<String>,
    /// 0 = active positions only (default), other values per byreal docs.
    pub status: Option<i32>,
}

pub(crate) struct GetTopLps;

impl DynAomiTool for GetTopLps {
    type App = ByrealApp;
    type Args = GetTopLpsArgs;
    const NAME: &'static str = "byreal_lp_get_top_performers";
    const DESCRIPTION: &'static str = "Leaderboard of top byreal CLMM LP positions, ranked by liquidity / earnings / PnL / copy count. Each row carries the wallet, pool, tick range, current liquidity, fees earned, PnL, and copy follower count. The killer Copy Farming dataset — feed sort_field='earnedUsdPercent' or 'pnlUsdPercent' to find the highest-Sharpe strategies.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(lp_client()?.list_top_positions(
            args.pool_address.as_deref(),
            args.page.unwrap_or(1),
            args.page_size.unwrap_or(DEFAULT_PAGE_SIZE),
            args.sort_field.as_deref().unwrap_or("liquidity"),
            args.sort_type.as_deref().unwrap_or("desc"),
            args.status,
        )?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetProviderOverviewArgs {
    /// LP wallet address to inspect.
    pub provider_address: String,
}

pub(crate) struct GetProviderOverview;

impl DynAomiTool for GetProviderOverview {
    type App = ByrealApp;
    type Args = GetProviderOverviewArgs;
    const NAME: &'static str = "byreal_lp_get_provider_overview";
    const DESCRIPTION: &'static str = "Deep dive on one LP wallet: total bonus earned, unclaimed bonus, copies / follows counts, copy-bonus + follow-bonus splits. Use after `byreal_lp_get_top_performers` to evaluate a candidate provider before mirroring their strategy.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        ok(lp_client()?.get_provider_overview(&args.provider_address)?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetPositionsArgs {
    /// Wallet address. Optional — falls back to the connected SVM wallet.
    pub wallet: Option<String>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub sort_field: Option<String>,
    pub sort_type: Option<String>,
    pub pool_address: Option<String>,
    pub status: Option<String>,
}

pub(crate) struct GetPositions;

impl DynAomiTool for GetPositions {
    type App = ByrealApp;
    type Args = GetPositionsArgs;
    const NAME: &'static str = "byreal_lp_get_positions";
    const DESCRIPTION: &'static str = "List a wallet's open + closed CLMM positions on byreal: each position's pool, tick range, current liquidity USD, fees earned, PnL, APR, accumulated bonus.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let wallet = resolve_address(args.wallet, &ctx, "svm")?;
        ok(lp_client()?.list_positions(
            &wallet,
            args.page.unwrap_or(1),
            args.page_size.unwrap_or(DEFAULT_PAGE_SIZE),
            args.sort_field.as_deref(),
            args.sort_type.as_deref(),
            args.pool_address.as_deref(),
            args.status.as_deref(),
        )?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetUnclaimedRewardsArgs {
    /// Wallet address. Optional — falls back to the connected SVM wallet.
    pub wallet: Option<String>,
}

pub(crate) struct GetUnclaimedRewards;

impl DynAomiTool for GetUnclaimedRewards {
    type App = ByrealApp;
    type Args = GetUnclaimedRewardsArgs;
    const NAME: &'static str = "byreal_lp_get_unclaimed_rewards";
    const DESCRIPTION: &'static str = "Pending fees + incentive rewards across a wallet's open and closed byreal positions. Returns `unclaimedOpenIncentives` and `unclaimedClosedIncentives` arrays; use the position addresses from these to drive `byreal_lp_build_claim_rewards`.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let wallet = resolve_address(args.wallet, &ctx, "svm")?;
        ok(lp_client()?.get_unclaimed_data(&wallet)?)
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct GetEpochBonusArgs {
    /// Wallet to query bonus for. Optional — falls back to the connected SVM wallet.
    pub wallet: Option<String>,
    /// Bonus type filter (-1 = all, default).
    pub bonus_type: Option<i32>,
}

pub(crate) struct GetEpochBonus;

impl DynAomiTool for GetEpochBonus {
    type App = ByrealApp;
    type Args = GetEpochBonusArgs;
    const NAME: &'static str = "byreal_lp_get_epoch_bonus";
    const DESCRIPTION: &'static str = "Current Copy Farmer incentive epoch info for a wallet: which epochs have bonuses available, claimable amounts per epoch, schedule.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let wallet = resolve_address(args.wallet, &ctx, "svm")?;
        ok(lp_client()?.get_epoch_bonus(&wallet, args.bonus_type.unwrap_or(-1))?)
    }
}

// ===========================================================================
// WRITE TOOLS — claim rewards via build/submit pair
// ===========================================================================

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub(crate) struct BuildClaimRewardsArgs {
    /// One or more position addresses to claim rewards from. Look these up
    /// via `byreal_lp_get_unclaimed_rewards` first.
    pub position_addresses: Vec<String>,
    /// Wallet that owns the positions. Optional — falls back to the connected SVM wallet.
    pub wallet: Option<String>,
    /// Optional bonus type filter; pass through unchanged unless you know what it means.
    pub bonus_type: Option<i32>,
}

pub(crate) struct BuildClaimRewards;

impl DynAomiTool for BuildClaimRewards {
    type App = ByrealApp;
    type Args = BuildClaimRewardsArgs;
    const NAME: &'static str = "byreal_lp_build_claim_rewards";
    const DESCRIPTION: &'static str = "Build (do not submit) a Copy Farming reward / fee claim. Internally encodes the claim against byreal's incentive contract and returns a preview + routed `svm_stage_tx` → `svm_commit_tx` steps (staged venue-broadcast; the kernel routes who signs). v1 supports single-transaction claims only — if byreal's encoder returns multiple txs (large position batches), this errors with a hint to retry with fewer position_addresses at a time.";

    fn run_with_routes(
        _app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        if args.position_addresses.is_empty() {
            return Err("[byreal] position_addresses must be a non-empty list".to_string());
        }
        let wallet = resolve_address(args.wallet, &ctx, "svm")?;

        let encoded =
            lp_client()?.encode_reward(&wallet, &args.position_addresses, args.bonus_type)?;

        let order_code = encoded
            .get("orderCode")
            .and_then(Value::as_str)
            .ok_or_else(|| "[byreal] reward encode missing `orderCode`".to_string())?
            .to_string();

        let items = encoded
            .get("rewardEncodeItems")
            .and_then(Value::as_array)
            .ok_or_else(|| "[byreal] reward encode missing `rewardEncodeItems`".to_string())?;

        if items.is_empty() {
            return Err(
                "[byreal] reward encoder returned 0 transactions — nothing to claim?".to_string(),
            );
        }
        if items.len() > 1 {
            return Err(format!(
                "[byreal] v1 supports single-tx claims; got {} txs. Retry with fewer position_addresses at a time.",
                items.len()
            ));
        }
        let unsigned_tx = items[0]
            .get("transaction")
            .and_then(Value::as_str)
            .ok_or_else(|| "[byreal] reward encode item missing `transaction`".to_string())?
            .to_string();

        let submit_template = serde_json::to_value(&SubmitClaimRewardsArgs {
            confirmation: Some("confirm".to_string()),
            order_code: order_code.clone(),
            wallet: wallet.clone(),
            signed_tx: None,
        })
        .map_err(|e| format!("[byreal] submit template serialize: {e}"))?;

        let preview = json!({
            "action_kind": "claim_rewards",
            "preview": {
                "wallet": wallet,
                "position_addresses": args.position_addresses,
                "tx_count": items.len(),
                "order_code": order_code,
                "encode_items": items,
            },
            "submit_args_template": submit_template.clone(),
        });

        let description = format!(
            "byreal claim rewards: {} position(s)",
            args.position_addresses.len()
        );

        build_venue_commit_routes::<SubmitClaimRewards>(
            preview,
            unsigned_tx,
            description,
            submit_template,
        )
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub(crate) struct SubmitClaimRewardsArgs {
    /// Must be `"confirm"`. Forwarded from the build_* preview.
    pub confirmation: Option<String>,
    /// Opaque order handle from the encode step.
    pub order_code: String,
    /// Wallet that owns the positions.
    pub wallet: String,
    /// Base64 signed Solana tx. Filled in by the runtime from the
    /// `svm_commit_tx` result.
    pub signed_tx: Option<String>,
}

pub(crate) struct SubmitClaimRewards;

impl DynAomiTool for SubmitClaimRewards {
    type App = ByrealApp;
    type Args = SubmitClaimRewardsArgs;
    const NAME: &'static str = "byreal_lp_submit_claim_rewards";
    const DESCRIPTION: &'static str = "Submit a signed reward / fee claim prepared by `byreal_lp_build_claim_rewards`. The `signed_tx` field is filled in automatically by the runtime.";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        validate_confirmation(args.confirmation.as_deref())?;
        let signed = args.signed_tx.as_deref().ok_or_else(|| {
            "[byreal] signed_tx missing — wait for the svm_commit_tx result".to_string()
        })?;
        // byreal's submit endpoint expects the signed tx wrapped in an array under
        // `signedTxPayload` — match what the frontend sends.
        let payload = json!([signed]);
        ok(lp_client()?.submit_reward_order(&args.order_code, &args.wallet, payload)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_claim_rewards_args_round_trip() {
        let args = SubmitClaimRewardsArgs {
            confirmation: Some("confirm".to_string()),
            order_code: "abc123".to_string(),
            wallet: "Sol1111".to_string(),
            signed_tx: Some("AAA=".to_string()),
        };
        let v = serde_json::to_value(&args).unwrap();
        assert_eq!(v["order_code"], "abc123");
        let back: SubmitClaimRewardsArgs = serde_json::from_value(v).unwrap();
        assert_eq!(back.signed_tx.as_deref(), Some("AAA="));
    }
}
