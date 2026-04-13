use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const SYSTEM_NEXT_ACTION_KEY: &str = "SYSTEM_NEXT_ACTION";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum NextAction {
    ToolCalls(Vec<NextActionTool>),
    #[allow(dead_code)]
    Instructions(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct NextActionTool {
    name: String,
    reason: String,
    args: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    condition: Option<String>,
}

fn build_polymarket_follow_up_result(
    mut result: Value,
    wallet_tool: &str,
    wallet_request: Value,
    follow_up_step: &str,
    follow_up_args_template: Value,
    follow_up_reason: &str,
    callback_field: &str,
) -> Result<Value, String> {
    let tool_calls = vec![
        NextActionTool {
            name: wallet_tool.to_string(),
            reason: "REQUIRED: Call this tool with these exact args. Do NOT skip or assume it was already sent.".to_string(),
            args: wallet_request.clone(),
            condition: None,
        },
        NextActionTool {
            name: follow_up_step.to_string(),
            reason: follow_up_reason.to_string(),
            args: follow_up_args_template,
            condition: Some(
                format!(
                    "After wallet callback reports signature success; include {callback_field} from callback."
                ),
            ),
        },
    ];

    let action_value = serde_json::to_value(NextAction::ToolCalls(tool_calls))
        .map_err(|e| format!("Failed to serialize SYSTEM_NEXT_ACTION: {e}"))?;

    let obj = result
        .as_object_mut()
        .ok_or_else(|| "result is not an object".to_string())?;
    obj.insert("wallet_request".to_string(), wallet_request);
    obj.insert(SYSTEM_NEXT_ACTION_KEY.to_string(), action_value);

    Ok(result)
}

// ============================================================================
// Tool 1: SearchPolymarket
// ============================================================================

pub(crate) struct SearchPolymarket;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchPolymarketArgs {
    /// Maximum number of markets to return (default: 100, max: 1000)
    pub(crate) limit: Option<u32>,
    /// Pagination offset (default: 0)
    pub(crate) offset: Option<u32>,
    /// Filter for active markets
    pub(crate) active: Option<bool>,
    /// Filter for closed markets
    pub(crate) closed: Option<bool>,
    /// Filter for archived markets
    pub(crate) archived: Option<bool>,
    /// Filter by tag/category (e.g., 'crypto', 'sports', 'politics')
    pub(crate) tag: Option<String>,
}

impl DynAomiTool for SearchPolymarket {
    type App = PolymarketApp;
    type Args = SearchPolymarketArgs;
    const NAME: &'static str = "search_polymarket";
    const DESCRIPTION: &'static str = "Query Polymarket prediction markets with filtering options. Returns a list of markets with their current prices, volumes, liquidity, and other metadata.";

    fn run(_app: &PolymarketApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = PolymarketClient::new()?;
        let params = GetMarketsParams {
            limit: args.limit,
            offset: args.offset,
            active: args.active,
            closed: args.closed,
            archived: args.archived,
            tag: args.tag,
        };

        let markets = client.get_markets(&params)?;
        let formatted: Vec<Value> = markets
            .iter()
            .map(|m| {
                json!({
                    "id": m.id,
                    "question": m.question,
                    "slug": m.slug,
                    "outcomes": m.outcomes,
                    "outcome_prices": m.outcome_prices,
                    "volume": m.volume_num,
                    "liquidity": m.liquidity_num,
                    "active": m.active,
                    "closed": m.closed,
                    "category": m.category,
                    "start_date": m.start_date,
                    "end_date": m.end_date,
                })
            })
            .collect();

        Ok(json!({
            "markets_count": formatted.len(),
            "markets": formatted,
        }))
    }
}

// ============================================================================
// Tool 2: GetPolymarketDetails
// ============================================================================

pub(crate) struct GetPolymarketDetails;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPolymarketDetailsArgs {
    /// Market ID, slug (e.g., 'will-bitcoin-reach-100k-by-2025'), or condition ID (0x-prefixed)
    market_id_or_slug: String,
}

impl DynAomiTool for GetPolymarketDetails {
    type App = PolymarketApp;
    type Args = GetPolymarketDetailsArgs;
    const NAME: &'static str = "get_polymarket_details";
    const DESCRIPTION: &'static str =
        "Get detailed information about a specific Polymarket prediction market by its ID or slug.";

    fn run(_app: &PolymarketApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = PolymarketClient::new()?;
        let market = client.get_market(&args.market_id_or_slug)?;
        let (mut yes_token_id, mut no_token_id) = extract_outcome_token_ids(&market);
        let mut tokens = market.extra.get("tokens").cloned();
        if (yes_token_id.is_none() || no_token_id.is_none())
            && let Some(condition_id) = market.condition_id.as_deref()
            && let Ok((sdk_yes, sdk_no, sdk_tokens)) = fetch_clob_outcome_token_ids(condition_id)
        {
            if yes_token_id.is_none() {
                yes_token_id = sdk_yes;
            }
            if no_token_id.is_none() {
                no_token_id = sdk_no;
            }
            if tokens.is_none() {
                tokens = sdk_tokens;
            }
        }

        Ok(json!({
            "id": market.id,
            "question": market.question,
            "slug": market.slug,
            "condition_id": market.condition_id,
            "yes_token_id": yes_token_id,
            "no_token_id": no_token_id,
            "description": market.description,
            "outcomes": market.outcomes,
            "outcome_prices": market.outcome_prices,
            "tokens": tokens,
            "volume": market.volume,
            "volume_num": market.volume_num,
            "liquidity": market.liquidity,
            "liquidity_num": market.liquidity_num,
            "start_date": market.start_date,
            "end_date": market.end_date,
            "image": market.image,
            "active": market.active,
            "closed": market.closed,
            "archived": market.archived,
            "category": market.category,
            "market_type": market.market_type,
        }))
    }
}

// ============================================================================
// Tool 3: GetPolymarketTrades
// ============================================================================

pub(crate) struct GetPolymarketTrades;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPolymarketTradesArgs {
    /// Maximum number of trades to return (default: 100, max: 10000)
    limit: Option<u32>,
    /// Pagination offset (default: 0)
    offset: Option<u32>,
    /// Filter by market condition ID (comma-separated for multiple)
    market: Option<String>,
    /// Filter by user wallet address (0x-prefixed)
    user: Option<String>,
    /// Filter by trade side ('BUY' or 'SELL')
    side: Option<String>,
}

impl DynAomiTool for GetPolymarketTrades {
    type App = PolymarketApp;
    type Args = GetPolymarketTradesArgs;
    const NAME: &'static str = "get_polymarket_trades";
    const DESCRIPTION: &'static str = "Retrieve historical trades from Polymarket. Returns trade history with timestamps, prices, sizes, and user information.";

    fn run(_app: &PolymarketApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = PolymarketClient::new()?;
        let params = GetTradesParams {
            limit: args.limit,
            offset: args.offset,
            market: args.market,
            user: args.user,
            side: args.side,
        };

        let trades = client.get_trades(&params)?;
        let formatted: Vec<Value> = trades
            .iter()
            .map(|t| {
                json!({
                    "id": t.id,
                    "market": t.market,
                    "asset": t.asset,
                    "side": t.side,
                    "size": t.size,
                    "price": t.price,
                    "timestamp": t.timestamp,
                    "transaction_hash": t.transaction_hash,
                    "outcome": t.outcome,
                    "proxy_wallet": t.proxy_wallet,
                    "condition_id": t.condition_id,
                    "title": t.title,
                    "slug": t.slug,
                })
            })
            .collect();

        Ok(json!({
            "trades_count": formatted.len(),
            "trades": formatted,
        }))
    }
}

// ============================================================================
// Tool 4: ResolvePolymarketTradeIntent
// ============================================================================

pub(crate) struct ResolvePolymarketTradeIntent;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ResolvePolymarketTradeIntentArgs {
    /// Raw user request, e.g. "buy yes trump 2028 for $100"
    user_request: String,
    /// Number of ranked candidates to return (default: 5, max: 20)
    candidate_limit: Option<u32>,
    /// Number of open markets to search for ranking (default: 200, max: 1000)
    search_market_limit: Option<u32>,
}

impl DynAomiTool for ResolvePolymarketTradeIntent {
    type App = PolymarketApp;
    type Args = ResolvePolymarketTradeIntentArgs;
    const NAME: &'static str = "resolve_polymarket_trade_intent";
    const DESCRIPTION: &'static str = "Parse a natural language trading request and return ranked relevant Polymarket candidates. If ambiguous, indicates that user selection is required.";

    fn run(_app: &PolymarketApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let intent = parse_trade_intent(&args.user_request)?;
        let search_market_limit = args
            .search_market_limit
            .unwrap_or(DEFAULT_INTENT_SEARCH_MARKET_LIMIT)
            .clamp(1, MAX_INTENT_SEARCH_MARKET_LIMIT);
        let candidate_limit = args
            .candidate_limit
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_INTENT_CANDIDATE_LIMIT)
            .clamp(1, 20);

        let search_params = GetMarketsParams {
            limit: Some(search_market_limit),
            offset: Some(0),
            active: Some(true),
            closed: Some(false),
            archived: Some(false),
            tag: None,
        };

        let client = PolymarketClient::new()?;
        let markets = client.get_markets(&search_params)?;

        let ranked = rank_market_candidates(&intent, &markets);
        let top1_score = ranked.first().map(|c| c.score).unwrap_or(0.0);
        let top2_score = ranked.get(1).map(|c| c.score);
        let requires_user_selection = requires_selection(top1_score, top2_score);
        let candidates: Vec<_> = ranked.into_iter().take(candidate_limit).collect();

        let selection_reason = if candidates.is_empty() {
            Some("No relevant active Polymarket markets found for this request.".to_string())
        } else if requires_user_selection {
            Some(
                "Multiple relevant markets match this request. User must choose a candidate_id before placing an order."
                    .to_string(),
            )
        } else {
            None
        };

        Ok(json!({
            "user_request": args.user_request,
            "parsed_intent": {
                "action": intent.action,
                "outcome": intent.outcome,
                "year": intent.year,
                "size_usd": intent.size_usd,
                "search_query": intent.search_query,
            },
            "requires_selection": requires_user_selection,
            "selection_reason": selection_reason,
            "candidate_count": candidates.len(),
            "recommended_candidate_id": if !requires_user_selection && !candidates.is_empty() { Some("C1") } else { None::<&str> },
            "candidates": candidates.iter().enumerate().map(|(idx, c)| json!({
                "candidate_id": format!("C{}", idx + 1),
                "market_id": c.market_id,
                "condition_id": c.condition_id,
                "question": c.question,
                "slug": c.slug,
                "close_time": c.close_time,
                "yes_price": c.yes_price,
                "no_price": c.no_price,
                "volume": c.volume,
                "liquidity": c.liquidity,
                "score": c.score,
                "url": c.url,
            })).collect::<Vec<_>>(),
            "next_step_hint": if requires_user_selection {
                Some("Reply with candidate_id and outcome (YES/NO), e.g. 'C2 YES'.")
            } else { None::<&str> },
        }))
    }
}

// ============================================================================
// Tool 5: BuildPolymarketOrder
// ============================================================================

pub(crate) struct BuildPolymarketOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BuildPolymarketOrderArgs {
    /// Market id, slug, or condition id selected by the user.
    market_id_or_slug: String,
    /// Desired outcome: YES or NO.
    outcome: String,
    /// Optional side (default: BUY).
    side: Option<String>,
    /// Optional notional in USDC.
    size_usd: Option<f64>,
    /// Optional explicit shares quantity.
    shares: Option<f64>,
    /// Optional limit price in (0, 1]. If omitted, build a market order plan.
    limit_price: Option<f64>,
    /// Optional order type. Limit: GTC/FOK/GTD/FAK. Market: FOK/FAK.
    order_type: Option<String>,
    /// Optional post-only flag for limit orders.
    post_only: Option<bool>,
    /// Optional signature type override: proxy, eoa, or gnosis-safe.
    signature_type: Option<String>,
    /// Optional Polymarket funder override.
    funder: Option<String>,
    /// Optional wallet address override for wallet-mode execution.
    wallet_address: Option<String>,
}

impl DynAomiTool for BuildPolymarketOrder {
    type App = PolymarketApp;
    type Args = BuildPolymarketOrderArgs;
    const NAME: &'static str = "build_polymarket_order";
    const DESCRIPTION: &'static str = "Build a canonical Polymarket order plan. Preferred behavior: return a preview plus submit_args_template. If wallet signing is required, also return SYSTEM_NEXT_ACTION with the exact signing step.";

    fn run(_app: &PolymarketApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let connected_wallet = args
            .wallet_address
            .clone()
            .or_else(|| ctx.attribute_string(&["domain", "evm", "address"]));
        let (execution_mode, wallet_address) =
            determine_polymarket_execution(connected_wallet.as_deref())?;

        let client = PolymarketClient::new()?;
        let market = client.get_market(&args.market_id_or_slug)?;
        let (mut yes_token_id, mut no_token_id) = extract_outcome_token_ids(&market);
        let mut tokens = market.extra.get("tokens").cloned();
        if (yes_token_id.is_none() || no_token_id.is_none())
            && let Some(condition_id) = market.condition_id.as_deref()
            && let Ok((sdk_yes, sdk_no, sdk_tokens)) = fetch_clob_outcome_token_ids(condition_id)
        {
            if yes_token_id.is_none() {
                yes_token_id = sdk_yes;
            }
            if no_token_id.is_none() {
                no_token_id = sdk_no;
            }
            if tokens.is_none() {
                tokens = sdk_tokens;
            }
        }

        let plan = build_polymarket_order_plan_from_market(
            &market,
            &args.market_id_or_slug,
            &args.outcome,
            args.side.as_deref(),
            args.size_usd,
            args.shares,
            args.limit_price,
            args.order_type.as_deref(),
            args.post_only,
            args.signature_type.as_deref(),
            args.funder.as_deref(),
            &execution_mode,
            wallet_address.as_deref(),
        )?;

        let (yes_price, no_price) = extract_yes_no_prices(&market);
        let mut result = json!({
            "source": "polymarket",
            "execution_mode": plan.execution_mode,
            "market": {
                "market_id": market.id,
                "slug": market.slug,
                "condition_id": market.condition_id,
                "question": market.question,
                "close_time": market.end_date,
                "yes_price": yes_price,
                "no_price": no_price,
                "yes_token_id": yes_token_id,
                "no_token_id": no_token_id,
                "tokens": tokens,
            },
            "order_preview": {
                "order_kind": plan.order_kind,
                "side": plan.side,
                "outcome": plan.outcome,
                "token_id": plan.token_id,
                "amount": plan.amount,
                "amount_kind": plan.amount_kind,
                "price": plan.price,
                "size": plan.size,
                "reference_price": plan.reference_price,
                "estimated_shares": plan.estimated_shares,
                "order_type": plan.order_type,
                "post_only": plan.post_only,
            },
            "requires_user_confirmation": true,
            "confirmation_phrase": "confirm",
            "warnings": plan.warnings,
            "submit_args_template": {
                "confirmation": "confirm",
                "order_plan": plan.clone(),
            },
            "next_step_hint": "After the user confirms, call submit_polymarket_order with submit_args_template unless SYSTEM_NEXT_ACTION already defines the next step.",
        });

        if plan.execution_mode == "WALLET" {
            let clob_auth = build_clob_auth_context(
                plan.wallet_address
                    .as_deref()
                    .ok_or_else(|| "wallet mode requires wallet_address".to_string())?,
            );
            let wallet_request = json!({
                "typed_data": build_clob_auth_typed_data(&clob_auth),
                "description": "Polymarket CLOB auth: sign to prepare order submission",
            });
            let obj = result
                .as_object_mut()
                .ok_or_else(|| "result is not an object".to_string())?;
            obj.insert(
                "submit_args_template".to_string(),
                json!({
                    "confirmation": "confirm",
                    "order_plan": plan.clone(),
                    "clob_auth": clob_auth.clone(),
                }),
            );

            return build_polymarket_follow_up_result(
                result,
                "send_eip712_to_wallet",
                wallet_request,
                "submit_polymarket_order",
                json!({
                    "confirmation": "confirm",
                    "order_plan": plan,
                    "clob_auth": clob_auth,
                }),
                "Create or derive Polymarket credentials, build the exact order payload, and request the final order signature.",
                "clob_l1_signature",
            );
        }

        Ok(result)
    }
}

// ============================================================================
// Tool 6: SubmitPolymarketOrder
// ============================================================================

pub(crate) struct SubmitPolymarketOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SubmitPolymarketOrderArgs {
    /// Explicit confirmation token; must be "confirm".
    confirmation: Option<String>,
    /// Canonical order plan returned by build_polymarket_order.
    order_plan: PolymarketOrderPlan,
    /// Optional override private key for direct SDK execution.
    private_key: Option<String>,
    /// Wallet-mode CLOB auth context returned by build_polymarket_order.
    clob_auth: Option<ClobAuthContext>,
    /// Wallet signature for the ClobAuth EIP-712 payload.
    clob_l1_signature: Option<String>,
    /// Prepared exact order returned by a previous submit_polymarket_order wallet stage.
    prepared_order: Option<PreparedPolymarketOrder>,
    /// Wallet signature for the final Polymarket order EIP-712 payload.
    order_signature: Option<String>,
}

impl DynAomiTool for SubmitPolymarketOrder {
    type App = PolymarketApp;
    type Args = SubmitPolymarketOrderArgs;
    const NAME: &'static str = "submit_polymarket_order";
    const DESCRIPTION: &'static str = "Execute a canonical Polymarket order plan. Direct mode submits through the official SDK. Wallet mode returns the exact next signing step or submits the final wallet-signed order.";

    fn run(_app: &PolymarketApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        validate_confirmation_token(args.confirmation.as_deref())?;

        if args.order_plan.execution_mode == "DIRECT_SDK" {
            return submit_direct_order_plan(&args.order_plan, args.private_key.as_deref());
        }

        let clob_auth = args.clob_auth.clone().ok_or_else(|| {
            "wallet-mode submit requires clob_auth from build_polymarket_order".to_string()
        })?;
        let clob_l1_signature = args.clob_l1_signature.clone().ok_or_else(|| {
            "wallet-mode submit requires clob_l1_signature from the ClobAuth wallet callback"
                .to_string()
        })?;

        if let Some(prepared_order) = args.prepared_order.clone() {
            if let Some(order_signature) = args.order_signature.as_deref() {
                return submit_wallet_signed_order(
                    &args.order_plan,
                    &clob_auth,
                    &clob_l1_signature,
                    &prepared_order,
                    order_signature,
                );
            }

            let result = json!({
                "source": "polymarket",
                "execution_mode": "WALLET",
                "stage": "awaiting_order_signature",
                "prepared_order": prepared_order,
                "submit_args_template": {
                    "confirmation": "confirm",
                    "order_plan": args.order_plan.clone(),
                    "clob_auth": clob_auth.clone(),
                    "clob_l1_signature": clob_l1_signature.clone(),
                    "prepared_order": prepared_order.clone(),
                },
            });
            let wallet_request = json!({
                "typed_data": build_order_typed_data(&prepared_order),
                "description": build_prepared_order_description(&args.order_plan),
            });
            return build_polymarket_follow_up_result(
                result,
                "send_eip712_to_wallet",
                wallet_request,
                "submit_polymarket_order",
                json!({
                    "confirmation": "confirm",
                    "order_plan": args.order_plan,
                    "clob_auth": clob_auth,
                    "clob_l1_signature": clob_l1_signature,
                    "prepared_order": prepared_order,
                }),
                "Submit the exact prepared Polymarket order after the wallet signs it.",
                "order_signature",
            );
        }

        let (prepared_order, typed_data, funder_address) =
            prepare_wallet_order_signature(&args.order_plan, &clob_auth, &clob_l1_signature)?;
        let result = json!({
            "source": "polymarket",
            "execution_mode": "WALLET",
            "stage": "awaiting_order_signature",
            "funder_address": funder_address.map(|addr| addr.to_string()),
            "prepared_order": prepared_order,
            "submit_args_template": {
                "confirmation": "confirm",
                "order_plan": args.order_plan.clone(),
                "clob_auth": clob_auth.clone(),
                "clob_l1_signature": clob_l1_signature.clone(),
                "prepared_order": prepared_order.clone(),
            },
        });
        let wallet_request = json!({
            "typed_data": typed_data,
            "description": build_prepared_order_description(&args.order_plan),
        });

        build_polymarket_follow_up_result(
            result,
            "send_eip712_to_wallet",
            wallet_request,
            "submit_polymarket_order",
            json!({
                "confirmation": "confirm",
                "order_plan": args.order_plan,
                "clob_auth": clob_auth,
                "clob_l1_signature": clob_l1_signature,
                "prepared_order": prepared_order,
            }),
            "Submit the exact prepared Polymarket order after the wallet signs it.",
            "order_signature",
        )
    }
}
