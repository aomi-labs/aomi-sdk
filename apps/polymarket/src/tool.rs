use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

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

        Ok(json!({
            "id": market.id,
            "question": market.question,
            "slug": market.slug,
            "condition_id": market.condition_id,
            "description": market.description,
            "outcomes": market.outcomes,
            "outcome_prices": market.outcome_prices,
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
// Tool 5: BuildPolymarketOrderPreview
// ============================================================================

pub(crate) struct BuildPolymarketOrderPreview;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BuildPolymarketOrderPreviewArgs {
    /// Market id or slug selected by user (typically from resolve_polymarket_trade_intent candidates)
    market_id_or_slug: String,
    /// Desired outcome side: YES or NO
    outcome: String,
    /// Optional side (default: BUY)
    side: Option<String>,
    /// Optional notional in USD/USDC
    size_usd: Option<f64>,
    /// Optional explicit shares quantity
    shares: Option<f64>,
    /// Optional limit price in [0, 1]
    limit_price: Option<f64>,
    /// Optional order time in force (e.g., GTC, IOC)
    time_in_force: Option<String>,
}

impl DynAomiTool for BuildPolymarketOrderPreview {
    type App = PolymarketApp;
    type Args = BuildPolymarketOrderPreviewArgs;
    const NAME: &'static str = "build_polymarket_order_preview";
    const DESCRIPTION: &'static str = "Build a deterministic order preview (token_id, side, price/size interpretation) and require explicit user confirmation before submission.";

    fn run(_app: &PolymarketApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let size_mode = match (args.size_usd, args.shares) {
            (Some(_), Some(_)) => {
                return Err("Provide either size_usd or shares, not both.".to_string());
            }
            (None, None) => {
                return Err("Missing order size. Provide either size_usd or shares.".to_string());
            }
            (Some(v), None) if v > 0.0 => ("usd", v),
            (None, Some(v)) if v > 0.0 => ("shares", v),
            _ => return Err("Size values must be positive numbers.".to_string()),
        };

        let side = normalize_side(args.side.as_deref())?;
        let outcome = normalize_yes_no(&args.outcome)?;

        if let Some(price) = args.limit_price
            && (!(0.0..=1.0).contains(&price) || price == 0.0)
        {
            return Err("limit_price must be within (0, 1].".to_string());
        }

        let client = PolymarketClient::new()?;
        let market = client.get_market(&args.market_id_or_slug)?;

        let (yes_price, no_price) = extract_yes_no_prices(&market);
        let market_price = if outcome == "YES" {
            yes_price
        } else {
            no_price
        };
        let execution_price = args.limit_price.or(market_price);

        let (yes_token_id, no_token_id) = extract_outcome_token_ids(&market);
        let token_id = if outcome == "YES" {
            yes_token_id
        } else {
            no_token_id
        };

        if token_id.is_none() {
            return Err(format!(
                "Failed to resolve token_id for outcome {} from market metadata.",
                outcome
            ));
        }

        let estimated_shares = if size_mode.0 == "usd" {
            execution_price.and_then(|px| {
                if px > 0.0 {
                    Some(size_mode.1 / px)
                } else {
                    None
                }
            })
        } else {
            Some(size_mode.1)
        };

        let mut warnings = Vec::<String>::new();
        if market_price.is_none() {
            warnings.push(
                "Live outcome price unavailable from market metadata; provide explicit limit_price before submission."
                    .to_string(),
            );
        }
        if size_mode.0 == "usd" && execution_price.is_none() {
            warnings.push(
                "Unable to estimate shares because no reference price is available.".to_string(),
            );
        }
        if size_mode.0 == "usd" && execution_price.is_some_and(|px| px <= 0.0) {
            warnings.push(
                "Unable to estimate shares because reference or limit price is zero.".to_string(),
            );
        }

        Ok(json!({
            "market": {
                "market_id": market.id,
                "slug": market.slug,
                "condition_id": market.condition_id,
                "question": market.question,
                "close_time": market.end_date,
                "yes_price": yes_price,
                "no_price": no_price,
            },
            "order_preview": {
                "side": side,
                "outcome": outcome,
                "token_id": token_id,
                "size_mode": size_mode.0,
                "size_value": size_mode.1,
                "limit_price": args.limit_price,
                "reference_price": market_price,
                "execution_price": execution_price,
                "estimated_shares": estimated_shares,
                "time_in_force": args.time_in_force,
            },
            "requires_user_confirmation": true,
            "confirmation_phrase": "confirm",
            "warnings": warnings,
            "next_step_hint": "After user confirms, construct/sign the final order and call place_polymarket_order.",
        }))
    }
}

// ============================================================================
// Tool 6: GetPolymarketClobSignature
// ============================================================================

pub(crate) struct GetPolymarketClobSignature;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPolymarketClobSignatureArgs {
    /// Optional override for Unix timestamp (seconds as string). If omitted, uses current UTC time.
    timestamp: Option<String>,
    /// Optional nonce for ClobAuth (default: "0")
    nonce: Option<String>,
    /// Optional override for the ClobAuth attestation message.
    message: Option<String>,
}

impl DynAomiTool for GetPolymarketClobSignature {
    type App = PolymarketApp;
    type Args = GetPolymarketClobSignatureArgs;
    const NAME: &'static str = "get_polymarket_clob_signature";
    const DESCRIPTION: &'static str = "Build the canonical Polymarket ClobAuth EIP-712 typed data payload. Returns address, timestamp, nonce, and the full EIP-712 typed_data JSON. The host should then use `send_eip712_to_wallet` to request a signature, and pass that signature to ensure_polymarket_clob_credentials.";

    fn run(_app: &PolymarketApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let address = ctx
            .attribute_string(&["domain", "evm", "address"])
            .ok_or_else(|| {
                "No wallet connected. Ask the user to run /connect first.".to_string()
            })?;

        let timestamp = args
            .timestamp
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| {
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs().to_string())
                    .unwrap_or_else(|_| "0".to_string())
            });

        if !timestamp.chars().all(|c| c.is_ascii_digit()) {
            return Err("timestamp must be Unix seconds as a numeric string".to_string());
        }

        let nonce = args
            .nonce
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "0".to_string());
        let nonce_u64: u64 = nonce
            .parse()
            .map_err(|_| "nonce must be an unsigned integer string".to_string())?;

        let message = args
            .message
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "This message attests that I control the given wallet".to_string());

        let typed_data = json!({
            "domain": {
                "name": "ClobAuthDomain",
                "version": "1",
                "chainId": 137
            },
            "types": {
                "EIP712Domain": [
                    { "name": "name", "type": "string" },
                    { "name": "version", "type": "string" },
                    { "name": "chainId", "type": "uint256" }
                ],
                "ClobAuth": [
                    { "name": "address", "type": "address" },
                    { "name": "timestamp", "type": "string" },
                    { "name": "nonce", "type": "uint256" },
                    { "name": "message", "type": "string" }
                ]
            },
            "primaryType": "ClobAuth",
            "message": {
                "address": address,
                "timestamp": timestamp,
                "nonce": nonce_u64,
                "message": message
            }
        });

        Ok(json!({
            "address": typed_data["message"]["address"],
            "timestamp": typed_data["message"]["timestamp"],
            "nonce": nonce,
            "typed_data": typed_data,
            "description": "Polymarket CLOB auth (L1): sign to create/derive API credentials",
            "next_step": "Use the host's send_eip712_to_wallet tool to send typed_data to the wallet. After signature is produced, call ensure_polymarket_clob_credentials with the exact same address/timestamp/nonce and returned signature."
        }))
    }
}

// ============================================================================
// Tool 7: EnsurePolymarketClobCredentials
// ============================================================================

pub(crate) struct EnsurePolymarketClobCredentials;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct EnsurePolymarketClobCredentialsArgs {
    /// Wallet address used for CLOB L1 authentication (must match what was signed in get_polymarket_clob_signature)
    address: String,
    /// L1 signature from wallet for CLOB auth challenge (from send_eip712_to_wallet result)
    signature: String,
    /// Timestamp used in the L1 signature (must be identical to get_polymarket_clob_signature output)
    timestamp: String,
    /// Optional nonce for CLOB L1 auth (default: "0")
    nonce: Option<String>,
}

impl DynAomiTool for EnsurePolymarketClobCredentials {
    type App = PolymarketApp;
    type Args = EnsurePolymarketClobCredentialsArgs;
    const NAME: &'static str = "ensure_polymarket_clob_credentials";
    const DESCRIPTION: &'static str = "Create or derive Polymarket CLOB API credentials (key, secret, passphrase) from L1 auth headers. Requires address/signature/timestamp from the EIP-712 signing step (get_polymarket_clob_signature + send_eip712_to_wallet).";

    fn run(_app: &PolymarketApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let l1_auth = ClobL1Auth {
            address: args.address,
            signature: args.signature,
            timestamp: args.timestamp,
            nonce: args.nonce,
        };

        let client = PolymarketClient::new()?;
        let creds = client.create_or_derive_api_credentials(&l1_auth)?;

        Ok(json!({
            "api_key": creds.key,
            "api_secret": creds.secret,
            "passphrase": creds.passphrase,
        }))
    }
}

// ============================================================================
// Tool 8: PlacePolymarketOrder
// ============================================================================

pub(crate) struct PlacePolymarketOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ClobAuthArgs {
    /// Existing CLOB API key
    api_key: Option<String>,
    /// Existing CLOB API secret
    api_secret: Option<String>,
    /// Existing CLOB passphrase
    passphrase: Option<String>,
    /// L1 auth payload used to create/derive API creds when credentials are missing
    l1_auth: Option<ClobL1AuthArgs>,
    /// Optional precomputed L2 headers (if omitted, backend computes from secret)
    l2_auth: Option<ClobL2AuthArgs>,
    /// Auto bootstrap credentials when missing (default: true)
    auto_create_or_derive: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ClobL1AuthArgs {
    /// Wallet address used for L1 auth
    address: String,
    /// L1 signature for CLOB auth
    signature: String,
    /// L1 timestamp used in signature
    timestamp: String,
    /// Optional L1 nonce (default: 0)
    nonce: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ClobL2AuthArgs {
    /// Precomputed POLY_SIGNATURE
    signature: Option<String>,
    /// POLY_TIMESTAMP used in L2 signature
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlacePolymarketOrderArgs {
    /// Explicit user confirmation token; must be "confirm"
    confirmation: Option<String>,
    /// Wallet address (0x-prefixed) that signed the order
    owner: String,
    /// 0x signature returned from the wallet
    signature: String,
    /// JSON object describing the order payload per Polymarket docs
    order: Value,
    /// Optional client order id for idempotency
    client_id: Option<String>,
    /// Optional override URL for the orders endpoint
    endpoint: Option<String>,
    /// Optional API key value inserted as X-API-KEY
    api_key: Option<String>,
    /// Optional CLOB auth bundle for L2 headers and automatic create/derive bootstrap via L1 auth
    clob_auth: Option<ClobAuthArgs>,
    /// Optional JSON object with additional top-level fields to merge into the request
    extra_fields: Option<Value>,
}

impl DynAomiTool for PlacePolymarketOrder {
    type App = PolymarketApp;
    type Args = PlacePolymarketOrderArgs;
    const NAME: &'static str = "place_polymarket_order";
    const DESCRIPTION: &'static str = "Submit a signed Polymarket order to the CLOB API. Provide the wallet address that signed, the 0x signature string, and the order JSON. Requires confirmation='confirm'.";

    fn run(_app: &PolymarketApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        validate_confirmation_token(args.confirmation.as_deref())?;

        let clob_auth = args.clob_auth.map(|auth| {
            let has_any_creds =
                auth.api_key.is_some() || auth.api_secret.is_some() || auth.passphrase.is_some();
            let credentials = if has_any_creds {
                Some(ClobApiCredentials {
                    key: auth.api_key.unwrap_or_default(),
                    secret: auth.api_secret.unwrap_or_default(),
                    passphrase: auth.passphrase.unwrap_or_default(),
                })
            } else {
                None
            };

            ClobAuthBundle {
                credentials,
                l1_auth: auth.l1_auth.map(|l1| ClobL1Auth {
                    address: l1.address,
                    signature: l1.signature,
                    timestamp: l1.timestamp,
                    nonce: l1.nonce,
                }),
                l2: auth.l2_auth.map(|l2| ClobL2Auth {
                    signature: l2.signature,
                    timestamp: l2.timestamp,
                }),
                auto_create_or_derive: auth.auto_create_or_derive,
            }
        });

        let order = match args.order {
            Value::String(raw) => serde_json::from_str::<Value>(&raw)
                .map_err(|e| format!("order must be valid JSON string: {e}"))?,
            other => other,
        };

        let request = SubmitOrderRequest {
            owner: args.owner,
            signature: args.signature,
            order,
            client_id: args.client_id,
            endpoint: args.endpoint,
            api_key: args.api_key,
            clob_auth,
            extra_fields: args.extra_fields,
        };

        let client = PolymarketClient::new()?;
        client.submit_order(request)
    }
}
