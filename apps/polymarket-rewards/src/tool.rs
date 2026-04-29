use crate::client::*;
use aomi_sdk::*;
use serde_json::{Value, json};

fn to_json_value<T: serde::Serialize>(value: &T) -> Result<Value, String> {
    serde_json::to_value(value).map_err(|e| format!("failed to encode JSON payload: {e}"))
}

/// Wallet step + deferred follow-up. The wallet tool binds its completion
/// artifact under `callback_field`, and the runtime fires the continuation
/// once that alias resolves.
fn build_rewards_follow_up_result<T>(
    mut result: Value,
    wallet_request: Value,
    follow_up_args_template: Value,
    callback_field: &str,
) -> Result<ToolReturn, String>
where
    T: RouteTarget,
{
    let obj = result
        .as_object_mut()
        .ok_or_else(|| "result is not an object".to_string())?;
    obj.insert("wallet_request".to_string(), wallet_request.clone());

    Ok(ToolReturn::route(result)
        .next(|next| {
            next.add::<host::CommitEip712>(wallet_request)
                .bind_as(callback_field);
        })
        .after::<T>(follow_up_args_template)
        .awaits(callback_field)
        .note("Wallet signed — continue the rewards flow.")
        .build())
}

/// Single immediate next step the LLM should walk to after this tool returns.
fn next_step_immediate<T>(
    result: Value,
    follow_up_args: Value,
    prompt: &str,
) -> Result<ToolReturn, String>
where
    T: RouteTarget,
{
    Ok(ToolReturn::with_route(
        result,
        RouteStep::on_return_to::<T>(follow_up_args).prompt(prompt),
    ))
}

/// Single deferred follow-up gated on an artifact produced by an out-of-band
/// wallet signature flow that the host is already staging for the user.
fn next_step_after_wallet_signature<T>(
    result: Value,
    follow_up_args: Value,
    callback_field: &str,
    prompt: &str,
) -> Result<ToolReturn, String>
where
    T: RouteTarget,
{
    Ok(ToolReturn::with_route(
        result,
        RouteStep::on_bound_to::<T>(follow_up_args, callback_field).prompt(prompt),
    ))
}

fn build_session_eip712_request(
    id: &str,
    address: &str,
    description: String,
    typed_data: Value,
    group_id: &str,
) -> Value {
    let chain_id = typed_data
        .get("domain")
        .and_then(|domain| domain.get("chainId"))
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
        })
        .unwrap_or(137);
    let created_at = chrono::Utc::now().timestamp();

    serde_json::to_value(SessionPendingTransaction {
        id: id.to_string(),
        kind: "eip712Sign",
        chain_id,
        from: address.to_string(),
        to: String::new(),
        value: "0".to_string(),
        data: String::new(),
        gas: "0".to_string(),
        description,
        typed_data,
        group_id: group_id.to_string(),
        created_at,
        state: "created",
    })
    .unwrap_or(Value::Null)
}

fn snap_down_to_tick(price: f64, tick_size: f64) -> f64 {
    if !tick_size.is_finite() || tick_size <= 0.0 {
        return price;
    }
    ((price / tick_size).floor() * tick_size).clamp(0.01, 0.99)
}

fn snap_up_to_tick(price: f64, tick_size: f64) -> f64 {
    if !tick_size.is_finite() || tick_size <= 0.0 {
        return price;
    }
    ((price / tick_size).ceil() * tick_size).clamp(0.01, 0.99)
}

fn round_price_display(price: f64) -> f64 {
    (price * 10000.0).round() / 10000.0
}

fn truncate_share_size(size: f64) -> f64 {
    if !size.is_finite() || size <= 0.0 {
        return 0.0;
    }
    (size * 100.0).floor() / 100.0
}

fn infer_tick_size_from_orderbook(orderbook: &OrderBook) -> Option<f64> {
    let mut prices: Vec<f64> = orderbook
        .bids
        .iter()
        .chain(orderbook.asks.iter())
        .map(|level| level.price)
        .collect();
    if let Some(best_bid) = orderbook.best_bid {
        prices.push(best_bid);
    }
    if let Some(best_ask) = orderbook.best_ask {
        prices.push(best_ask);
    }

    prices.retain(|price| price.is_finite() && *price > 0.0 && *price < 1.0);
    if prices.len() < 2 {
        return None;
    }

    prices.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    prices.dedup_by(|a, b| (*a - *b).abs() < 1e-9);

    prices
        .windows(2)
        .filter_map(|pair| {
            let diff = pair[1] - pair[0];
            (diff > 1e-9).then_some(diff)
        })
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(round_price_display)
}

fn effective_tick_size(orderbook: &OrderBook) -> f64 {
    orderbook
        .tick_size
        .or_else(|| infer_tick_size_from_orderbook(orderbook))
        .unwrap_or(0.001)
}

// ============================================================================
// Tool 1: FindRewardMarkets
// ============================================================================

pub(crate) struct EnsureRewardClobCredentials;

impl DynAomiTool for EnsureRewardClobCredentials {
    type App = PolymarketRewardsApp;
    type Args = EnsureRewardClobCredentialsArgs;
    const NAME: &'static str = "ensure_reward_clob_credentials";
    const DESCRIPTION: &'static str = "Create or derive Polymarket CLOB credentials for the connected wallet. Preferred behavior: if no `clob_l1_signature` is provided, return the exact `commit_eip712` action needed to sign ClobAuth. After the wallet callback, call this tool again with `clob_auth` and `clob_l1_signature` to receive `api_key`, `api_secret`, and `passphrase`.";

    fn run_with_routes(
        _app: &PolymarketRewardsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let clob_auth = args
            .clob_auth
            .unwrap_or_else(|| build_reward_clob_auth_context(&args.address));

        if let Some(clob_l1_signature) = args.clob_l1_signature {
            let client = PolymarketRewardsClient::new()?;
            let credentials = client.create_or_derive_api_credentials(&ClobL1Auth {
                address: clob_auth.address.clone(),
                signature: clob_l1_signature,
                timestamp: clob_auth.timestamp.clone(),
                nonce: Some(clob_auth.nonce.clone()),
            })?;

            return Ok(ToolReturn::value(json!({
                "address": clob_auth.address,
                "status": "ready",
                "credentials": credentials,
                "next_step_hint": "Use these credentials as api_key, api_secret, and passphrase when calling execute_quote_plan or get_quote_plan_status.",
            })));
        }

        let result = json!({
            "address": args.address,
            "status": "awaiting_wallet_signature",
            "clob_auth": clob_auth.clone(),
            "submit_args_template": to_json_value(&EnsureRewardClobCredentialsArgs {
                address: args.address.clone(),
                clob_auth: Some(clob_auth.clone()),
                clob_l1_signature: None,
            })?,
            "next_step_hint": "The host should call commit_eip712 with the exact typed_data below, then call ensure_reward_clob_credentials again with clob_auth and clob_l1_signature from the wallet callback.",
        });
        let wallet_request = to_json_value(&WalletEip712Request {
            typed_data: build_reward_clob_auth_typed_data(&clob_auth),
            description: "Polymarket CLOB auth: sign to create or derive rewards-app credentials"
                .to_string(),
        })?;

        build_rewards_follow_up_result::<EnsureRewardClobCredentials>(
            result,
            wallet_request,
            to_json_value(&EnsureRewardClobCredentialsArgs {
                address: args.address,
                clob_auth: Some(clob_auth),
                clob_l1_signature: None,
            })?,
            "clob_l1_signature",
        )
    }
}

// ============================================================================
// Tool 1: FindRewardMarkets
// ============================================================================

pub(crate) struct FindRewardMarkets;

impl DynAomiTool for FindRewardMarkets {
    type App = PolymarketRewardsApp;
    type Args = FindRewardMarketsArgs;
    const NAME: &'static str = "find_reward_markets";
    const DESCRIPTION: &'static str = "Discover active Polymarket markets enrolled in the liquidity reward program. Returns markets with reward eligibility metadata (min order size, max qualifying spread, reward pool). Use this first to identify candidates before ranking.";

    fn run(
        _app: &PolymarketRewardsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let limit = args.limit.unwrap_or(20).clamp(1, 100);
        let filter = args.filter.as_deref().unwrap_or("all");

        let client = PolymarketRewardsClient::new()?;

        // Gamma already includes rewardsMinSize, rewardsMaxSpread, clobRewards, and clobTokenIds.
        // Fetch more than needed so filters have something to work with.
        let gamma_markets = client
            .fetch_gamma_markets(limit * 3, None)
            .unwrap_or_default();

        // Optionally enrich with CLOB total_daily_rate (best-effort; don't fail if unavailable).
        let clob_configs = client
            .fetch_clob_reward_configs(limit * 3)
            .unwrap_or_default();

        // Build a lookup map for CLOB configs.
        let clob_map: std::collections::HashMap<String, &ClobRewardConfig> = clob_configs
            .iter()
            .map(|c| (c.condition_id.clone(), c))
            .collect();

        // Merge and filter.
        let mut markets: Vec<RewardMarket> = gamma_markets
            .iter()
            .filter_map(|gm| {
                let cid = gm.condition_id.as_deref().or(gm.id.as_deref())?;
                let clob_cfg = clob_map.get(cid).copied();
                merge_into_reward_market(gm, clob_cfg)
            })
            .collect();

        // Apply filter.
        match filter {
            "balanced" => {
                markets.retain(|m| m.yes_price.map(|p| (p - 0.5).abs() < 0.15).unwrap_or(false))
            }
            "liquid" => markets.retain(|m| m.liquidity > 10_000.0),
            _ => {}
        }

        markets.truncate(limit as usize);

        let results: Vec<Value> = markets
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let score = score_market(m);
                json!({
                    "rank": i + 1,
                    "condition_id": m.condition_id,
                    "question": m.question,
                    "slug": m.slug,
                    "yes_token_id": m.yes_token_id,
                    "no_token_id": m.no_token_id,
                    "yes_price": m.yes_price,
                    "no_price": m.no_price,
                    "liquidity_usd": m.liquidity,
                    "rewards_min_size": m.rewards_min_size,
                    "rewards_max_spread": m.rewards_max_spread,
                    "daily_reward_pool_usd": m.daily_reward_pool,
                    "market_competitiveness": m.market_competitiveness,
                    "end_date": m.end_date,
                    "estimated_apy_pct": (score.estimated_apy_pct * 10.0).round() / 10.0,
                })
            })
            .collect();

        Ok(json!({
            "filter": filter,
            "market_count": results.len(),
            "markets": results,
            "next_step_hint": "Call rank_reward_plans to score and rank these markets by reward APY and capital efficiency.",
        }))
    }
}

// ============================================================================
// Tool 2: RankRewardPlans
// ============================================================================

pub(crate) struct RankRewardPlans;

impl DynAomiTool for RankRewardPlans {
    type App = PolymarketRewardsApp;
    type Args = RankRewardPlansArgs;
    const NAME: &'static str = "rank_reward_plans";
    const DESCRIPTION: &'static str = "Score and rank reward-eligible markets by a deterministic model: reward_density × spread_room × capital_efficiency × balance_score. Returns ranked deployment plans with score breakdown and estimated APY. Use this to pick the best market before resolving execution details.";

    fn run(
        _app: &PolymarketRewardsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let result_limit = args.limit.unwrap_or(10).clamp(1, 50);
        let scan_limit = args.scan_limit.unwrap_or(100).clamp(10, 500);
        let min_daily_reward = args.min_daily_reward.unwrap_or(0.0);
        let min_liquidity = args.min_liquidity.unwrap_or(0.0);
        let max_spread_threshold = args.max_spread_threshold.unwrap_or(1.0);

        let client = PolymarketRewardsClient::new()?;

        // Primary: Gamma includes all reward fields + live bid/ask.
        let gamma_markets = client
            .fetch_gamma_markets(scan_limit, None)
            .unwrap_or_default();

        // Best-effort CLOB enrichment for total_daily_rate.
        let clob_configs = client
            .fetch_clob_reward_configs(scan_limit)
            .unwrap_or_default();
        let clob_map: std::collections::HashMap<String, &ClobRewardConfig> = clob_configs
            .iter()
            .map(|c| (c.condition_id.clone(), c))
            .collect();

        let mut plans: Vec<RewardPlan> = gamma_markets
            .iter()
            .filter_map(|gm| {
                let cid = gm.condition_id.as_deref().or(gm.id.as_deref())?;
                let clob_cfg = clob_map.get(cid).copied();
                let market = merge_into_reward_market(gm, clob_cfg)?;

                // Apply filters before scoring.
                if market.daily_reward_pool < min_daily_reward {
                    return None;
                }
                if market.liquidity < min_liquidity {
                    return None;
                }
                if market.rewards_max_spread > max_spread_threshold {
                    return None;
                }

                let s = score_market(&market);
                Some(RewardPlan {
                    plan_id: String::new(), // assigned after sort
                    condition_id: market.condition_id.clone(),
                    question: market.question.clone(),
                    slug: market.slug.clone(),
                    yes_token_id: market.yes_token_id.clone(),
                    no_token_id: market.no_token_id.clone(),
                    score: s.score,
                    reward_density: s.reward_density,
                    capital_efficiency: s.capital_efficiency,
                    spread_room: s.spread_room,
                    balance_score: s.balance_score,
                    rewards_min_size: market.rewards_min_size,
                    rewards_max_spread: market.rewards_max_spread,
                    daily_reward_pool: market.daily_reward_pool,
                    estimated_apy_pct: s.estimated_apy_pct,
                    liquidity: market.liquidity,
                    yes_price: market.yes_price,
                    no_price: market.no_price,
                    end_date: market.end_date.clone(),
                })
            })
            .collect();

        // Sort descending by score.
        plans.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        plans.truncate(result_limit as usize);

        // Assign P1, P2, ... IDs.
        for (i, plan) in plans.iter_mut().enumerate() {
            plan.plan_id = format!("P{}", i + 1);
        }

        let ranked: Vec<Value> = plans
            .iter()
            .map(|p| {
                json!({
                    "plan_id": p.plan_id,
                    "condition_id": p.condition_id,
                    "question": p.question,
                    "slug": p.slug,
                    "yes_token_id": p.yes_token_id,
                    "no_token_id": p.no_token_id,
                    "score": (p.score * 1000.0).round() / 1000.0,
                    "score_breakdown": {
                        "reward_density": (p.reward_density * 1_000_000.0).round() / 1_000_000.0,
                        "capital_efficiency": (p.capital_efficiency * 10000.0).round() / 10000.0,
                        "spread_room": p.spread_room,
                        "balance_score": (p.balance_score * 100.0).round() / 100.0,
                    },
                    "estimated_apy_pct": (p.estimated_apy_pct * 10.0).round() / 10.0,
                    "daily_reward_pool_usd": p.daily_reward_pool,
                    "rewards_min_size_usd": p.rewards_min_size,
                    "rewards_max_spread": p.rewards_max_spread,
                    "liquidity_usd": p.liquidity,
                    "yes_price": p.yes_price,
                    "no_price": p.no_price,
                    "end_date": p.end_date,
                })
            })
            .collect();

        let top_condition_ids: Vec<String> = plans.iter().map(|p| p.condition_id.clone()).collect();

        Ok(json!({
            "plan_count": ranked.len(),
            "plans": ranked,
            "ranked_condition_ids": top_condition_ids,
            "next_step_hint": "Pick a plan_id (e.g. 'P1') and call resolve_reward_deployment with it and ranked_condition_ids.",
        }))
    }
}

// ============================================================================
// Tool 3: ResolveRewardDeployment
// ============================================================================

pub(crate) struct ResolveRewardDeployment;

impl DynAomiTool for ResolveRewardDeployment {
    type App = PolymarketRewardsApp;
    type Args = ResolveRewardDeploymentArgs;
    const NAME: &'static str = "resolve_reward_deployment";
    const DESCRIPTION: &'static str = "Select a specific ranked plan and resolve live execution parameters: fetch the live orderbook, compute optimal quote prices within the reward spread, and calculate expected position size and reward APY. Returns concrete bid/ask prices for build_quote_plan.";

    fn run(
        _app: &PolymarketRewardsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        // Resolve condition_id from plan_id + ranked list, or treat as direct condition_id.
        let condition_id = if args.plan_id.starts_with('P') {
            let idx: usize = args
                .plan_id
                .trim_start_matches('P')
                .parse::<usize>()
                .map_err(|_| format!("Invalid plan_id '{}'. Use 'P1', 'P2', etc.", args.plan_id))?
                .saturating_sub(1);

            args.ranked_condition_ids
                .as_ref()
                .and_then(|ids| ids.get(idx))
                .cloned()
                .ok_or_else(|| {
                    format!(
                        "plan_id '{}' not found in ranked_condition_ids. Provide the ranked_condition_ids from rank_reward_plans.",
                        args.plan_id
                    )
                })?
        } else {
            args.plan_id.clone()
        };

        let client = PolymarketRewardsClient::new()?;

        // Fetch market metadata from Gamma (includes reward fields and live prices).
        let gamma_markets =
            client.fetch_gamma_markets(5, Some(std::slice::from_ref(&condition_id)))?;
        let gamma = gamma_markets
            .into_iter()
            .next()
            .ok_or_else(|| format!("Market not found for condition_id: {condition_id}"))?;

        // Best-effort CLOB enrichment.
        let clob_configs = client.fetch_clob_reward_configs(500).unwrap_or_default();
        let clob_cfg = clob_configs.iter().find(|c| c.condition_id == condition_id);

        let market = merge_into_reward_market(&gamma, clob_cfg)
            .ok_or_else(|| "Market is not enrolled in the reward program.".to_string())?;

        // Fetch live YES-side orderbook.
        let ob_result = client.fetch_orderbook(&market.yes_token_id);
        let (ob, geoblocked) = match ob_result {
            Ok(ob) => (Some(ob), false),
            Err(e) if e.contains("403") || e.contains("geoblock") => (None, true),
            Err(e) => return Err(e),
        };

        let capital = args
            .capital_usd
            .unwrap_or(market.rewards_min_size.max(50.0));

        // Compute optimal quote prices.
        // Strategy: place YES bid and ask symmetrically around mid, within max_spread.
        let (yes_bid, yes_ask, mid, current_spread, tick_size) = if let Some(ref ob) = ob {
            let mid = ob.mid_price.unwrap_or(market.yes_price.unwrap_or(0.5));
            let half_spread = (market.rewards_max_spread * 0.8 / 2.0).min(0.04);
            let tick_size = effective_tick_size(ob);
            let bid = snap_down_to_tick((mid - half_spread).clamp(0.01, 0.99), tick_size);
            let ask = snap_up_to_tick((mid + half_spread).clamp(0.01, 0.99), tick_size);
            (bid, ask, mid, ob.current_spread, Some(tick_size))
        } else {
            // Use Gamma prices when orderbook is unavailable.
            let mid = market.yes_price.unwrap_or(0.5);
            let half_spread = (market.rewards_max_spread * 0.8 / 2.0).min(0.04);
            (
                (mid - half_spread).max(0.01),
                (mid + half_spread).max(0.01),
                mid,
                None,
                None,
            )
        };

        // Implied NO-side prices (sum to 1).
        let no_bid = tick_size
            .map(|tick| snap_down_to_tick((1.0 - yes_ask).max(0.01), tick))
            .unwrap_or_else(|| (1.0 - yes_ask).max(0.01));
        let no_ask = tick_size
            .map(|tick| snap_up_to_tick((1.0 - yes_bid).max(0.01), tick))
            .unwrap_or_else(|| (1.0 - yes_bid).max(0.01));

        // Estimated shares per side.
        let yes_shares = if yes_bid > 0.0 {
            capital / yes_bid
        } else {
            0.0
        };
        let no_shares = if no_bid > 0.0 { capital / no_bid } else { 0.0 };

        // Reward qualification check.
        let proposed_spread = yes_ask - yes_bid;
        let qualifies =
            proposed_spread <= market.rewards_max_spread && capital >= market.rewards_min_size;

        let mut warnings = Vec::<String>::new();
        if geoblocked {
            warnings.push("Orderbook unavailable (geoblocked). Prices derived from Gamma metadata — verify before executing.".to_string());
        }
        if !qualifies {
            if proposed_spread > market.rewards_max_spread {
                warnings.push(format!(
                    "Proposed spread {:.4} exceeds rewards_max_spread {:.4}. Tighten the spread to qualify.",
                    proposed_spread, market.rewards_max_spread
                ));
            }
            if capital < market.rewards_min_size {
                warnings.push(format!(
                    "Capital {:.2} is below rewards_min_size {:.2}. Increase order size to qualify.",
                    capital, market.rewards_min_size
                ));
            }
        }

        let score = score_market(&market);

        let ob_summary = ob.as_ref().map(|ob| {
            json!({
                "best_bid": ob.best_bid,
                "best_ask": ob.best_ask,
                "mid_price": ob.mid_price,
                "current_spread": ob.current_spread,
                "tick_size": ob.tick_size,
                "bid_depth_3": ob.bids.iter().take(3).map(|l| json!({"price": l.price, "size": l.size})).collect::<Vec<_>>(),
                "ask_depth_3": ob.asks.iter().take(3).map(|l| json!({"price": l.price, "size": l.size})).collect::<Vec<_>>(),
            })
        });

        Ok(json!({
            "condition_id": market.condition_id,
            "question": market.question,
            "slug": market.slug,
            "yes_token_id": market.yes_token_id,
            "no_token_id": market.no_token_id,
            "live_orderbook": ob_summary,
            "live_mid_price": round_price_display(mid),
            "current_spread": current_spread,
            "geoblocked": geoblocked,
            "deployment": {
                "capital_usd": capital,
                "yes_bid_price": round_price_display(yes_bid),
                "yes_ask_price": round_price_display(yes_ask),
                "no_bid_price": round_price_display(no_bid),
                "no_ask_price": round_price_display(no_ask),
                "proposed_spread": round_price_display(proposed_spread),
                "yes_shares": truncate_share_size(yes_shares),
                "no_shares": truncate_share_size(no_shares),
                "tick_size": tick_size,
            },
            "reward_params": {
                "rewards_min_size": market.rewards_min_size,
                "rewards_max_spread": market.rewards_max_spread,
                "daily_reward_pool_usd": market.daily_reward_pool,
                "qualifies_for_rewards": qualifies,
            },
            "estimated_apy_pct": (score.estimated_apy_pct * 10.0).round() / 10.0,
            "warnings": warnings,
            "next_step_hint": "Call build_quote_plan with condition_id, token IDs, and the deployment prices above.",
        }))
    }
}

// ============================================================================
// Tool 4: BuildQuotePlan
// ============================================================================

pub(crate) struct BuildQuotePlan;

impl DynAomiTool for BuildQuotePlan {
    type App = PolymarketRewardsApp;
    type Args = BuildQuotePlanArgs;
    const NAME: &'static str = "build_quote_plan";
    const DESCRIPTION: &'static str = "Construct a full execution preview for a two-sided liquidity quote (YES bid + YES ask, with implied NO-side mirrors). Validates reward qualification, shows estimated APY and reward earnings, and returns order payloads ready for wallet signing. Requires explicit user confirmation before execution.";

    fn run(
        _app: &PolymarketRewardsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let client = PolymarketRewardsClient::new()?;
        let canonical_market = client.fetch_reward_market_by_condition_id(&args.condition_id)?;

        // Validation.
        if args.yes_bid_price <= 0.0 || args.yes_bid_price >= 1.0 {
            return Err("yes_bid_price must be in (0, 1)".to_string());
        }
        if args.yes_ask_price <= 0.0 || args.yes_ask_price >= 1.0 {
            return Err("yes_ask_price must be in (0, 1)".to_string());
        }
        if args.yes_ask_price <= args.yes_bid_price {
            return Err("yes_ask_price must be greater than yes_bid_price".to_string());
        }
        if args.order_size_usd <= 0.0 {
            return Err("order_size_usd must be positive".to_string());
        }

        let tick_size = client
            .fetch_orderbook(&canonical_market.yes_token_id)
            .ok()
            .map(|orderbook| effective_tick_size(&orderbook))
            .unwrap_or(0.001);
        let yes_bid_price = snap_down_to_tick(args.yes_bid_price, tick_size);
        let yes_ask_price = snap_up_to_tick(args.yes_ask_price, tick_size);
        let proposed_spread = yes_ask_price - yes_bid_price;
        let max_spread = args.rewards_max_spread.unwrap_or(1.0);
        let min_size = args.rewards_min_size.unwrap_or(0.0);

        let mut warnings = Vec::<String>::new();
        if args.yes_token_id != canonical_market.yes_token_id
            || args.no_token_id != canonical_market.no_token_id
        {
            warnings.push(
                "Quote token IDs did not match the current live reward market. The plugin replaced them with the canonical YES/NO token IDs for this condition before building the quote."
                    .to_string(),
            );
        }
        if (yes_bid_price - args.yes_bid_price).abs() > f64::EPSILON
            || (yes_ask_price - args.yes_ask_price).abs() > f64::EPSILON
        {
            warnings.push(format!(
                "Quote prices were snapped to the live market tick size ({tick_size:.4}) before building the signed orders."
            ));
        }
        let mut qualifies = true;

        if proposed_spread > max_spread {
            qualifies = false;
            warnings.push(format!(
                "Spread {:.4} exceeds rewards_max_spread {:.4}. Orders will NOT qualify for rewards.",
                proposed_spread, max_spread
            ));
        }
        if args.order_size_usd < min_size {
            qualifies = false;
            warnings.push(format!(
                "Order size {:.2} USDC is below rewards_min_size {:.2}. Orders will NOT qualify for rewards.",
                args.order_size_usd, min_size
            ));
        }

        let tif = args
            .time_in_force
            .as_deref()
            .unwrap_or("GTC")
            .to_uppercase();

        let yes_mid = (yes_bid_price + yes_ask_price) / 2.0;
        let no_bid = snap_down_to_tick((1.0 - yes_ask_price).max(0.01), tick_size);
        let no_ask = snap_up_to_tick((1.0 - yes_bid_price).max(0.01), tick_size);

        let yes_bid_shares = args.order_size_usd / yes_bid_price;
        let yes_ask_shares = args.order_size_usd / yes_ask_price;
        let no_bid_shares = args.order_size_usd / no_bid;
        let no_ask_shares = args.order_size_usd / no_ask;
        let yes_token_id = canonical_market.yes_token_id.clone();
        let no_token_id = canonical_market.no_token_id.clone();

        // Build order payload templates.
        // These are the unsigned order structures the model can hand to the wallet signer.
        // The host should sign each via EIP-712 / Polymarket order signing and call execute_quote_plan.
        let tif = match tif.as_str() {
            "GTC" => QuoteTimeInForce::Gtc,
            "FOK" => QuoteTimeInForce::Fok,
            "GTD" => QuoteTimeInForce::Gtd,
            "FAK" => QuoteTimeInForce::Fak,
            other => return Err(format!("unsupported time_in_force `{other}`")),
        };

        let yes_bid_order = QuoteOrderTemplate {
            token_id: yes_token_id.clone(),
            price: round_price_display(yes_bid_price),
            size: truncate_share_size(yes_bid_shares),
            side: QuoteSide::Buy,
            kind: QuoteOrderKind::Limit,
            time_in_force: tif,
            outcome: QuoteOutcome::Yes,
            maker_amount: args.order_size_usd,
            taker_amount: truncate_share_size(yes_bid_shares),
        };

        let yes_ask_order = QuoteOrderTemplate {
            token_id: yes_token_id,
            price: round_price_display(yes_ask_price),
            size: truncate_share_size(yes_ask_shares),
            side: QuoteSide::Sell,
            kind: QuoteOrderKind::Limit,
            time_in_force: tif,
            outcome: QuoteOutcome::Yes,
            maker_amount: truncate_share_size(yes_ask_shares),
            taker_amount: args.order_size_usd,
        };

        let no_bid_order = QuoteOrderTemplate {
            token_id: no_token_id.clone(),
            price: round_price_display(no_bid),
            size: truncate_share_size(no_bid_shares),
            side: QuoteSide::Buy,
            kind: QuoteOrderKind::Limit,
            time_in_force: tif,
            outcome: QuoteOutcome::No,
            maker_amount: args.order_size_usd,
            taker_amount: truncate_share_size(no_bid_shares),
        };

        let no_ask_order = QuoteOrderTemplate {
            token_id: no_token_id,
            price: round_price_display(no_ask),
            size: truncate_share_size(no_ask_shares),
            side: QuoteSide::Sell,
            kind: QuoteOrderKind::Limit,
            time_in_force: tif,
            outcome: QuoteOutcome::No,
            maker_amount: truncate_share_size(no_ask_shares),
            taker_amount: args.order_size_usd,
        };

        let execution_mode = args.execution_mode.unwrap_or(QuoteExecutionMode::FourLeg);
        if execution_mode == QuoteExecutionMode::TwoLegBidOnly {
            warnings.push(
                "Two-leg bid-only mode reduces funding requirements for live smoke tests, but it may earn reduced rewards compared with the full four-leg quote.".to_string(),
            );
        }

        let (selected_order_labels, total_capital, next_step_hint, _signer_description) =
            match execution_mode {
                QuoteExecutionMode::FourLeg => (
                    vec!["yes_bid", "yes_ask", "no_bid", "no_ask"],
                    args.order_size_usd * 4.0,
                    "If needed, call ensure_reward_clob_credentials first so the host can prompt for the ClobAuth signature and derive CLOB credentials automatically. If the user already confirmed this quote preview, do not stop for another approval during credential setup or order signing. Sign all four quote-leg orders via the host wallet (`commit_eip712`), call execute_quote_plan with simulate=true to review the exact signed orders, and only then wait for the user's reconfirmation before any live submission.".to_string(),
                    "Sign each quote-leg limit order for Polymarket reward deployment".to_string(),
                ),
                QuoteExecutionMode::TwoLegBidOnly => (
                    vec!["yes_bid", "no_bid"],
                    args.order_size_usd * 2.0,
                    "If the user confirms this two-leg preview, your very next assistant action must be a submit_reward_quote tool call using the exact submit_args_template below. Do not narrate that you are starting first, and do not omit yes_bid_order or no_bid_order. That staged tool will handle ClobAuth, both wallet signatures, the signed-order simulation, and the final live submit after the user reconfirms.".to_string(),
                    "Sign the two bid-side limit orders for a lower-capital Polymarket smoke test".to_string(),
                ),
            };

        let submit_args_template = match execution_mode {
            QuoteExecutionMode::TwoLegBidOnly => {
                Some(to_json_value(&BuildQuotePlanSubmitTemplate {
                    confirmation: Some("confirm".to_string()),
                    execution_mode,
                    condition_id: args.condition_id.clone(),
                    yes_bid_order: yes_bid_order.clone(),
                    no_bid_order: no_bid_order.clone(),
                })?)
            }
            QuoteExecutionMode::FourLeg => None,
        };

        Ok(json!({
            "condition_id": args.condition_id,
            "execution_mode": execution_mode,
            "quote_summary": {
                "yes_bid_price": round_price_display(yes_bid_price),
                "yes_ask_price": round_price_display(yes_ask_price),
                "no_bid_price": round_price_display(no_bid),
                "no_ask_price": round_price_display(no_ask),
                "yes_mid": round_price_display(yes_mid),
                "proposed_spread": round_price_display(proposed_spread),
                "order_size_usd": args.order_size_usd,
                "total_capital_at_risk_usd": total_capital,
                "time_in_force": tif,
                "qualifies_for_rewards": qualifies,
                "tick_size": tick_size,
            },
            "orders": {
                "yes_bid": yes_bid_order,
                "yes_ask": yes_ask_order,
                "no_bid": no_bid_order,
                "no_ask": no_ask_order,
            },
            "selected_order_labels": selected_order_labels,
            "reward_qualification": {
                "qualifies": qualifies,
                "rewards_max_spread": max_spread,
                "rewards_min_size": min_size,
                "proposed_spread": proposed_spread,
            },
            "warnings": warnings,
            "requires_user_confirmation": true,
            "confirmation_phrase": "confirm",
            "submit_args_template": submit_args_template,
            "next_step_hint": next_step_hint,
        }))
    }
}

// ============================================================================
// Tool 5: SubmitRewardQuote
// ============================================================================

pub(crate) struct SubmitRewardQuote;

impl DynAomiTool for SubmitRewardQuote {
    type App = PolymarketRewardsApp;
    type Args = SubmitRewardQuoteArgs;
    const NAME: &'static str = "submit_reward_quote";
    const DESCRIPTION: &'static str = "Run the connected-wallet Polymarket rewards smoke-test flow inside one staged tool. Stages: 1) request ClobAuth, 2) queue the YES+NO bid signature batch, 3) return a signed-order simulation, 4) submit live only after the user reconfirms. Stay inside this tool until it returns `simulation_ready`, `submitted`, or `submit_failed`.";

    fn run_with_routes(
        _app: &PolymarketRewardsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        validate_confirmation(args.confirmation.as_deref())?;

        let execution_mode = args
            .execution_mode
            .unwrap_or(QuoteExecutionMode::TwoLegBidOnly);
        if execution_mode != QuoteExecutionMode::TwoLegBidOnly {
            return Err(
                "submit_reward_quote currently supports execution_mode='two_leg_bid_only' only."
                    .to_string(),
            );
        }

        let yes_bid_order = args.yes_bid_order.clone().ok_or_else(|| {
            "submit_reward_quote requires yes_bid_order from build_quote_plan".to_string()
        })?;
        let no_bid_order = args.no_bid_order.clone().ok_or_else(|| {
            "submit_reward_quote requires no_bid_order from build_quote_plan".to_string()
        })?;
        let client = PolymarketRewardsClient::new()?;
        let canonical_market = args
            .condition_id
            .as_deref()
            .map(|condition_id| client.fetch_reward_market_by_condition_id(condition_id))
            .transpose()?;
        let mut yes_bid_order = yes_bid_order;
        let mut no_bid_order = no_bid_order;
        let market_question = canonical_market
            .as_ref()
            .map(|market| market.question.clone())
            .or_else(|| args.market_question.clone());
        if let Some(market) = canonical_market.as_ref() {
            yes_bid_order.token_id = market.yes_token_id.clone();
            no_bid_order.token_id = market.no_token_id.clone();
            let tick_size = client
                .fetch_orderbook(&market.yes_token_id)
                .ok()
                .map(|orderbook| effective_tick_size(&orderbook))
                .unwrap_or(0.001);
            let yes_bid_price = snap_down_to_tick(yes_bid_order.price, tick_size);
            let no_bid_price = snap_down_to_tick(no_bid_order.price, tick_size);
            yes_bid_order.price = round_price_display(yes_bid_price);
            no_bid_order.price = round_price_display(no_bid_price);
            yes_bid_order.size = truncate_share_size(yes_bid_order.maker_amount / yes_bid_price);
            no_bid_order.size = truncate_share_size(no_bid_order.maker_amount / no_bid_price);
            yes_bid_order.taker_amount = yes_bid_order.size;
            no_bid_order.taker_amount = no_bid_order.size;
        }
        let clob_auth = args
            .clob_auth
            .clone()
            .unwrap_or_else(|| build_reward_clob_auth_context(&args.address));
        let mut clob_l1_signature = args.clob_l1_signature.clone();
        if let Some(order_signatures) = args.order_signatures.as_ref()
            && clob_l1_signature.is_none()
        {
            for signed in order_signatures {
                if signed.id == "clob_auth" {
                    clob_l1_signature = Some(signed.signature.clone());
                    break;
                }
            }
        }

        if clob_l1_signature.is_none() {
            let request_group_id = format!("reward-clob-auth:{}", args.address);
            let pending_transactions = vec![build_session_eip712_request(
                "clob_auth",
                &args.address,
                "Polymarket CLOB auth: sign to create or derive rewards-app credentials"
                    .to_string(),
                build_reward_clob_auth_typed_data(&clob_auth),
                &request_group_id,
            )];
            let result = json!({
                "stage": "awaiting_clob_auth_signature",
                "execution_mode": execution_mode,
                "address": args.address,
                "status": "awaiting_wallet_signature",
                "clob_auth": clob_auth.clone(),
                "submit_args_template": to_json_value(&SubmitRewardQuoteArgs {
                    confirmation: Some("confirm".to_string()),
                    address: args.address.clone(),
                    condition_id: args.condition_id.clone(),
                    market_question: market_question.clone(),
                    execution_mode: Some(execution_mode),
                    yes_bid_order: Some(yes_bid_order.clone()),
                    no_bid_order: Some(no_bid_order.clone()),
                    clob_auth: Some(clob_auth.clone()),
                    clob_l1_signature: None,
                    credentials: None,
                    signature_type: args.signature_type.clone(),
                    funder: args.funder.clone(),
                    prepared_yes_bid_order: None,
                    prepared_no_bid_order: None,
                    order_signatures: None,
                    yes_bid_signature: None,
                    no_bid_signature: None,
                    simulation_confirmed: None,
                })?,
                "pending_sign_request_group": request_group_id,
                "pending_sign_request_count": pending_transactions.len(),
                "next_step_hint": "Use the frontend pending-signature menu to sign the ClobAuth request. The session now contains the exact EIP-712 payload needed to derive CLOB credentials.",
                "SESSION_PENDING_TRANSACTIONS": pending_transactions,
            });

            return next_step_after_wallet_signature::<SubmitRewardQuote>(
                result,
                to_json_value(&SubmitRewardQuoteArgs {
                    confirmation: Some("confirm".to_string()),
                    address: args.address.clone(),
                    condition_id: args.condition_id.clone(),
                    market_question: market_question.clone(),
                    execution_mode: Some(execution_mode),
                    yes_bid_order: Some(yes_bid_order.clone()),
                    no_bid_order: Some(no_bid_order.clone()),
                    clob_auth: Some(clob_auth),
                    clob_l1_signature: None,
                    credentials: None,
                    signature_type: args.signature_type.clone(),
                    funder: args.funder.clone(),
                    prepared_yes_bid_order: None,
                    prepared_no_bid_order: None,
                    order_signatures: None,
                    yes_bid_signature: None,
                    no_bid_signature: None,
                    simulation_confirmed: None,
                })?,
                "clob_l1_signature",
                "ClobAuth signed — derive credentials and continue the order-signing flow.",
            );
        }

        let clob_l1_signature =
            clob_l1_signature.ok_or_else(|| "missing clob_l1_signature".to_string())?;
        let credentials = if let Some(credentials) = args.credentials.clone() {
            validate_api_credentials(&credentials)?;
            credentials
        } else if args.simulation_confirmed.unwrap_or(false)
            || args.yes_bid_signature.is_some()
            || args.no_bid_signature.is_some()
        {
            client.derive_api_key(&ClobL1Auth {
                address: clob_auth.address.clone(),
                signature: clob_l1_signature.clone(),
                timestamp: clob_auth.timestamp.clone(),
                nonce: Some(clob_auth.nonce.clone()),
            })?
        } else {
            client.create_or_derive_api_credentials(&ClobL1Auth {
                address: clob_auth.address.clone(),
                signature: clob_l1_signature.clone(),
                timestamp: clob_auth.timestamp.clone(),
                nonce: Some(clob_auth.nonce.clone()),
            })?
        };

        let prepared_yes_bid_order =
            args.prepared_yes_bid_order
                .clone()
                .unwrap_or(build_prepared_reward_order(
                    &yes_bid_order,
                    args.address.as_str(),
                    args.signature_type.as_deref(),
                    args.funder.as_deref(),
                )?);
        let prepared_no_bid_order =
            args.prepared_no_bid_order
                .clone()
                .unwrap_or(build_prepared_reward_order(
                    &no_bid_order,
                    args.address.as_str(),
                    args.signature_type.as_deref(),
                    args.funder.as_deref(),
                )?);

        let mut yes_bid_signature = args.yes_bid_signature.clone();
        let mut no_bid_signature = args.no_bid_signature.clone();
        if let Some(order_signatures) = args.order_signatures.as_ref() {
            for signed in order_signatures {
                match signed.id.as_str() {
                    "yes_bid" if yes_bid_signature.is_none() => {
                        yes_bid_signature = Some(signed.signature.clone());
                    }
                    "no_bid" if no_bid_signature.is_none() => {
                        no_bid_signature = Some(signed.signature.clone());
                    }
                    _ => {}
                }
            }
        }

        if yes_bid_signature.is_none() || no_bid_signature.is_none() {
            let request_group_id = format!("reward-quote:{}", args.address);
            let yes_bid_typed_data = build_reward_order_typed_data(&prepared_yes_bid_order);
            let no_bid_typed_data = build_reward_order_typed_data(&prepared_no_bid_order);
            let pending_transactions = vec![
                build_session_eip712_request(
                    "yes_bid",
                    &args.address,
                    build_prepared_reward_order_description(
                        "YES bid",
                        &yes_bid_order,
                        market_question.as_deref(),
                    ),
                    yes_bid_typed_data,
                    &request_group_id,
                ),
                build_session_eip712_request(
                    "no_bid",
                    &args.address,
                    build_prepared_reward_order_description(
                        "NO bid",
                        &no_bid_order,
                        market_question.as_deref(),
                    ),
                    no_bid_typed_data,
                    &request_group_id,
                ),
            ];
            let result = json!({
                "stage": "awaiting_order_signatures",
                "execution_mode": execution_mode,
                "address": args.address,
                "credentials_ready": true,
                "prepared_yes_bid_order": prepared_yes_bid_order.clone(),
                "prepared_no_bid_order": prepared_no_bid_order.clone(),
                "submit_args_template": to_json_value(&SubmitRewardQuoteArgs {
                    confirmation: Some("confirm".to_string()),
                    address: args.address.clone(),
                    condition_id: args.condition_id.clone(),
                    market_question: market_question.clone(),
                    execution_mode: Some(execution_mode),
                    yes_bid_order: Some(yes_bid_order.clone()),
                    no_bid_order: Some(no_bid_order.clone()),
                    clob_auth: Some(clob_auth.clone()),
                    clob_l1_signature: Some(clob_l1_signature.clone()),
                    credentials: Some(credentials.clone()),
                    signature_type: args.signature_type.clone(),
                    funder: args.funder.clone(),
                    prepared_yes_bid_order: Some(prepared_yes_bid_order.clone()),
                    prepared_no_bid_order: Some(prepared_no_bid_order.clone()),
                    order_signatures: None,
                    yes_bid_signature: None,
                    no_bid_signature: None,
                    simulation_confirmed: None,
                })?,
                "pending_sign_request_group": request_group_id,
                "pending_sign_request_count": pending_transactions.len(),
                "next_step_hint": "Use the frontend pending-signature menu to batch-sign both bid orders. The session now contains the exact YES and NO EIP-712 requests.",
                "SESSION_PENDING_TRANSACTIONS": pending_transactions,
            });

            return next_step_after_wallet_signature::<SubmitRewardQuote>(
                result,
                to_json_value(&SubmitRewardQuoteArgs {
                    confirmation: Some("confirm".to_string()),
                    address: args.address.clone(),
                    condition_id: args.condition_id.clone(),
                    market_question: market_question.clone(),
                    execution_mode: Some(execution_mode),
                    yes_bid_order: Some(yes_bid_order.clone()),
                    no_bid_order: Some(no_bid_order.clone()),
                    clob_auth: Some(clob_auth),
                    clob_l1_signature: Some(clob_l1_signature),
                    credentials: Some(credentials),
                    signature_type: args.signature_type.clone(),
                    funder: args.funder.clone(),
                    prepared_yes_bid_order: Some(prepared_yes_bid_order),
                    prepared_no_bid_order: Some(prepared_no_bid_order),
                    order_signatures: None,
                    yes_bid_signature: None,
                    no_bid_signature: None,
                    simulation_confirmed: None,
                })?,
                "order_signatures",
                "Both bid orders signed in batch — continue the rewards submit flow.",
            );
        }

        let yes_bid_signature =
            yes_bid_signature.ok_or_else(|| "missing yes_bid_signature".to_string())?;
        let no_bid_signature =
            no_bid_signature.ok_or_else(|| "missing no_bid_signature".to_string())?;

        let yes_bid_envelope = signed_quote_envelope_from_prepared(
            &prepared_yes_bid_order,
            yes_bid_signature.as_str(),
            credentials.api_key.as_str(),
        );
        let no_bid_envelope = signed_quote_envelope_from_prepared(
            &prepared_no_bid_order,
            no_bid_signature.as_str(),
            credentials.api_key.as_str(),
        );

        if !args.simulation_confirmed.unwrap_or(false) {
            // No route here: the user must reconfirm the simulation preview
            // before live submission. The LLM reads `next_step_hint` and
            // `submit_args_template` in the body and stops to ask. Auto-firing
            // a route would bypass the human gate.
            return Ok(ToolReturn::value(json!({
                "stage": "simulation_ready",
                "mode": "simulation",
                "execution_mode": execution_mode,
                "status": "simulated_ok",
                "note": "Signed-order simulation complete. These exact orders were NOT submitted yet.",
                "simulation_preview": {
                    "market_question": market_question,
                    "total_capital_usdc": yes_bid_order.maker_amount + no_bid_order.maker_amount,
                    "yes_bid": {
                        "side": "BUY",
                        "outcome": "YES",
                        "price": yes_bid_order.price,
                        "shares": yes_bid_order.taker_amount,
                        "capital_usdc": yes_bid_order.maker_amount,
                    },
                    "no_bid": {
                        "side": "BUY",
                        "outcome": "NO",
                        "price": no_bid_order.price,
                        "shares": no_bid_order.taker_amount,
                        "capital_usdc": no_bid_order.maker_amount,
                    },
                    "reward_qualification": "qualifies",
                },
                "submit_args_template": to_json_value(&SubmitRewardQuoteArgs {
                    confirmation: Some("confirm".to_string()),
                    address: args.address.clone(),
                    condition_id: args.condition_id.clone(),
                    market_question: market_question.clone(),
                    execution_mode: Some(execution_mode),
                    yes_bid_order: Some(yes_bid_order.clone()),
                    no_bid_order: Some(no_bid_order.clone()),
                    clob_auth: Some(clob_auth),
                    clob_l1_signature: Some(clob_l1_signature),
                    credentials: Some(credentials.clone()),
                    signature_type: args.signature_type.clone(),
                    funder: args.funder.clone(),
                    prepared_yes_bid_order: Some(prepared_yes_bid_order),
                    prepared_no_bid_order: Some(prepared_no_bid_order),
                    order_signatures: None,
                    yes_bid_signature: Some(yes_bid_signature),
                    no_bid_signature: Some(no_bid_signature),
                    simulation_confirmed: Some(true),
                })?,
                "next_step_hint": "Show the compact signed-order simulation preview to the user, then stop. Only after the user reconfirms should you call submit_reward_quote again with submit_args_template to submit live.",
                "SESSION_PENDING_TRANSACTIONS": [],
            })));
        }

        if client.is_geoblocked() {
            return Err(
                "Order submission is unavailable: your region is geoblocked by Polymarket. The signed-order simulation succeeded, but live submission is blocked."
                    .to_string(),
            );
        }

        let creds = ClobCredentials {
            address: args.address.clone(),
            api_key: credentials.api_key.clone(),
            api_secret: credentials.api_secret.clone(),
            passphrase: credentials.passphrase.clone(),
            signature_type: args.signature_type.clone(),
            funder: args.funder.clone(),
        };
        let submit_results = match client
            .submit_orders(&creds, &[yes_bid_envelope.clone(), no_bid_envelope.clone()])
        {
            Ok(results) => results,
            Err(error) => {
                return Ok(ToolReturn::value(json!({
                    "stage": "submit_failed",
                    "mode": "live",
                    "execution_mode": execution_mode,
                    "status": "error",
                    "error": error,
                    "next_step_hint": "Stop here and show the exact submission error to the user. Do not re-rank markets, rebuild a fresh quote, or retry automatically.",
                    "SESSION_PENDING_TRANSACTIONS": [],
                })));
            }
        };
        Ok(ToolReturn::value(json!({
            "stage": "submitted",
            "mode": "live",
            "execution_mode": execution_mode,
            "status": "submitted",
            "order_count": submit_results.len(),
            "order_results": {
                "yes_bid_order": submit_results.first().cloned(),
                "no_bid_order": submit_results.get(1).cloned(),
            },
            "next_step_hint": "Show this exact live submission result to the user. If they want verification next, call get_quote_plan_status explicitly.",
            "SESSION_PENDING_TRANSACTIONS": [],
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aomi_sdk::{DynAomiTool, DynToolCallCtx, RouteTrigger};

    fn sample_order(token_id: &str, outcome: QuoteOutcome) -> QuoteOrderTemplate {
        QuoteOrderTemplate {
            token_id: token_id.to_string(),
            price: 0.49,
            size: 10.0,
            side: QuoteSide::Buy,
            kind: QuoteOrderKind::Limit,
            time_in_force: QuoteTimeInForce::Gtc,
            outcome,
            maker_amount: 4.9,
            taker_amount: 10.0,
        }
    }

    #[test]
    fn clob_auth_resume_route_uses_clob_signature_arg() {
        let result = SubmitRewardQuote::run_with_routes(
            &PolymarketRewardsApp,
            SubmitRewardQuoteArgs {
                confirmation: Some("confirm".to_string()),
                address: "0x000000000000000000000000000000000000dEaD".to_string(),
                condition_id: None,
                market_question: Some("Will BTC go up?".to_string()),
                execution_mode: Some(QuoteExecutionMode::TwoLegBidOnly),
                yes_bid_order: Some(sample_order("yes-token", QuoteOutcome::Yes)),
                no_bid_order: Some(sample_order("no-token", QuoteOutcome::No)),
                clob_auth: Some(build_reward_clob_auth_context(
                    "0x000000000000000000000000000000000000dEaD",
                )),
                clob_l1_signature: None,
                credentials: None,
                signature_type: None,
                funder: None,
                prepared_yes_bid_order: None,
                prepared_no_bid_order: None,
                order_signatures: None,
                yes_bid_signature: None,
                no_bid_signature: None,
                simulation_confirmed: None,
            },
            DynToolCallCtx {
                session_id: "session".to_string(),
                tool_name: "submit_reward_quote".to_string(),
                call_id: "call".to_string(),
                state_attributes: Default::default(),
            },
        )
        .expect("submit_reward_quote should stage clob auth signing");

        assert_eq!(result.routes.len(), 1);
        match &result.routes[0].trigger {
            RouteTrigger::OnBoundEvent { alias } => assert_eq!(alias, "clob_l1_signature"),
            other => panic!("unexpected trigger: {other:?}"),
        }
    }
}

// ============================================================================
// Tool 6: ExecuteQuotePlan
// ============================================================================

pub(crate) struct ExecuteQuotePlan;

impl DynAomiTool for ExecuteQuotePlan {
    type App = PolymarketRewardsApp;
    type Args = ExecuteQuotePlanArgs;
    const NAME: &'static str = "execute_quote_plan";
    const DESCRIPTION: &'static str = "Submit the signed quote from build_quote_plan to the Polymarket CLOB. Supports `execution_mode='four_leg'` for simulation and `execution_mode='two_leg_bid_only'` for lower-capital live smoke tests. Live submission is currently limited to the two bid legs (`yes_bid_order`, `no_bid_order`) and requires a prior signed-order simulation plus `simulation_confirmed=true`. Requires confirmation='confirm' and valid CLOB L2 credentials. Set simulate=true to do a dry run without submitting. After a successful live submission, immediately call get_quote_plan_status to verify open orders and earnings.";

    fn run_with_routes(
        _app: &PolymarketRewardsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        validate_confirmation(args.confirmation.as_deref())?;

        let simulate = args.simulate.unwrap_or(false);
        let execution_mode = args.execution_mode.unwrap_or(QuoteExecutionMode::FourLeg);
        let orders = match execution_mode {
            QuoteExecutionMode::FourLeg => [
                ("yes_bid_order", args.yes_bid_order),
                ("yes_ask_order", args.yes_ask_order),
                ("no_bid_order", args.no_bid_order),
                ("no_ask_order", args.no_ask_order),
            ]
            .into_iter()
            .map(|(name, value)| {
                value.ok_or_else(|| {
                    format!(
                        "Missing `{name}`. Four-leg execution requires yes_bid_order, yes_ask_order, no_bid_order, and no_ask_order."
                    )
                })
                .map(|order| (name, order))
            })
            .collect::<Result<Vec<_>, _>>()?,
            QuoteExecutionMode::TwoLegBidOnly => [
                ("yes_bid_order", args.yes_bid_order),
                ("no_bid_order", args.no_bid_order),
            ]
            .into_iter()
            .map(|(name, value)| {
                value.ok_or_else(|| {
                    format!(
                        "Missing `{name}`. Two-leg bid-only execution requires yes_bid_order and no_bid_order."
                    )
                })
                .map(|order| (name, order))
            })
            .collect::<Result<Vec<_>, _>>()?,
        };

        if simulate {
            let simulated_orders = orders
                .iter()
                .map(|(name, order)| {
                    Ok((
                        (*name).to_string(),
                        serde_json::to_value(order)
                            .map_err(|e| format!("failed to serialize simulated order: {e}"))?,
                    ))
                })
                .collect::<Result<serde_json::Map<String, Value>, String>>()?;

            return Ok(ToolReturn::value(json!({
                "mode": "simulation",
                "execution_mode": execution_mode,
                "order_count": simulated_orders.len(),
                "orders": simulated_orders,
                "status": "simulated_ok",
                "note": "simulate=true: orders were NOT submitted to the CLOB.",
                "next_step_hint": "Set simulate=false and re-confirm to submit live orders.",
            })));
        }

        if !args.simulation_confirmed.unwrap_or(false) {
            let simulated_orders = orders
                .iter()
                .map(|(name, order)| {
                    Ok((
                        (*name).to_string(),
                        serde_json::to_value(order)
                            .map_err(|e| format!("failed to serialize simulated order: {e}"))?,
                    ))
                })
                .collect::<Result<serde_json::Map<String, Value>, String>>()?;

            return Ok(ToolReturn::value(json!({
                "mode": "simulation_required",
                "execution_mode": execution_mode,
                "order_count": simulated_orders.len(),
                "orders": simulated_orders,
                "status": "simulation_required",
                "note": "Live execution is blocked until these exact signed orders are simulated and reviewed first.",
                "next_step_hint": "Review this signed-order simulation, then call execute_quote_plan again with the same signed payloads, confirmation='confirm', simulate=false, and simulation_confirmed=true to submit live.",
            })));
        }

        if execution_mode != QuoteExecutionMode::TwoLegBidOnly {
            return Err(
                "Live submission is temporarily limited to execution_mode='two_leg_bid_only' so we can keep funding requirements lower during smoke testing.".to_string(),
            );
        }

        let status_follow_up_args = json!({
            "address": args.address.clone(),
            "api_key": args.api_key.clone(),
            "api_secret": args.api_secret.clone(),
            "passphrase": args.passphrase.clone(),
            "signature_type": args.signature_type.clone(),
            "funder": args.funder.clone(),
            "include_earnings": true,
        });

        let creds = ClobCredentials {
            address: args.address,
            api_key: args.api_key,
            api_secret: args.api_secret,
            passphrase: args.passphrase,
            signature_type: args.signature_type,
            funder: args.funder,
        };

        let client = PolymarketRewardsClient::new()?;

        // Check geoblock before attempting submission.
        if client.is_geoblocked() {
            return Err(
                "Order submission is unavailable: your region is geoblocked by Polymarket. Use simulate=true for dry runs.".to_string(),
            );
        }

        let submit_results = client.submit_orders(
            &creds,
            &orders
                .iter()
                .map(|(_, order)| order.clone())
                .collect::<Vec<_>>(),
        )?;
        let mut submitted_orders = serde_json::Map::<String, Value>::new();
        for ((name, _), result) in orders.iter().zip(submit_results.into_iter()) {
            submitted_orders.insert((*name).to_string(), result);
        }

        let result = json!({
            "mode": "live",
            "execution_mode": execution_mode,
            "order_count": submitted_orders.len(),
            "order_results": submitted_orders,
            "status": "submitted",
            "next_step_hint": "Immediately call get_quote_plan_status to verify the open orders landed and to show reward earnings context.",
        });

        next_step_immediate::<GetQuotePlanStatus>(
            result,
            status_follow_up_args,
            "Immediately verify the live submission by fetching open orders and reward earnings so the user can see the deployed liquidity status.",
        )
    }
}

// ============================================================================
// Tool 6: GetQuotePlanStatus
// ============================================================================

pub(crate) struct GetQuotePlanStatus;

impl DynAomiTool for GetQuotePlanStatus {
    type App = PolymarketRewardsApp;
    type Args = GetQuotePlanStatusArgs;
    const NAME: &'static str = "get_quote_plan_status";
    const DESCRIPTION: &'static str = "Check the status of a previously executed quote plan: fetch open orders for the market, show fill rates, and optionally retrieve reward earnings for the connected address. Use after execute_quote_plan to verify orders landed and are qualifying for rewards.";

    fn run(
        _app: &PolymarketRewardsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<Value, String> {
        let creds = ClobCredentials {
            address: args.address.clone(),
            api_key: args.api_key,
            api_secret: args.api_secret,
            passphrase: args.passphrase,
            signature_type: args.signature_type,
            funder: args.funder,
        };

        let client = PolymarketRewardsClient::new()?;

        // Fetch open orders.
        let (order_summary, open_orders_error) =
            match client.fetch_open_orders(&creds, args.asset_id.as_deref()) {
                Ok(open_orders) => {
                    let summary = open_orders
                        .iter()
                        .map(|o| {
                            json!({
                                "order_id": o.get("id").or_else(|| o.get("order_id")),
                                "asset_id": o.get("asset_id"),
                                "side": o.get("side"),
                                "price": o.get("price"),
                                "size": o.get("size").or_else(|| o.get("original_size")),
                                "size_matched": o.get("size_matched"),
                                "status": o.get("status"),
                                "created_at": o.get("created_at"),
                            })
                        })
                        .collect::<Vec<_>>();
                    (summary, None)
                }
                Err(err) => (Vec::new(), Some(err)),
            };

        // Optionally fetch reward earnings.
        let earnings = if args.include_earnings.unwrap_or(true) {
            match client.fetch_reward_earnings(&creds) {
                Ok(e) => Some(e),
                Err(err) => Some(json!({"error": err})),
            }
        } else {
            None
        };

        Ok(json!({
            "address": args.address,
            "asset_filter": args.asset_id,
            "open_order_count": order_summary.len(),
            "open_orders": order_summary,
            "open_orders_error": open_orders_error,
            "reward_earnings": earnings,
            "note": "Open orders within rewards_max_spread and >= rewards_min_size are qualifying for maker rewards.",
            "next_step_hint": "Use withdraw_quote_liquidity to cancel resting quote orders when you want to pull liquidity from the book.",
        }))
    }
}

// ============================================================================
// Tool 7: WithdrawQuoteLiquidity
// ============================================================================

pub(crate) struct WithdrawQuoteLiquidity;

impl DynAomiTool for WithdrawQuoteLiquidity {
    type App = PolymarketRewardsApp;
    type Args = WithdrawQuoteLiquidityArgs;
    const NAME: &'static str = "withdraw_quote_liquidity";
    const DESCRIPTION: &'static str = "Cancel resting quote orders to withdraw liquidity from the Polymarket orderbook. This only removes open orders; it does not unwind already filled YES/NO positions. Provide either explicit `order_ids`, or a `condition_id`, or an `asset_id`. Set simulate=true to preview what would be canceled.";

    fn run_with_routes(
        _app: &PolymarketRewardsApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let simulate = args.simulate.unwrap_or(false);
        if !simulate {
            validate_confirmation(args.confirmation.as_deref())?;
        }

        let has_order_ids = args
            .order_ids
            .as_ref()
            .map(|ids| !ids.is_empty())
            .unwrap_or(false);
        let has_condition_id = args
            .condition_id
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        let has_asset_id = args
            .asset_id
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);

        if !has_order_ids && !has_condition_id && !has_asset_id {
            return Err(
                "Provide at least one cancellation scope: `order_ids`, `condition_id`, or `asset_id`."
                    .to_string(),
            );
        }

        let creds = ClobCredentials {
            address: args.address.clone(),
            api_key: args.api_key,
            api_secret: args.api_secret,
            passphrase: args.passphrase,
            signature_type: args.signature_type,
            funder: args.funder,
        };

        let client = PolymarketRewardsClient::new()?;

        if simulate {
            let open_orders = client.fetch_open_orders(&creds, None)?;
            let filtered_orders: Vec<Value> = open_orders
                .into_iter()
                .filter(|order| {
                    let order_id_matches = args.order_ids.as_ref().is_some_and(|ids| {
                        order
                            .get("id")
                            .and_then(Value::as_str)
                            .map(|id| ids.iter().any(|candidate| candidate == id))
                            .unwrap_or(false)
                    });
                    let condition_id_matches = args.condition_id.as_ref().is_some_and(|cid| {
                        order
                            .get("market")
                            .and_then(Value::as_str)
                            .map(|market| market.eq_ignore_ascii_case(cid))
                            .unwrap_or(false)
                    });
                    let asset_id_matches = args.asset_id.as_ref().is_some_and(|asset_id| {
                        order
                            .get("asset_id")
                            .and_then(Value::as_str)
                            .map(|value| value == asset_id)
                            .unwrap_or(false)
                    });

                    if has_order_ids {
                        order_id_matches
                    } else if has_condition_id && has_asset_id {
                        condition_id_matches && asset_id_matches
                    } else if has_condition_id {
                        condition_id_matches
                    } else {
                        asset_id_matches
                    }
                })
                .collect();

            let order_ids = filtered_orders
                .iter()
                .filter_map(|order| order.get("id").and_then(Value::as_str))
                .collect::<Vec<_>>();

            return Ok(ToolReturn::value(json!({
                "mode": "simulation",
                "status": "simulated_ok",
                "matched_order_count": filtered_orders.len(),
                "matched_order_ids": order_ids,
                "matched_orders": filtered_orders,
                "note": "simulate=true: no orders were canceled.",
                "next_step_hint": "Set simulate=false and confirmation='confirm' to cancel the matched resting liquidity.",
            })));
        }

        let status_follow_up_args = json!({
            "address": args.address.clone(),
            "api_key": creds.api_key.clone(),
            "api_secret": creds.api_secret.clone(),
            "passphrase": creds.passphrase.clone(),
            "signature_type": creds.signature_type.clone(),
            "funder": creds.funder.clone(),
            "include_earnings": true,
        });

        let result = if let Some(order_ids) = args.order_ids.as_ref().filter(|ids| !ids.is_empty())
        {
            client.cancel_orders(&creds, order_ids)?
        } else {
            client.cancel_market_orders(
                &creds,
                args.condition_id.as_deref(),
                args.asset_id.as_deref(),
            )?
        };

        let result = json!({
            "mode": "live",
            "status": "cancellation_submitted",
            "scope": {
                "order_ids": args.order_ids,
                "condition_id": args.condition_id,
                "asset_id": args.asset_id,
            },
            "result": result,
            "next_step_hint": "Immediately call get_quote_plan_status to verify that the resting quote liquidity is gone. Filled positions, if any, remain in the wallet.",
        });

        next_step_immediate::<GetQuotePlanStatus>(
            result,
            status_follow_up_args,
            "Immediately verify the cancellation by fetching open orders and reward earnings so the user can see whether the resting liquidity is gone.",
        )
    }
}
