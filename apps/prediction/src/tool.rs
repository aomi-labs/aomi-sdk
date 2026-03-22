use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{json, Value};

impl DynAomiTool for SearchPolymarket {
    type App = PredictionApp;
    type Args = SearchPolymarketArgs;
    const NAME: &'static str = "search_polymarket";
    const DESCRIPTION: &'static str = "Query Polymarket prediction markets with filtering options. Returns a list of markets with their current prices, volumes, liquidity, and other metadata.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
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
                    "id": m.get("id"),
                    "question": m.get("question"),
                    "slug": m.get("slug"),
                    "outcomes": m.get("outcomes"),
                    "outcome_prices": m.get("outcomePrices").or_else(|| m.get("outcome_prices")),
                    "volume": m.get("volumeNum").or_else(|| m.get("volume_num")),
                    "liquidity": m.get("liquidityNum").or_else(|| m.get("liquidity_num")),
                    "active": m.get("active"),
                    "closed": m.get("closed"),
                    "category": m.get("category"),
                    "start_date": m.get("startDate").or_else(|| m.get("start_date")),
                    "end_date": m.get("endDate").or_else(|| m.get("end_date")),
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
    /// Market ID or slug (e.g., 'will-bitcoin-reach-100k-by-2025' or market ID)
    market_id_or_slug: String,
}

impl DynAomiTool for GetPolymarketDetails {
    type App = PredictionApp;
    type Args = GetPolymarketDetailsArgs;
    const NAME: &'static str = "get_polymarket_details";
    const DESCRIPTION: &'static str =
        "Get detailed information about a specific Polymarket prediction market by its ID or slug.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = PolymarketClient::new()?;
        let market = client.get_market(&args.market_id_or_slug)?;
        Ok(json!({
            "id": market.get("id"),
            "question": market.get("question"),
            "slug": market.get("slug"),
            "condition_id": market.get("conditionId").or_else(|| market.get("condition_id")),
            "description": market.get("description"),
            "outcomes": market.get("outcomes"),
            "outcome_prices": market.get("outcomePrices").or_else(|| market.get("outcome_prices")),
            "volume": market.get("volume"),
            "volume_num": market.get("volumeNum").or_else(|| market.get("volume_num")),
            "liquidity": market.get("liquidity"),
            "liquidity_num": market.get("liquidityNum").or_else(|| market.get("liquidity_num")),
            "start_date": market.get("startDate").or_else(|| market.get("start_date")),
            "end_date": market.get("endDate").or_else(|| market.get("end_date")),
            "image": market.get("image"),
            "active": market.get("active"),
            "closed": market.get("closed"),
            "archived": market.get("archived"),
            "category": market.get("category"),
            "market_type": market.get("marketType").or_else(|| market.get("market_type")),
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
    type App = PredictionApp;
    type Args = GetPolymarketTradesArgs;
    const NAME: &'static str = "get_polymarket_trades";
    const DESCRIPTION: &'static str = "Retrieve historical trades from Polymarket. Returns trade history with timestamps, prices, sizes, and user information.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
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
                    "id": t.get("id"),
                    "market": t.get("market"),
                    "asset": t.get("asset"),
                    "side": t.get("side"),
                    "size": t.get("size"),
                    "price": t.get("price"),
                    "timestamp": t.get("timestamp"),
                    "transaction_hash": t.get("transactionHash").or_else(|| t.get("transaction_hash")),
                    "outcome": t.get("outcome"),
                    "proxy_wallet": t.get("proxyWallet").or_else(|| t.get("proxy_wallet")),
                    "condition_id": t.get("conditionId").or_else(|| t.get("condition_id")),
                    "title": t.get("title"),
                    "slug": t.get("slug"),
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
    type App = PredictionApp;
    type Args = ResolvePolymarketTradeIntentArgs;
    const NAME: &'static str = "resolve_polymarket_trade_intent";
    const DESCRIPTION: &'static str = "Parse a natural language trading request and return ranked relevant Polymarket candidates. If ambiguous, indicates that user selection is required.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
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

        let client = PolymarketClient::new()?;
        let params = GetMarketsParams {
            limit: Some(search_market_limit),
            offset: Some(0),
            active: Some(true),
            closed: Some(false),
            archived: Some(false),
            tag: None,
        };
        let markets = client.get_markets(&params)?;
        let ranked = rank_market_candidates(&intent, &markets);

        let top1_score = ranked
            .first()
            .and_then(|c| c.get("score"))
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let top2_score = ranked
            .get(1)
            .and_then(|c| c.get("score"))
            .and_then(Value::as_f64);
        let needs_selection = requires_selection(top1_score, top2_score);
        let candidates: Vec<Value> = ranked
            .into_iter()
            .take(candidate_limit)
            .enumerate()
            .map(|(idx, mut c)| {
                if let Some(obj) = c.as_object_mut() {
                    obj.insert(
                        "candidate_id".to_string(),
                        Value::String(format!("C{}", idx + 1)),
                    );
                }
                c
            })
            .collect();

        let selection_reason = if candidates.is_empty() {
            Some("No relevant active Polymarket markets found for this request.")
        } else if needs_selection {
            Some("Multiple relevant markets match this request. User must choose a candidate_id before placing an order.")
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
            "requires_selection": needs_selection,
            "selection_reason": selection_reason,
            "candidate_count": candidates.len(),
            "recommended_candidate_id": if !needs_selection && !candidates.is_empty() { Some("C1") } else { None::<&str> },
            "candidates": candidates,
            "next_step_hint": if needs_selection { Some("Reply with candidate_id and outcome (YES/NO), e.g. 'C2 YES'.") } else { None::<&str> },
        }))
    }
}

// ============================================================================
// Tool 5: BuildPolymarketOrderPreview
// ============================================================================

pub(crate) struct BuildPolymarketOrderPreview;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BuildPolymarketOrderPreviewArgs {
    /// Market id or slug selected by user
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
    type App = PredictionApp;
    type Args = BuildPolymarketOrderPreviewArgs;
    const NAME: &'static str = "build_polymarket_order_preview";
    const DESCRIPTION: &'static str = "Build a deterministic order preview (token_id, side, price/size interpretation) and require explicit user confirmation before submission.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let size_mode = match (args.size_usd, args.shares) {
            (Some(_), Some(_)) => return Err("Provide either size_usd or shares, not both.".into()),
            (None, None) => {
                return Err("Missing order size. Provide either size_usd or shares.".into())
            }
            (Some(v), None) if v > 0.0 => ("usd", v),
            (None, Some(v)) if v > 0.0 => ("shares", v),
            _ => return Err("Size values must be positive numbers.".into()),
        };

        let side = match args.side.as_deref() {
            None => "BUY".to_string(),
            Some(v) => match v.trim().to_ascii_uppercase().as_str() {
                "BUY" => "BUY".to_string(),
                "SELL" => "SELL".to_string(),
                _ => return Err("side must be BUY or SELL".into()),
            },
        };

        let outcome = match args.outcome.trim().to_ascii_lowercase().as_str() {
            "yes" | "y" => "YES".to_string(),
            "no" | "n" => "NO".to_string(),
            _ => return Err("outcome must be YES or NO".into()),
        };

        if let Some(price) = args.limit_price {
            if !(0.0..=1.0).contains(&price) || price == 0.0 {
                return Err("limit_price must be within (0, 1].".into());
            }
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
            warnings.push("Live outcome price unavailable from market metadata; provide explicit limit_price before submission.".to_string());
        }
        if size_mode.0 == "usd" && execution_price.is_none() {
            warnings.push(
                "Unable to estimate shares because no reference price is available.".to_string(),
            );
        }

        Ok(json!({
            "market": {
                "market_id": market.get("id"),
                "slug": market.get("slug"),
                "condition_id": market.get("conditionId").or_else(|| market.get("condition_id")),
                "question": market.get("question"),
                "close_time": market.get("endDate").or_else(|| market.get("end_date")),
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
    /// Wallet address (0x-prefixed) to authenticate
    address: String,
    /// Optional override for Unix timestamp (seconds as string). If omitted, uses current UTC time.
    timestamp: Option<String>,
    /// Optional nonce for ClobAuth (default: "0")
    nonce: Option<String>,
    /// Optional override for the ClobAuth attestation message.
    message: Option<String>,
}

impl DynAomiTool for GetPolymarketClobSignature {
    type App = PredictionApp;
    type Args = GetPolymarketClobSignatureArgs;
    const NAME: &'static str = "get_polymarket_clob_signature";
    const DESCRIPTION: &'static str = "Build the canonical Polymarket ClobAuth EIP-712 typed data for signing. Returns the typed_data JSON that must be signed by the wallet, along with the address/timestamp/nonce to reuse for ensure_polymarket_clob_credentials.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let address = if args.address.is_empty() {
            // Try to get address from context
            _ctx.attribute_string(&["domain", "evm", "address"])
                .ok_or_else(|| {
                    "No wallet address provided. Pass an address or connect a wallet via /connect."
                        .to_string()
                })?
        } else {
            args.address
        };

        let timestamp = args
            .timestamp
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(now_unix_timestamp);

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
            "next_step": "Sign the typed_data with the wallet, then call ensure_polymarket_clob_credentials with address/timestamp/nonce and the returned signature."
        }))
    }
}

// ============================================================================
// Tool 7: EnsurePolymarketClobCredentials
// ============================================================================

pub(crate) struct EnsurePolymarketClobCredentials;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct EnsurePolymarketClobCredentialsArgs {
    /// Wallet address used for CLOB L1 authentication
    address: String,
    /// L1 signature from wallet for CLOB auth challenge
    signature: String,
    /// Timestamp used in the L1 signature
    timestamp: String,
    /// Optional nonce for CLOB L1 auth (default: 0)
    nonce: Option<String>,
}

impl DynAomiTool for EnsurePolymarketClobCredentials {
    type App = PredictionApp;
    type Args = EnsurePolymarketClobCredentialsArgs;
    const NAME: &'static str = "ensure_polymarket_clob_credentials";
    const DESCRIPTION: &'static str = "Create or derive Polymarket CLOB API credentials (key, secret, passphrase) from L1 auth headers for a wallet.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
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
pub(crate) struct PlacePolymarketOrderArgs {
    /// Explicit user confirmation token; must be "confirm"
    confirmation: Option<String>,
    /// Wallet address (0x-prefixed) that signed the order
    owner: String,
    /// 0x signature returned from the wallet
    signature: String,
    /// JSON object or JSON string describing the order payload per Polymarket docs
    order: Value,
    /// Optional client order id for idempotency
    client_id: Option<String>,
    /// Optional override URL for the orders endpoint
    endpoint: Option<String>,
    /// Optional API key value inserted as X-API-KEY
    api_key: Option<String>,
    /// Optional CLOB auth bundle for L2 headers and automatic create/derive bootstrap via L1 auth
    clob_auth: Option<PlaceOrderClobAuthArgs>,
    /// Optional JSON object with additional top-level fields to merge into the request
    extra_fields: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceOrderClobAuthArgs {
    /// Existing CLOB API key
    api_key: Option<String>,
    /// Existing CLOB API secret
    api_secret: Option<String>,
    /// Existing CLOB passphrase
    passphrase: Option<String>,
    /// L1 auth payload used to create/derive API creds when credentials are missing
    l1_auth: Option<PlaceOrderL1AuthArgs>,
    /// Optional precomputed L2 signature
    l2_signature: Option<String>,
    /// Optional precomputed L2 timestamp
    l2_timestamp: Option<String>,
    /// Auto bootstrap credentials when missing (default: true)
    #[allow(dead_code)]
    auto_create_or_derive: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceOrderL1AuthArgs {
    /// Wallet address used for L1 auth
    address: String,
    /// L1 signature for CLOB auth
    signature: String,
    /// L1 timestamp used in signature
    timestamp: String,
    /// Optional L1 nonce (default: 0)
    nonce: Option<String>,
}

impl DynAomiTool for PlacePolymarketOrder {
    type App = PredictionApp;
    type Args = PlacePolymarketOrderArgs;
    const NAME: &'static str = "place_polymarket_order";
    const DESCRIPTION: &'static str = "Submit a signed Polymarket order to the CLOB API. Provide the wallet address that signed, the 0x signature string, and the order JSON.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        // Validate confirmation
        match args.confirmation.as_deref() {
            Some(c) if c.trim().eq_ignore_ascii_case("confirm") => {}
            _ => {
                return Err(
                    "Missing explicit confirmation. Require confirmation='confirm' before order submission."
                        .to_string(),
                )
            }
        }

        let order = match args.order {
            Value::String(raw) => serde_json::from_str::<Value>(&raw)
                .map_err(|e| format!("order must be valid JSON string: {e}"))?,
            other => other,
        };

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
                l2_signature: auth.l2_signature,
                l2_timestamp: auth.l2_timestamp,
            }
        });

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

// ============================================================================
// Tool 9: SimmerRegister
// ============================================================================

pub(crate) struct SimmerRegister;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SimmerRegisterArgs {
    /// Agent name (e.g., "aomi-trader")
    name: String,
    /// Brief description of what the agent does
    description: Option<String>,
}

impl DynAomiTool for SimmerRegister {
    type App = PredictionApp;
    type Args = SimmerRegisterArgs;
    const NAME: &'static str = "simmer_register";
    const DESCRIPTION: &'static str = "Register a new agent with Simmer. Returns API key and claim URL. The claim URL must be sent to the user to enable real USDC trading.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let result = simmer_register_agent(&args.name, args.description.as_deref())?;
        Ok(json!({
            "status": "registered",
            "agent_id": result.get("agent_id"),
            "api_key": result.get("api_key"),
            "claim_code": result.get("claim_code"),
            "claim_url": result.get("claim_url"),
            "starting_balance": result.get("starting_balance"),
            "limits": result.get("limits"),
            "next_steps": [
                "1. Save the api_key securely (use /apikey simmer <key>)",
                "2. Send claim_url to user",
                "3. Start trading with $SIM (virtual) immediately",
                "4. After user claims, real trading is enabled"
            ]
        }))
    }
}

// ============================================================================
// Tool 10: SimmerStatus
// ============================================================================

pub(crate) struct SimmerStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SimmerStatusArgs {
    /// Simmer API key (sk_...)
    api_key: String,
}

impl DynAomiTool for SimmerStatus {
    type App = PredictionApp;
    type Args = SimmerStatusArgs;
    const NAME: &'static str = "simmer_status";
    const DESCRIPTION: &'static str =
        "Get agent status: balance, claim status, whether real trading is enabled, and limits.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = SimmerClient::new(&args.api_key, "simmer")?;
        let status = client.get_agent_status()?;
        Ok(json!({
            "agent_id": status.get("agent_id"),
            "name": status.get("name"),
            "status": status.get("status"),
            "sim_balance": status.get("sim_balance"),
            "usdc_balance": status.get("balance_usdc"),
            "real_trading_enabled": status.get("real_trading_enabled"),
            "claim_url": status.get("claim_url"),
            "limits": status.get("limits"),
        }))
    }
}

// ============================================================================
// Tool 11: SimmerBriefing
// ============================================================================

pub(crate) struct SimmerBriefing;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SimmerBriefingArgs {
    /// Simmer API key (sk_...)
    api_key: String,
    /// ISO timestamp to get changes since (optional, defaults to 24h ago)
    since: Option<String>,
}

impl DynAomiTool for SimmerBriefing {
    type App = PredictionApp;
    type Args = SimmerBriefingArgs;
    const NAME: &'static str = "simmer_briefing";
    const DESCRIPTION: &'static str = "Get a full briefing from Simmer: portfolio, positions, opportunities, risk alerts, and performance. Use this for periodic check-ins instead of multiple API calls.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = SimmerClient::new(&args.api_key, "simmer")?;
        let briefing = client.get_briefing(args.since.as_deref())?;
        Ok(json!({
            "portfolio": briefing.get("portfolio"),
            "positions": briefing.get("positions"),
            "opportunities": briefing.get("opportunities"),
            "risk_alerts": briefing.get("risk_alerts"),
            "performance": briefing.get("performance"),
            "checked_at": briefing.get("checked_at"),
        }))
    }
}

// ============================================================================
// Tool 12: FetchSimmerMarketContext
// ============================================================================

pub(crate) struct FetchSimmerMarketContext;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FetchSimmerMarketContextArgs {
    /// Simmer API key (sk_...)
    api_key: String,
    /// Market ID to analyze before trading
    market_id: String,
}

impl DynAomiTool for FetchSimmerMarketContext {
    type App = PredictionApp;
    type Args = FetchSimmerMarketContextArgs;
    const NAME: &'static str = "fetch_simmer_market_context";
    const DESCRIPTION: &'static str = "Get detailed context for a specific market before trading. Returns position info, warnings, slippage estimate, fees, and resolution criteria.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = SimmerClient::new(&args.api_key, "simmer")?;
        let context = client.get_market_context(&args.market_id)?;
        Ok(json!({
            "market": context.get("market"),
            "position": context.get("position"),
            "warnings": context.get("warnings"),
            "slippage_estimate": context.get("slippage_estimate"),
            "time_to_resolution": context.get("time_to_resolution"),
            "resolution_criteria": context.get("resolution_criteria"),
            "fees": {
                "is_paid": context.get("is_paid"),
                "fee_rate_bps": context.get("fee_rate_bps"),
                "note": context.get("fee_note"),
            }
        }))
    }
}

// ============================================================================
// Tool 13: SimmerPlaceOrder
// ============================================================================

pub(crate) struct SimmerPlaceOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SimmerPlaceOrderArgs {
    /// Simmer API key (sk_...)
    api_key: String,
    /// Market ID to trade on
    market_id: String,
    /// Outcome to bet on: "yes" or "no"
    side: String,
    /// Amount in USD (or $SIM for simmer venue)
    amount: f64,
    /// Trading venue: simmer (sandbox $SIM), polymarket (real USDC), kalshi (real USDC)
    venue: Option<String>,
    /// Action: "buy" or "sell" (default: buy)
    action: Option<String>,
    /// Your thesis for this trade -- displayed publicly on Simmer, builds reputation
    reasoning: Option<String>,
}

impl DynAomiTool for SimmerPlaceOrder {
    type App = PredictionApp;
    type Args = SimmerPlaceOrderArgs;
    const NAME: &'static str = "simmer_place_order";
    const DESCRIPTION: &'static str = "Place an order via Simmer SDK. Executes trades on Polymarket, Kalshi, or Simmer sandbox. Requires Simmer API key via /apikey simmer <key>. Include reasoning to build public reputation.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let venue = args
            .venue
            .as_deref()
            .map(parse_venue)
            .transpose()?
            .unwrap_or_else(|| "simmer".to_string());

        let client = SimmerClient::new(&args.api_key, &venue)?;

        let mut body = json!({
            "market_id": args.market_id,
            "side": args.side.to_lowercase(),
            "amount": args.amount,
            "venue": venue,
            "action": args.action.as_deref().unwrap_or("buy").to_lowercase(),
            "source": "sdk:aomi",
        });
        if let Some(reasoning) = &args.reasoning {
            body["reasoning"] = Value::String(reasoning.clone());
        }

        match client.trade(body) {
            Ok(response) => Ok(json!({
                "status": "success",
                "trade_id": response.get("trade_id"),
                "market_id": response.get("market_id"),
                "side": response.get("side"),
                "shares": response.get("shares_bought").or_else(|| response.get("shares_sold")),
                "cost": response.get("cost"),
                "average_price": response.get("average_price"),
                "venue": response.get("venue"),
                "reasoning": args.reasoning,
            })),
            Err(e) => Ok(json!({
                "status": "error",
                "message": e,
                "order_details": {
                    "market_id": args.market_id,
                    "side": args.side,
                    "amount": args.amount,
                    "venue": venue,
                }
            })),
        }
    }
}

// ============================================================================
// Tool 14: SimmerGetPositions
// ============================================================================

pub(crate) struct SimmerGetPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SimmerGetPositionsArgs {
    /// Simmer API key (sk_...)
    api_key: String,
    /// Optional venue filter: simmer, polymarket, or kalshi
    venue: Option<String>,
}

impl DynAomiTool for SimmerGetPositions {
    type App = PredictionApp;
    type Args = SimmerGetPositionsArgs;
    const NAME: &'static str = "simmer_get_positions";
    const DESCRIPTION: &'static str = "Get all open positions from Simmer. Shows market, side, shares, cost basis, current value, and P&L.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let venue = args
            .venue
            .as_deref()
            .map(parse_venue)
            .transpose()?
            .unwrap_or_else(|| "simmer".to_string());

        let client = SimmerClient::new(&args.api_key, &venue)?;
        let result = client.get_positions()?;

        let positions = result
            .get("positions")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let total_pnl: f64 = positions
            .iter()
            .filter_map(|p| p.get("pnl").and_then(Value::as_f64))
            .sum();

        Ok(json!({
            "positions_count": positions.len(),
            "total_pnl": format!("${:.2}", total_pnl),
            "positions": positions,
        }))
    }
}

// ============================================================================
// Tool 15: SimmerGetPortfolio
// ============================================================================

pub(crate) struct SimmerGetPortfolio;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SimmerGetPortfolioArgs {
    /// Simmer API key (sk_...)
    api_key: String,
}

impl DynAomiTool for SimmerGetPortfolio {
    type App = PredictionApp;
    type Args = SimmerGetPortfolioArgs;
    const NAME: &'static str = "simmer_get_portfolio";
    const DESCRIPTION: &'static str = "Get portfolio summary from Simmer. Shows balance, positions value, total value, realized and unrealized P&L.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = SimmerClient::new(&args.api_key, "simmer")?;
        let portfolio = client.get_portfolio()?;
        Ok(json!({
            "balance": portfolio.get("balance"),
            "currency": portfolio.get("currency"),
            "positions_value": portfolio.get("positions_value"),
            "total_value": portfolio.get("total_value"),
            "realized_pnl": portfolio.get("realized_pnl"),
            "unrealized_pnl": portfolio.get("unrealized_pnl"),
        }))
    }
}

// ============================================================================
// Tool 16: SearchSimmerMarkets
// ============================================================================

pub(crate) struct SearchSimmerMarkets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchSimmerMarketsArgs {
    /// Simmer API key (sk_...)
    api_key: String,
    /// Filter by import source: polymarket, kalshi
    import_source: Option<String>,
    /// Filter by status: active, resolved
    status: Option<String>,
    /// Maximum number of markets to return (default: 20)
    limit: Option<u32>,
}

impl DynAomiTool for SearchSimmerMarkets {
    type App = PredictionApp;
    type Args = SearchSimmerMarketsArgs;
    const NAME: &'static str = "search_simmer_markets";
    const DESCRIPTION: &'static str =
        "Get available markets from Simmer. Can filter by source (Polymarket/Kalshi) and status.";

    fn run(_app: &PredictionApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = SimmerClient::new(&args.api_key, "simmer")?;
        let result = client.get_markets(
            args.import_source.as_deref(),
            args.status.as_deref(),
            args.limit,
        )?;

        let markets = result
            .get("markets")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        Ok(json!({
            "markets_count": markets.len(),
            "markets": markets,
        }))
    }
}
