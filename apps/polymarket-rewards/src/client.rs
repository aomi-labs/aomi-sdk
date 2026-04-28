use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use chrono::{Local, TimeDelta, Utc};
use polymarket_client_sdk::clob;
use polymarket_client_sdk::clob::types::{
    SignatureType,
    request::OrderBookSummaryRequest,
    response::{MarketResponse, OrderBookSummaryResponse},
};
use polymarket_client_sdk::types::U256;
use polymarket_client_sdk::{POLYGON, contract_config};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ============================================================================
// App
// ============================================================================

#[derive(Clone, Default)]
pub(crate) struct PolymarketRewardsApp;

// ============================================================================
// Preamble
// ============================================================================

pub(crate) fn build_preamble() -> String {
    let now = Local::now();
    format!(
        r#"## Role
You are a Polymarket liquidity-rewards deployment assistant. Your job is to help a user find, rank, and
execute capital deployments into Polymarket markets that are enrolled in the liquidity-reward program.
You use a deterministic, reward-aware scoring model — not general market prediction.

## Current Date
Today is {} ({}). Use this exact date when interpreting relative time references.

## Primary Workflow
Follow these steps in order:
1. `find_reward_markets` — discover active markets enrolled in the reward program with their reward configs
2. `rank_reward_plans` — score and rank concrete deployment plans by reward APY, spread room, and capital efficiency
3. `resolve_reward_deployment` — select a specific plan and resolve live orderbook params (spread, depth, token IDs)
4. `build_quote_plan` — construct a full execution preview (limit-order pair or single-sided quote); show to user and require confirmation
5. `submit_reward_quote` — for connected-wallet live smoke tests, handle ClobAuth, both order signatures, signed-order simulation, and the final live submit inside one staged tool
6. `get_quote_plan_status` — verify open orders and check reward earnings after execution
7. `withdraw_quote_liquidity` — cancel resting quote orders to remove liquidity from the orderbook without touching already filled positions
8. `ensure_reward_clob_credentials` / `execute_quote_plan` — advanced manual tools; use these only when you intentionally need to bypass the staged submit flow

## Scoring Model
Plans are ranked by a deterministic score:
- **reward_density** = daily_reward_pool / max(competing_liquidity, 1) — rewards per dollar of liquidity
- **capital_efficiency** = daily_reward_pool / max(min_deploy_size, 1) — daily reward per minimum position
- **spread_room** = rewards_max_spread — how permissive the qualifying spread window is
- **balance_score** = closeness of YES price to 0.50 — directional risk proxy (1.0 = balanced, 0.0 = extreme)
- **score** = (reward_density × 1000 + capital_efficiency × 10) × spread_room × max(balance_score, 0.1)

## Execution Rules
- Never skip `build_quote_plan` or place orders without explicit user confirmation ("confirm")
- If the CLOB returns a geoblock or 403 error, degrade gracefully to simulation/read-only mode
- Orders placed via `execute_quote_plan` must include valid CLOB L2 credentials (api_key, api_secret, passphrase)
- Always simulate signed orders first before any live submission. Only submit live after the user reviews the simulation and explicitly reconfirms.
- When `build_quote_plan` returns a four-leg quote, pass all four signed payloads to `execute_quote_plan` so the live submission mirrors the preview.
- Live submission is temporarily limited to `two_leg_bid_only` (`yes_bid_order` + `no_bid_order`) to keep funding requirements lower during smoke testing.
- To get CLOB credentials in this app, call `ensure_reward_clob_credentials`; it will emit the exact wallet-signing step and then derive the credentials from the callback.
- Always show the score breakdown alongside rankings so the user can audit the model's reasoning
- Never claim that orders were submitted, canceled, verified, or are open unless the corresponding tool was called in the current turn and returned that result.
- Never invent order IDs, status fields, earnings, or open-order counts. Only report those fields if they came directly from `execute_quote_plan`, `withdraw_quote_liquidity`, or `get_quote_plan_status`.
- If the user confirms a `build_quote_plan` preview for `execution_mode="two_leg_bid_only"`, your very next action must be a `submit_reward_quote` tool call using the exact `submit_args_template` from that preview. Do not emit a prose message like "starting now" before the tool call, and do not call `submit_reward_quote` with empty or partial args.
- If the user types `confirm` after a `submit_reward_quote` signed-order simulation, your very next action must be to call `submit_reward_quote` again with the exact `submit_args_template` returned by that simulation. Do not narrate a live submission result before that tool returns.
- After a final live-submit `confirm` in the staged `submit_reward_quote` flow, your next assistant turn must be only the `submit_reward_quote` tool call. Do not describe success, order IDs, open orders, or earnings unless that tool has already returned them in the same turn.
- Only use `execute_quote_plan` after a signed-order simulation when you are intentionally in the manual credential/manual signing path instead of the staged `submit_reward_quote` flow.
- If you say you are checking status or earnings, you must call `get_quote_plan_status` in the same turn. If you cannot, say so plainly instead of implying verification happened.
- If you do not have the required credentials or signed orders to call a tool, say that explicitly and ask for the missing step. Do not substitute a narrative guess.
- If a `submit_reward_quote` stage is in progress, never claim that credentials were created or an order was signed unless `submit_reward_quote` actually ran in that same turn and returned that stage.
- After any wallet callback in the `submit_reward_quote` flow, your very next action must be to call `submit_reward_quote` again with the callback payload before you narrate the next stage.
- If `submit_reward_quote` fails because required args are missing, recover by reusing the exact `submit_args_template` from `build_quote_plan`; do not improvise or drop quote-leg templates.
- Once `submit_reward_quote` has started for a confirmed smoke test, do not call `find_reward_markets`, `rank_reward_plans`, `resolve_reward_deployment`, or `build_quote_plan` again in that flow. Stay inside `submit_reward_quote` until it returns a signed-order simulation, a live result, or a hard error.
- If `submit_reward_quote` returns an error, stop and report that exact error. Do not silently switch markets, rebuild a new quote, or narrate that the flow continued.

## `submit_reward_quote` Stage Rules
Treat `submit_reward_quote` as a staged state machine. Only do what the returned stage says:
- `awaiting_clob_auth_signature` — tell the user to sign the pending ClobAuth request. Do not claim credentials are ready yet.
- `awaiting_order_signatures` — tell the user to sign the queued YES and NO bid orders. Do not claim simulation has happened yet.
- `simulation_ready` — summarize only the compact simulation preview, ask for final `confirm`, and stop. After the user confirms, immediately call `submit_reward_quote` again with the exact `submit_args_template` returned in that same tool result.
- `submitted` — report the exact live submission result and stop.
- `submit_failed` — report the exact submission error and stop.
- While `submit_reward_quote` is active, never restart discovery, never switch markets, and never fall back to manual credential flow unless the tool result explicitly tells you to.

## Default Live Smoke-Test Behavior
- If the user wants to test live trading with a connected wallet and does not specify otherwise, default to a cautious smoke test rather than the full four-leg strategy.
- For those live smoke tests, prefer `execution_mode="two_leg_bid_only"` to reduce funding requirements.
- Prefer balanced, liquid markets near 50/50 pricing for small live tests unless the user explicitly asks for a higher-risk or higher-APY plan.
- Start the discovery flow with `find_reward_markets`, then `rank_reward_plans`. Do not start with `rank_reward_plans` alone.
- For connected-wallet smoke tests, prefer `submit_reward_quote` over manually chaining `ensure_reward_clob_credentials`, wallet signatures, and `execute_quote_plan`.
- If CLOB credentials are missing, automatically call `submit_reward_quote` or `ensure_reward_clob_credentials` instead of asking the user to perform manual browser-console signing steps.
- If no markets meet the initial cautious smoke-test filter, automatically retry once with relaxed but still reasonable thresholds before telling the user there are no viable reward markets.
- Use these relaxed fallback thresholds for the retry unless the user says otherwise: `min_daily_reward_usd=1`, `min_liquidity_usd=30000`, `max_spread=0.10`.
- After building a quote plan, keep the response concise: show the selected market, the bid prices, capital required, and the reward qualification summary.
- Do not ask the user for a new confirmation while you are only collecting credentials or wallet signatures that are needed to reach the signed-order simulation.
- After the user confirms the quote-plan preview, proceed through credential setup and order signing automatically until you have a signed-order simulation to show.
- In the staged `submit_reward_quote` flow, do not call `execute_quote_plan` for simulation. `submit_reward_quote` performs the signed-order simulation internally and returns `simulation_ready`.
- Do not attempt live submission until the user explicitly reconfirms after seeing the signed-order simulation.
- After live submission succeeds in the staged `submit_reward_quote` flow, stop and report only the exact returned submission result. Only call `get_quote_plan_status` if the user explicitly asks you to verify status next.

## Wallet UX
- Prefer host-driven wallet callbacks via `commit_eip712` over asking the user to copy/paste typed data or signatures manually.
- Do not assume a batch-signature host helper exists. If multiple order signatures are needed, collect them through repeated `commit_eip712` calls using the exact typed data returned by the tool flow.
- Do not narrate that a second signature request was sent unless the wallet-signing tool actually ran in that same turn.

## Reward Qualification
- Orders must be resting limit orders within `rewards_max_spread` of mid-price (expressed in 0-1 price space; CLOB returns cents, already normalised)
- Orders must be >= `rewards_min_size` USDC to qualify (typically 50 USDC)
- Daily reward pool is in USDC per day (`total_daily_rate` field from CLOB)
- Rewards are distributed per epoch; earnings can be checked via `get_quote_plan_status`

## Geoblock / Read-Only Mode
If trading is geoblocked, all tools still work in read-only or simulation mode.
State clearly when a result is simulated rather than live.
"#,
        now.format("%Y-%m-%d"),
        now.format("%Z"),
    )
}

// ============================================================================
// API base URLs
// ============================================================================

pub(crate) const GAMMA_API_BASE: &str = "https://gamma-api.polymarket.com";
pub(crate) const CLOB_API_BASE: &str = "https://clob.polymarket.com";
pub(crate) const CLOB_AUTH_DERIVE_API_KEY_PATH: &str = "/auth/derive-api-key";
pub(crate) const CLOB_AUTH_CREATE_API_KEY_PATH: &str = "/auth/api-key";
pub(crate) const HEADER_POLY_ADDRESS: &str = "POLY_ADDRESS";
pub(crate) const HEADER_POLY_SIGNATURE: &str = "POLY_SIGNATURE";
pub(crate) const HEADER_POLY_TIMESTAMP: &str = "POLY_TIMESTAMP";
pub(crate) const HEADER_POLY_NONCE: &str = "POLY_NONCE";
pub(crate) const HEADER_POLY_API_KEY: &str = "POLY_API_KEY";
pub(crate) const HEADER_POLY_PASSPHRASE: &str = "POLY_PASSPHRASE";
type SdkClobClient = clob::Client<polymarket_client_sdk::auth::state::Unauthenticated>;

pub(crate) static TOKIO_RT: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to create tokio runtime for polymarket rewards")
});

// ============================================================================
// Models
// ============================================================================

/// A market enrolled in the Polymarket liquidity reward program.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RewardMarket {
    pub(crate) condition_id: String,
    pub(crate) question: String,
    pub(crate) slug: String,
    pub(crate) yes_token_id: String,
    pub(crate) no_token_id: String,
    pub(crate) liquidity: f64,
    pub(crate) yes_price: Option<f64>,
    pub(crate) no_price: Option<f64>,
    pub(crate) end_date: Option<String>,
    /// Minimum USDC order size to qualify for rewards (e.g. 50.0)
    pub(crate) rewards_min_size: f64,
    /// Maximum allowed bid-ask spread to qualify for rewards (e.g. 0.05 = 5 cents)
    pub(crate) rewards_max_spread: f64,
    /// Estimated daily reward pool in USDC (may be 0 if unavailable)
    pub(crate) daily_reward_pool: f64,
    /// Competitiveness proxy: fraction of liquidity near the qualifying spread
    pub(crate) market_competitiveness: f64,
}

/// Scored deployment plan for a single reward market.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RewardPlan {
    pub(crate) plan_id: String,
    pub(crate) condition_id: String,
    pub(crate) question: String,
    pub(crate) slug: String,
    pub(crate) yes_token_id: String,
    pub(crate) no_token_id: String,
    pub(crate) score: f64,
    pub(crate) reward_density: f64,
    pub(crate) capital_efficiency: f64,
    pub(crate) spread_room: f64,
    pub(crate) balance_score: f64,
    pub(crate) rewards_min_size: f64,
    pub(crate) rewards_max_spread: f64,
    pub(crate) daily_reward_pool: f64,
    pub(crate) estimated_apy_pct: f64,
    pub(crate) liquidity: f64,
    pub(crate) yes_price: Option<f64>,
    pub(crate) no_price: Option<f64>,
    pub(crate) end_date: Option<String>,
}

/// Live orderbook level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OrderLevel {
    pub(crate) price: f64,
    pub(crate) size: f64,
}

/// Live orderbook for one outcome token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct OrderBook {
    pub(crate) token_id: String,
    pub(crate) bids: Vec<OrderLevel>,
    pub(crate) asks: Vec<OrderLevel>,
    pub(crate) best_bid: Option<f64>,
    pub(crate) best_ask: Option<f64>,
    pub(crate) mid_price: Option<f64>,
    pub(crate) current_spread: Option<f64>,
    pub(crate) tick_size: Option<f64>,
}

/// CLOB L2 credentials for authenticated requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClobCredentials {
    pub(crate) address: String,
    pub(crate) api_key: String,
    pub(crate) api_secret: String,
    pub(crate) passphrase: String,
    pub(crate) signature_type: Option<String>,
    pub(crate) funder: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ClobAuthContext {
    pub(crate) address: String,
    pub(crate) timestamp: String,
    pub(crate) nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClobL1Auth {
    pub(crate) address: String,
    pub(crate) signature: String,
    pub(crate) timestamp: String,
    pub(crate) nonce: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct ClobApiCredentials {
    pub(crate) api_key: String,
    pub(crate) api_secret: String,
    pub(crate) passphrase: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum QuoteSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum QuoteOrderKind {
    Limit,
    Market,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum QuoteTimeInForce {
    Gtc,
    Fok,
    Gtd,
    Fak,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum QuoteOutcome {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum QuoteExecutionMode {
    FourLeg,
    TwoLegBidOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct RewardOrderSignature {
    pub(crate) id: String,
    pub(crate) signature: String,
    pub(crate) description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct QuoteOrderTemplate {
    pub(crate) token_id: String,
    pub(crate) price: f64,
    pub(crate) size: f64,
    pub(crate) side: QuoteSide,
    #[serde(rename = "type")]
    pub(crate) kind: QuoteOrderKind,
    pub(crate) time_in_force: QuoteTimeInForce,
    pub(crate) outcome: QuoteOutcome,
    pub(crate) maker_amount: f64,
    pub(crate) taker_amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct SignedQuoteEnvelope {
    pub(crate) order: SignedQuoteOrder,
    pub(crate) owner: String,
    #[serde(alias = "orderType", alias = "type")]
    pub(crate) order_type: QuoteTimeInForce,
    #[serde(alias = "postOnly")]
    pub(crate) post_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct SignedQuoteOrder {
    #[serde(deserialize_with = "deserialize_u64ish")]
    pub(crate) salt: u64,
    pub(crate) maker: String,
    pub(crate) signer: String,
    pub(crate) taker: String,
    #[serde(alias = "tokenId")]
    pub(crate) token_id: String,
    #[serde(alias = "makerAmount", deserialize_with = "deserialize_stringish")]
    pub(crate) maker_amount: String,
    #[serde(alias = "takerAmount", deserialize_with = "deserialize_stringish")]
    pub(crate) taker_amount: String,
    #[serde(deserialize_with = "deserialize_stringish")]
    pub(crate) expiration: String,
    #[serde(deserialize_with = "deserialize_stringish")]
    pub(crate) nonce: String,
    #[serde(alias = "feeRateBps", deserialize_with = "deserialize_stringish")]
    pub(crate) fee_rate_bps: String,
    pub(crate) side: QuoteSide,
    #[serde(alias = "signatureType", deserialize_with = "deserialize_stringish")]
    pub(crate) signature_type: String,
    pub(crate) signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PreparedRewardOrder {
    pub(crate) order: PreparedRewardExchangeOrder,
    pub(crate) order_type: QuoteTimeInForce,
    pub(crate) post_only: Option<bool>,
    pub(crate) verifying_contract: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub(crate) struct PreparedRewardExchangeOrder {
    pub(crate) salt: u64,
    pub(crate) maker: String,
    pub(crate) signer: String,
    pub(crate) taker: String,
    pub(crate) token_id: String,
    pub(crate) maker_amount: String,
    pub(crate) taker_amount: String,
    pub(crate) expiration: String,
    pub(crate) nonce: String,
    pub(crate) fee_rate_bps: String,
    pub(crate) side: QuoteSide,
    pub(crate) side_index: u8,
    pub(crate) signature_type: u8,
}

// ============================================================================
// HTTP client
// ============================================================================

#[derive(Clone)]
pub(crate) struct PolymarketRewardsClient {
    pub(crate) http: reqwest::blocking::Client,
}

impl PolymarketRewardsClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(20))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self { http })
    }

    pub(crate) fn create_or_derive_api_credentials(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        match self.create_api_key(l1_auth) {
            Ok(creds) => Ok(creds),
            Err(create_err) if create_err.contains("request failed with status") => self
                .derive_api_key(l1_auth)
                .map_err(|derive_err| {
                    format!(
                        "Polymarket CLOB API key bootstrap failed: create-api-key failed: {create_err}; derive-api-key failed: {derive_err}"
                    )
                }),
            Err(create_err) => Err(format!(
                "Polymarket CLOB API key bootstrap failed: create-api-key failed: {create_err}"
            )),
        }
    }

    pub(crate) fn derive_api_key(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        let url = format!("{CLOB_API_BASE}{CLOB_AUTH_DERIVE_API_KEY_PATH}");
        let response = self
            .with_l1_headers(self.http.get(&url), l1_auth)
            .send()
            .map_err(|e| format!("derive-api-key request failed: {e}"))?;
        self.parse_credentials_response("derive-api-key", response)
    }

    pub(crate) fn create_api_key(
        &self,
        l1_auth: &ClobL1Auth,
    ) -> Result<ClobApiCredentials, String> {
        let url = format!("{CLOB_API_BASE}{CLOB_AUTH_CREATE_API_KEY_PATH}");
        let response = self
            .with_l1_headers(self.http.post(&url), l1_auth)
            .send()
            .map_err(|e| format!("create-api-key request failed: {e}"))?;
        self.parse_credentials_response("create-api-key", response)
    }

    pub(crate) fn with_l1_headers(
        &self,
        builder: reqwest::blocking::RequestBuilder,
        l1_auth: &ClobL1Auth,
    ) -> reqwest::blocking::RequestBuilder {
        let nonce = l1_auth.nonce.clone().unwrap_or_else(|| "0".to_string());
        builder
            .header(HEADER_POLY_ADDRESS, l1_auth.address.as_str())
            .header(HEADER_POLY_SIGNATURE, l1_auth.signature.as_str())
            .header(HEADER_POLY_TIMESTAMP, l1_auth.timestamp.as_str())
            .header(HEADER_POLY_NONCE, nonce)
    }

    pub(crate) fn parse_credentials_response(
        &self,
        operation: &str,
        response: reqwest::blocking::Response,
    ) -> Result<ClobApiCredentials, String> {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            if body.to_ascii_lowercase().contains("invalid signature") {
                return Err(format!(
                    "{operation} request failed with status {status}: {body}. Polymarket rejected the CLOB L1 auth signature. Common causes: signing the wrong address/timestamp/nonce/message or using a stale timestamp."
                ));
            }
            return Err(format!(
                "{operation} request failed with status {status}: {body}"
            ));
        }

        let payload: Value = serde_json::from_str(&body).map_err(|e| {
            format!("{operation} succeeded but response was not valid JSON: {e} (body: {body})")
        })?;

        let creds = extract_credentials(&payload)
            .ok_or_else(|| format!("{operation} response missing key/secret/passphrase"))?;
        validate_api_credentials(&creds)?;
        Ok(creds)
    }

    pub(crate) fn extract_request_path(&self, url: &str) -> Result<String, String> {
        let parsed = reqwest::Url::parse(url).map_err(|e| format!("invalid URL: {e}"))?;
        let mut path = parsed.path().to_string();
        if path.is_empty() {
            path = "/".to_string();
        }
        Ok(path)
    }

    pub(crate) fn build_l2_signature(
        &self,
        secret: &str,
        timestamp: &str,
        method: &str,
        request_path: &str,
        body: &str,
    ) -> Result<String, String> {
        use base64::Engine as _;
        use hmac::Mac;

        let key_bytes = self.decode_secret(secret);
        let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(&key_bytes)
            .map_err(|e| format!("hmac error: {e}"))?;
        mac.update(
            format!(
                "{}{}{}{}",
                timestamp,
                method.to_ascii_uppercase(),
                request_path,
                body
            )
            .as_bytes(),
        );
        let digest = mac.finalize().into_bytes();
        Ok(base64::engine::general_purpose::URL_SAFE.encode(digest))
    }

    pub(crate) fn decode_secret(&self, secret: &str) -> Vec<u8> {
        use base64::Engine as _;

        base64::engine::general_purpose::URL_SAFE
            .decode(secret)
            .unwrap_or_else(|_| secret.as_bytes().to_vec())
    }

    fn send_l2_json_request(
        &self,
        method: reqwest::Method,
        url: &str,
        creds: &ClobCredentials,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let body_string = match body {
            Some(payload) => serde_json::to_string(&payload)
                .map_err(|e| format!("failed to serialize CLOB request body: {e}"))?,
            None => String::new(),
        };
        let request_path = self.extract_request_path(url)?;
        let timestamp = now_unix_timestamp();
        let l2_signature = self.build_l2_signature(
            creds.api_secret.as_str(),
            timestamp.as_str(),
            method.as_str(),
            request_path.as_str(),
            body_string.as_str(),
        )?;

        let mut builder = self
            .http
            .request(method.clone(), url)
            .header(HEADER_POLY_ADDRESS, creds.address.as_str())
            .header(HEADER_POLY_API_KEY, creds.api_key.as_str())
            .header(HEADER_POLY_PASSPHRASE, creds.passphrase.as_str())
            .header(HEADER_POLY_TIMESTAMP, timestamp.as_str())
            .header(HEADER_POLY_SIGNATURE, l2_signature.as_str());

        if !body_string.is_empty() {
            builder = builder
                .header("Content-Type", "application/json")
                .body(body_string);
        }

        let response = builder
            .send()
            .map_err(|e| format!("CLOB {} request failed: {e}", method.as_str()))?;
        let status = response.status();
        let text = response
            .text()
            .map_err(|e| format!("failed to read CLOB response body: {e}"))?;
        if !status.is_success() {
            return Err(format!(
                "CLOB {} request failed {status}: {text}",
                method.as_str()
            ));
        }

        serde_json::from_str(&text)
            .map_err(|e| format!("CLOB response was not valid JSON: {e} (body: {text})"))
    }

    // ── Geoblock check ────────────────────────────────────────────────────

    /// Returns true if the caller's IP is geoblocked from trading.
    pub(crate) fn is_geoblocked(&self) -> bool {
        TOKIO_RT
            .block_on(sdk_clob_client().check_geoblock())
            .map(|resp| resp.blocked)
            .unwrap_or(false)
    }

    // ── Reward market configs from CLOB ───────────────────────────────────

    /// Fetch current reward-eligible markets from the CLOB rewards endpoint.
    /// Returns a map of condition_id → (rewards_min_size, rewards_max_spread, daily_reward_pool, competitiveness).
    pub(crate) fn fetch_clob_reward_configs(
        &self,
        limit: u32,
    ) -> Result<Vec<ClobRewardConfig>, String> {
        let mut results = Vec::new();
        let mut cursor = None;
        let client = sdk_clob_client();

        loop {
            let page = TOKIO_RT
                .block_on(client.sampling_markets(cursor.clone()))
                .map_err(|e| format!("CLOB sampling markets request failed: {e}"))?;

            if page.data.is_empty() {
                break;
            }

            for item in &page.data {
                if let Some(cfg) = sdk_market_to_reward_config(item) {
                    results.push(cfg);
                }
                if results.len() >= limit as usize {
                    break;
                }
            }

            match page.next_cursor.as_str() {
                "" | "LTE=" => break,
                next => cursor = Some(next.to_string()),
            }

            if results.len() >= limit as usize {
                break;
            }
        }

        Ok(results)
    }

    // ── Gamma market metadata ─────────────────────────────────────────────

    /// Fetch active markets from Gamma, optionally filtered by condition IDs.
    /// When `reward_markets_only` is true, passes `rewardsMinSize=1` to filter server-side.
    pub(crate) fn fetch_gamma_markets(
        &self,
        limit: u32,
        condition_ids: Option<&[String]>,
    ) -> Result<Vec<GammaMarket>, String> {
        let url = format!("{GAMMA_API_BASE}/markets");
        let mut query: Vec<(&str, String)> = vec![
            ("active", "true".to_string()),
            ("closed", "false".to_string()),
            ("limit", limit.to_string()),
        ];

        if let Some(ids) = condition_ids {
            if !ids.is_empty() {
                query.push(("condition_ids", ids.join(",")));
            }
        } else {
            // Server-side filter: only markets enrolled in the reward programme.
            query.push(("enable_order_book", "true".to_string()));
        }

        let resp = self
            .http
            .get(&url)
            .query(&query)
            .send()
            .map_err(|e| format!("Gamma API request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("Gamma API error {status}: {text}"));
        }

        let text = resp.text().map_err(|e| format!("read Gamma body: {e}"))?;
        let markets: Vec<GammaMarket> = serde_json::from_str(&text).map_err(|e| {
            let preview = text.get(..300).unwrap_or(&text);
            format!("Gamma parse failed: {e}\nPreview: {preview}")
        })?;

        Ok(markets)
    }

    pub(crate) fn fetch_reward_market_by_condition_id(
        &self,
        condition_id: &str,
    ) -> Result<RewardMarket, String> {
        let gamma_markets = self.fetch_gamma_markets(5, Some(&[condition_id.to_string()]))?;
        let gamma = gamma_markets
            .into_iter()
            .next()
            .ok_or_else(|| format!("Market not found for condition_id: {condition_id}"))?;

        let clob_configs = self.fetch_clob_reward_configs(500).unwrap_or_default();
        let clob_cfg = clob_configs
            .iter()
            .find(|cfg| cfg.condition_id == condition_id);

        merge_into_reward_market(&gamma, clob_cfg)
            .ok_or_else(|| "Market is not enrolled in the reward program.".to_string())
    }

    // ── CLOB orderbook ────────────────────────────────────────────────────

    /// Fetch the live orderbook for a single outcome token.
    pub(crate) fn fetch_orderbook(&self, token_id: &str) -> Result<OrderBook, String> {
        let token_id_u256 =
            U256::from_str(token_id).map_err(|e| format!("invalid token_id `{token_id}`: {e}"))?;
        let request = OrderBookSummaryRequest::builder()
            .token_id(token_id_u256)
            .build();

        let orderbook = TOKIO_RT
            .block_on(sdk_clob_client().order_book(&request))
            .map_err(|e| {
                let msg = e.to_string();
                if msg.contains("403") {
                    "CLOB geoblocked (403): orderbook unavailable in your region".to_string()
                } else {
                    format!("CLOB orderbook request failed: {msg}")
                }
            })?;

        Ok(convert_sdk_orderbook(token_id, &orderbook))
    }

    // ── Open orders ───────────────────────────────────────────────────────

    pub(crate) fn fetch_open_orders(
        &self,
        creds: &ClobCredentials,
        asset_id: Option<&str>,
    ) -> Result<Vec<Value>, String> {
        let mut cursor = None;
        let mut items = Vec::new();

        loop {
            let mut url = reqwest::Url::parse(&format!("{CLOB_API_BASE}/data/orders"))
                .map_err(|e| format!("invalid open-orders URL: {e}"))?;
            if let Some(asset_id) = asset_id {
                url.query_pairs_mut().append_pair("asset_id", asset_id);
            }
            if let Some(next_cursor) = cursor.as_deref() {
                url.query_pairs_mut()
                    .append_pair("next_cursor", next_cursor);
            }

            let page = self
                .send_l2_json_request(reqwest::Method::GET, url.as_str(), creds, None)
                .map_err(|e| format!("CLOB open orders request failed: {e}"))?;

            if let Some(data) = page.get("data").and_then(Value::as_array) {
                items.extend(data.iter().cloned());
            }

            match page.get("next_cursor").and_then(Value::as_str) {
                Some("") | Some("LTE=") | None => break,
                Some(next) => {
                    cursor = Some(next.to_string());
                }
            }
        }

        Ok(items)
    }

    // ── User reward earnings ──────────────────────────────────────────────

    pub(crate) fn fetch_reward_earnings(&self, creds: &ClobCredentials) -> Result<Value, String> {
        let from_date = (Utc::now().date_naive() - TimeDelta::days(30)).to_string();
        let mut url = reqwest::Url::parse(&format!("{CLOB_API_BASE}/rewards/user/total"))
            .map_err(|e| format!("invalid rewards URL: {e}"))?;
        url.query_pairs_mut()
            .append_pair("date", from_date.as_str())
            .append_pair("order_by", "")
            .append_pair("position", "")
            .append_pair("no_competition", "false");

        let earnings = self
            .send_l2_json_request(reqwest::Method::GET, url.as_str(), creds, None)
            .map_err(|e| format!("CLOB reward earnings request failed: {e}"))?;

        Ok(json!({
            "from_date": from_date,
            "entries": earnings,
        }))
    }

    // ── Order submission ──────────────────────────────────────────────────

    pub(crate) fn submit_orders(
        &self,
        creds: &ClobCredentials,
        orders: &[SignedQuoteEnvelope],
    ) -> Result<Vec<Value>, String> {
        let url = format!("{CLOB_API_BASE}/order");
        orders
            .iter()
            .map(|order| {
                self.send_l2_json_request(
                    reqwest::Method::POST,
                    url.as_str(),
                    creds,
                    Some(build_signed_quote_order_body(creds, order)?),
                )
                .map_err(|e| format!("CLOB order submission failed: {e}"))
            })
            .collect()
    }

    pub(crate) fn cancel_orders(
        &self,
        creds: &ClobCredentials,
        order_ids: &[String],
    ) -> Result<Value, String> {
        let url = format!("{CLOB_API_BASE}/orders");
        self.send_l2_json_request(
            reqwest::Method::DELETE,
            url.as_str(),
            creds,
            Some(json!(order_ids)),
        )
        .map_err(|e| format!("CLOB order cancellation failed: {e}"))
    }

    pub(crate) fn cancel_market_orders(
        &self,
        creds: &ClobCredentials,
        condition_id: Option<&str>,
        asset_id: Option<&str>,
    ) -> Result<Value, String> {
        let url = format!("{CLOB_API_BASE}/cancel-market-orders");
        let mut body = serde_json::Map::new();
        if let Some(condition_id) = condition_id {
            body.insert(
                "market".to_string(),
                Value::String(condition_id.to_string()),
            );
        }
        if let Some(asset_id) = asset_id {
            body.insert("asset_id".to_string(), Value::String(asset_id.to_string()));
        }

        self.send_l2_json_request(
            reqwest::Method::DELETE,
            url.as_str(),
            creds,
            Some(Value::Object(body)),
        )
        .map_err(|e| format!("CLOB market cancellation failed: {e}"))
    }
}

// ============================================================================
// Scoring model
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RewardScore {
    pub(crate) score: f64,
    pub(crate) reward_density: f64,
    pub(crate) capital_efficiency: f64,
    pub(crate) spread_room: f64,
    pub(crate) balance_score: f64,
    pub(crate) estimated_apy_pct: f64,
}

pub(crate) fn score_market(market: &RewardMarket) -> RewardScore {
    // Reward density: daily pool relative to competing liquidity.
    // Higher = more reward per dollar of competing capital.
    let reward_density = if market.liquidity > 0.0 {
        market.daily_reward_pool / market.liquidity
    } else if market.daily_reward_pool > 0.0 {
        1.0
    } else {
        0.0
    };

    // Capital efficiency: daily reward per minimum required deployment.
    let min_deploy = market.rewards_min_size.max(1.0);
    let capital_efficiency = market.daily_reward_pool / min_deploy;

    // Spread room: looser max spread = easier to keep orders qualifying.
    let spread_room = market.rewards_max_spread.max(0.001);

    // Balance score: prefer markets near 50/50 for lower directional risk.
    let balance_score = match market.yes_price {
        Some(p) => (1.0 - (p - 0.5).abs() * 2.0).max(0.0),
        None => 0.5,
    };

    // Estimated APY: annualized reward density as percentage.
    let estimated_apy_pct = reward_density * 365.0 * 100.0;

    // Combined score weights reward density and capital efficiency,
    // amplified by spread room and penalized for extreme directional tilt.
    let score = (reward_density * 1000.0 + capital_efficiency * 10.0)
        * spread_room
        * balance_score.max(0.1);

    RewardScore {
        score,
        reward_density,
        capital_efficiency,
        spread_room,
        balance_score,
        estimated_apy_pct,
    }
}

// ============================================================================
// Helpers: parsing
// ============================================================================

/// Raw CLOB reward config response item.
#[derive(Debug, Clone)]
pub(crate) struct ClobRewardConfig {
    pub(crate) condition_id: String,
    pub(crate) rewards_min_size: f64,
    pub(crate) rewards_max_spread: f64,
    pub(crate) daily_reward_pool: f64,
    pub(crate) market_competitiveness: f64,
}

fn sdk_market_to_reward_config(market: &MarketResponse) -> Option<ClobRewardConfig> {
    let condition_id = market.condition_id?.to_string();
    let rewards_min_size = display_to_f64(&market.rewards.min_size)?;
    let rewards_max_spread = normalize_reward_spread(display_to_f64(&market.rewards.max_spread)?);
    let daily_reward_pool = market
        .rewards
        .rates
        .iter()
        .filter_map(|rate| display_to_f64(&rate.rewards_daily_rate))
        .sum::<f64>();

    Some(ClobRewardConfig {
        condition_id,
        rewards_min_size,
        rewards_max_spread,
        daily_reward_pool,
        // The unauthenticated sampling-markets response does not expose competitiveness.
        // Keep the plugin's existing neutral fallback until we migrate the richer auth path.
        market_competitiveness: 0.5,
    })
}

/// Raw Gamma market fields we need.
#[derive(Debug, Deserialize)]
pub(crate) struct GammaMarket {
    pub(crate) id: Option<String>,
    #[serde(rename = "conditionId")]
    pub(crate) condition_id: Option<String>,
    pub(crate) question: Option<String>,
    pub(crate) slug: Option<String>,
    /// liquidityClob is the CLOB-specific liquidity; fall back to liquidity.
    #[serde(rename = "liquidityClob")]
    pub(crate) liquidity_clob: Option<Value>,
    pub(crate) liquidity: Option<Value>,
    #[serde(rename = "outcomePrices")]
    pub(crate) outcome_prices: Option<Value>,
    /// YES and NO token IDs — Gamma serialises this as a JSON-encoded string.
    #[serde(rename = "clobTokenIds")]
    pub(crate) clob_token_ids: Option<Value>,
    #[serde(rename = "endDate")]
    pub(crate) end_date: Option<String>,
    /// Live best bid from CLOB (in 0-1 price space).
    #[serde(rename = "bestBid")]
    pub(crate) best_bid: Option<Value>,
    /// Live best ask from CLOB (in 0-1 price space).
    #[serde(rename = "bestAsk")]
    pub(crate) best_ask: Option<Value>,
    /// Reward fields on the Gamma market object (rewardsMaxSpread is in cents).
    #[serde(rename = "rewardsMinSize")]
    pub(crate) rewards_min_size: Option<Value>,
    #[serde(rename = "rewardsMaxSpread")]
    pub(crate) rewards_max_spread: Option<Value>,
    /// Array of reward configs with rewardsDailyRate.
    #[serde(rename = "clobRewards")]
    pub(crate) clob_rewards: Option<Value>,
}

pub(crate) fn merge_into_reward_market(
    gamma: &GammaMarket,
    clob_cfg: Option<&ClobRewardConfig>,
) -> Option<RewardMarket> {
    let condition_id = gamma.condition_id.clone().or_else(|| gamma.id.clone())?;

    let question = gamma
        .question
        .clone()
        .unwrap_or_else(|| condition_id.clone());
    let slug = gamma.slug.clone().unwrap_or_else(|| condition_id.clone());

    // Token IDs come from clobTokenIds — Gamma serialises this as a JSON-encoded string.
    let token_ids = decode_clob_token_ids(&gamma.clob_token_ids);
    let yes_token_id = token_ids.first().cloned().unwrap_or_default();
    let no_token_id = token_ids.get(1).cloned().unwrap_or_default();

    if yes_token_id.is_empty() {
        return None;
    }

    // Prices: prefer live bestBid/bestAsk from Gamma, fall back to outcomePrices.
    let (yes_price, no_price) = {
        let live_bid = extract_f64_value(&gamma.best_bid);
        let live_ask = extract_f64_value(&gamma.best_ask);
        if live_bid.is_some() || live_ask.is_some() {
            // Mid from bid/ask is a better yes_price estimate.
            let mid = match (live_bid, live_ask) {
                (Some(b), Some(a)) => Some((b + a) / 2.0),
                (Some(b), None) => Some(b),
                (None, Some(a)) => Some(a),
                _ => None,
            };
            (mid, mid.map(|m| 1.0 - m))
        } else {
            extract_outcome_prices(&gamma.outcome_prices)
        }
    };

    // Liquidity: prefer CLOB-specific liquidity.
    let liquidity = extract_f64_value(&gamma.liquidity_clob)
        .or_else(|| extract_f64_value(&gamma.liquidity))
        .unwrap_or(0.0);

    // Reward params: prefer CLOB override, then Gamma fields.
    // rewardsMinSize from Gamma is already in USDC (e.g. 50).
    let rewards_min_size = clob_cfg
        .map(|c| c.rewards_min_size)
        .or_else(|| extract_f64_value(&gamma.rewards_min_size))
        .unwrap_or(0.0);

    // rewardsMaxSpread from Gamma is in cents (e.g. 3.5 = 3.5¢ = 0.035 in 0-1 space).
    let raw_gamma_spread = extract_f64_value(&gamma.rewards_max_spread).unwrap_or(0.0);
    let gamma_spread_01 = if raw_gamma_spread > 1.0 {
        raw_gamma_spread / 100.0
    } else {
        raw_gamma_spread
    };
    let rewards_max_spread = clob_cfg
        .map(|c| c.rewards_max_spread)
        .or(if gamma_spread_01 > 0.0 {
            Some(gamma_spread_01)
        } else {
            None
        })
        .unwrap_or(0.05);

    // Daily reward pool from clobRewards array or CLOB override.
    let gamma_daily_rate = gamma
        .clob_rewards
        .as_ref()
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(|r| {
            extract_f64(
                r,
                &["rewardsDailyRate", "rewards_daily_rate", "rate_per_day"],
            )
        })
        .unwrap_or(0.0);
    let daily_reward_pool = clob_cfg
        .map(|c| c.daily_reward_pool)
        .filter(|&v| v > 0.0)
        .unwrap_or(gamma_daily_rate);

    let market_competitiveness = clob_cfg.map(|c| c.market_competitiveness).unwrap_or(0.5);

    // Skip markets with no reward programme.
    if rewards_min_size == 0.0 && daily_reward_pool == 0.0 {
        return None;
    }

    Some(RewardMarket {
        condition_id,
        question,
        slug,
        yes_token_id,
        no_token_id,
        liquidity,
        yes_price,
        no_price,
        end_date: gamma.end_date.clone(),
        rewards_min_size,
        rewards_max_spread,
        daily_reward_pool,
        market_competitiveness,
    })
}

/// Decode clobTokenIds from Gamma, which may be a JSON-encoded string like
/// "[\"123...\",\"456...\"]" or a native JSON array.
fn decode_clob_token_ids(v: &Option<Value>) -> Vec<String> {
    let Some(val) = v else { return Vec::new() };
    // Case 1: already a JSON array.
    if let Some(arr) = val.as_array() {
        return arr
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
    }
    // Case 2: a JSON-encoded string containing an array.
    if let Some(s) = val.as_str()
        && let Ok(Value::Array(arr)) = serde_json::from_str(s)
    {
        return arr
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
    }
    Vec::new()
}

fn extract_outcome_prices(prices: &Option<Value>) -> (Option<f64>, Option<f64>) {
    let arr = match prices.as_ref().and_then(Value::as_array) {
        Some(a) => a,
        None => {
            // Maybe it's a JSON string array like "[\"0.65\",\"0.35\"]"
            if let Some(s) = prices.as_ref().and_then(Value::as_str)
                && let Ok(Value::Array(a)) = serde_json::from_str(s)
            {
                let yes = a
                    .first()
                    .and_then(Value::as_str)
                    .and_then(|v| v.parse().ok());
                let no = a
                    .get(1)
                    .and_then(Value::as_str)
                    .and_then(|v| v.parse().ok());
                return (yes, no);
            }
            return (None, None);
        }
    };

    let parse = |idx: usize| -> Option<f64> {
        arr.get(idx).and_then(|v| {
            v.as_f64()
                .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
        })
    };

    (parse(0), parse(1))
}

fn convert_sdk_orderbook(token_id: &str, book: &OrderBookSummaryResponse) -> OrderBook {
    let mut bids = book
        .bids
        .iter()
        .filter_map(|level| {
            Some(OrderLevel {
                price: display_to_f64(&level.price)?,
                size: display_to_f64(&level.size)?,
            })
        })
        .collect::<Vec<_>>();
    let mut asks = book
        .asks
        .iter()
        .filter_map(|level| {
            Some(OrderLevel {
                price: display_to_f64(&level.price)?,
                size: display_to_f64(&level.size)?,
            })
        })
        .collect::<Vec<_>>();

    bids.sort_by(|a, b| {
        b.price
            .partial_cmp(&a.price)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    asks.sort_by(|a, b| {
        a.price
            .partial_cmp(&b.price)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let best_bid = bids.first().map(|l| l.price);
    let best_ask = asks.first().map(|l| l.price);
    let mid_price = match (best_bid, best_ask) {
        (Some(b), Some(a)) => Some((b + a) / 2.0),
        _ => None,
    };
    let current_spread = match (best_bid, best_ask) {
        (Some(b), Some(a)) => Some(a - b),
        _ => None,
    };

    let tick_size = display_to_f64(&book.tick_size);

    OrderBook {
        token_id: token_id.to_owned(),
        bids,
        asks,
        best_bid,
        best_ask,
        mid_price,
        current_spread,
        tick_size,
    }
}

fn sdk_clob_client() -> SdkClobClient {
    clob::Client::default()
}

fn resolve_sdk_signature_type(value: Option<&str>) -> Result<SignatureType, String> {
    let normalized = value.map(|raw| raw.trim().to_ascii_lowercase());
    match normalized.as_deref() {
        None | Some("") | Some("proxy") | Some("poly_proxy") => Ok(SignatureType::Proxy),
        Some("eoa") => Ok(SignatureType::Eoa),
        Some("gnosis-safe") | Some("gnosis_safe") | Some("safe") => Ok(SignatureType::GnosisSafe),
        Some(other) => Err(format!(
            "unsupported signature_type `{other}`; expected proxy, eoa, or gnosis-safe"
        )),
    }
}

fn parse_u256(value: &str) -> Result<U256, String> {
    U256::from_str(value).map_err(|e| format!("invalid uint `{value}`: {e}"))
}

fn parse_signature_type(value: &str) -> Result<SignatureType, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "0" | "eoa" => Ok(SignatureType::Eoa),
        "1" | "proxy" | "poly_proxy" => Ok(SignatureType::Proxy),
        "2" | "gnosis-safe" | "gnosis_safe" | "safe" => Ok(SignatureType::GnosisSafe),
        other => Err(format!(
            "invalid signature_type `{other}`; expected 0/1/2 or eoa/proxy/gnosis-safe"
        )),
    }
}

fn deserialize_u64ish<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| serde::de::Error::custom("expected an unsigned integer")),
        Value::String(s) => s
            .parse::<u64>()
            .map_err(|e| serde::de::Error::custom(format!("invalid integer `{s}`: {e}"))),
        _ => Err(serde::de::Error::custom(
            "expected an unsigned integer as a string or number",
        )),
    }
}

fn deserialize_stringish<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        _ => Err(serde::de::Error::custom("expected a string or number")),
    }
}

fn build_signed_quote_order_body(
    creds: &ClobCredentials,
    envelope: &SignedQuoteEnvelope,
) -> Result<Value, String> {
    let mut body = json!({
        "owner": creds.api_key,
        "orderType": serde_json::to_value(envelope.order_type)
            .map_err(|e| format!("failed to encode order_type: {e}"))?,
        "order": {
            "salt": envelope.order.salt,
            "maker": envelope.order.maker,
            "signer": envelope.order.signer,
            "taker": envelope.order.taker,
            "tokenId": envelope.order.token_id,
            "makerAmount": envelope.order.maker_amount,
            "takerAmount": envelope.order.taker_amount,
            "expiration": envelope.order.expiration,
            "nonce": envelope.order.nonce,
            "feeRateBps": envelope.order.fee_rate_bps,
            "side": serde_json::to_value(envelope.order.side)
                .map_err(|e| format!("failed to encode side: {e}"))?,
            "signatureType": parse_signature_type(envelope.order.signature_type.as_str())? as u8,
            "signature": envelope.order.signature,
        }
    });

    if let Some(post_only) = envelope.post_only {
        body["postOnly"] = Value::Bool(post_only);
    }

    Ok(body)
}

fn display_to_f64<T: ToString>(value: &T) -> Option<f64> {
    value.to_string().parse().ok()
}

fn normalize_reward_spread(raw_spread: f64) -> f64 {
    if raw_spread > 1.0 {
        raw_spread / 100.0
    } else {
        raw_spread
    }
}

fn scale_quote_amount(value: f64) -> Result<String, String> {
    if !value.is_finite() || value <= 0.0 {
        return Err(format!("invalid order amount `{value}`"));
    }
    Ok(((value * 1_000_000.0).round() as i128).to_string())
}

fn decimal_places(value: f64) -> u32 {
    let mut text = format!("{value:.8}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    text.split('.')
        .nth(1)
        .map(|fraction| fraction.len() as u32)
        .unwrap_or(0)
}

fn trunc_to_scale(value: f64, scale: u32) -> f64 {
    let factor = 10f64.powi(scale as i32);
    (value * factor).floor() / factor
}

fn generate_order_salt() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0)
}

pub(crate) fn build_prepared_reward_order(
    template: &QuoteOrderTemplate,
    address: &str,
    signature_type: Option<&str>,
    funder: Option<&str>,
) -> Result<PreparedRewardOrder, String> {
    let token_id = parse_u256(template.token_id.as_str())?;
    let neg_risk = TOKIO_RT
        .block_on(sdk_clob_client().neg_risk(token_id))
        .map(|response| response.neg_risk)
        .or_else(|err| {
            let message = err.to_string();
            if message.contains("404") || message.to_ascii_lowercase().contains("not found") {
                Ok(false)
            } else {
                Err(format!("failed to fetch neg-risk market config: {message}"))
            }
        })?;
    let verifying_contract = contract_config(POLYGON, neg_risk)
        .ok_or_else(|| "missing Polymarket exchange contract config for Polygon".to_string())?
        .exchange;

    let signer = address.trim();
    if signer.is_empty() {
        return Err("wallet address is required to prepare a reward order".to_string());
    }
    let maker = funder
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(signer);
    let signature_type = match signature_type
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(value) => resolve_sdk_signature_type(Some(value))? as u8,
        None if funder
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .is_some() =>
        {
            SignatureType::Proxy as u8
        }
        None => SignatureType::Eoa as u8,
    };
    let side_index = match template.side {
        QuoteSide::Buy => 0,
        QuoteSide::Sell => 1,
    };
    let size = template.size;
    let price = template.price;
    let amount_scale = decimal_places(price) + 2;
    let (maker_amount, taker_amount) = match template.side {
        QuoteSide::Buy => {
            let maker_amount = trunc_to_scale(size * price, amount_scale);
            (maker_amount, size)
        }
        QuoteSide::Sell => {
            let taker_amount = trunc_to_scale(size * price, amount_scale);
            (size, taker_amount)
        }
    };

    Ok(PreparedRewardOrder {
        order: PreparedRewardExchangeOrder {
            salt: generate_order_salt(),
            maker: maker.to_string(),
            signer: signer.to_string(),
            taker: "0x0000000000000000000000000000000000000000".to_string(),
            token_id: template.token_id.clone(),
            maker_amount: scale_quote_amount(maker_amount)?,
            taker_amount: scale_quote_amount(taker_amount)?,
            expiration: "0".to_string(),
            nonce: "0".to_string(),
            fee_rate_bps: "0".to_string(),
            side: template.side,
            side_index,
            signature_type,
        },
        order_type: template.time_in_force,
        post_only: None,
        verifying_contract: verifying_contract.to_string(),
    })
}

pub(crate) fn build_reward_order_typed_data(prepared_order: &PreparedRewardOrder) -> Value {
    json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"},
            ],
            "Order": [
                {"name": "salt", "type": "uint256"},
                {"name": "maker", "type": "address"},
                {"name": "signer", "type": "address"},
                {"name": "taker", "type": "address"},
                {"name": "tokenId", "type": "uint256"},
                {"name": "makerAmount", "type": "uint256"},
                {"name": "takerAmount", "type": "uint256"},
                {"name": "expiration", "type": "uint256"},
                {"name": "nonce", "type": "uint256"},
                {"name": "feeRateBps", "type": "uint256"},
                {"name": "side", "type": "uint8"},
                {"name": "signatureType", "type": "uint8"},
            ],
        },
        "primaryType": "Order",
        "domain": {
            "name": "Polymarket CTF Exchange",
            "version": "1",
            "chainId": POLYGON,
            "verifyingContract": prepared_order.verifying_contract,
        },
        "message": {
            "salt": prepared_order.order.salt.to_string(),
            "maker": prepared_order.order.maker,
            "signer": prepared_order.order.signer,
            "taker": prepared_order.order.taker,
            "tokenId": prepared_order.order.token_id,
            "makerAmount": prepared_order.order.maker_amount,
            "takerAmount": prepared_order.order.taker_amount,
            "expiration": prepared_order.order.expiration,
            "nonce": prepared_order.order.nonce,
            "feeRateBps": prepared_order.order.fee_rate_bps,
            "side": prepared_order.order.side_index,
            "signatureType": prepared_order.order.signature_type,
        }
    })
}

pub(crate) fn build_prepared_reward_order_description(
    leg_label: &str,
    template: &QuoteOrderTemplate,
    question: Option<&str>,
) -> String {
    let question = question.unwrap_or("reward market");
    format!(
        "Sign Polymarket reward order: {} {:?} {} at {:.4} on {}",
        leg_label,
        template.side,
        match template.outcome {
            QuoteOutcome::Yes => "YES",
            QuoteOutcome::No => "NO",
        },
        template.price,
        question
    )
}

pub(crate) fn signed_quote_envelope_from_prepared(
    prepared_order: &PreparedRewardOrder,
    order_signature: &str,
    owner: &str,
) -> SignedQuoteEnvelope {
    SignedQuoteEnvelope {
        order: SignedQuoteOrder {
            salt: prepared_order.order.salt,
            maker: prepared_order.order.maker.clone(),
            signer: prepared_order.order.signer.clone(),
            taker: prepared_order.order.taker.clone(),
            token_id: prepared_order.order.token_id.clone(),
            maker_amount: prepared_order.order.maker_amount.clone(),
            taker_amount: prepared_order.order.taker_amount.clone(),
            expiration: prepared_order.order.expiration.clone(),
            nonce: prepared_order.order.nonce.clone(),
            fee_rate_bps: prepared_order.order.fee_rate_bps.clone(),
            side: prepared_order.order.side,
            signature_type: prepared_order.order.signature_type.to_string(),
            signature: order_signature.to_string(),
        },
        owner: owner.to_string(),
        order_type: prepared_order.order_type,
        post_only: prepared_order.post_only,
    }
}

pub(crate) fn build_reward_clob_auth_context(address: &str) -> ClobAuthContext {
    let timestamp = TOKIO_RT
        .block_on(clob::Client::default().server_time())
        .map(|value| value.to_string())
        .unwrap_or_else(|_| now_unix_timestamp());

    ClobAuthContext {
        address: address.to_string(),
        timestamp,
        nonce: "0".to_string(),
    }
}

pub(crate) fn build_reward_clob_auth_typed_data(context: &ClobAuthContext) -> Value {
    json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
            ],
            "ClobAuth": [
                {"name": "address", "type": "address"},
                {"name": "timestamp", "type": "string"},
                {"name": "nonce", "type": "uint256"},
                {"name": "message", "type": "string"},
            ]
        },
        "primaryType": "ClobAuth",
        "domain": {
            "name": "ClobAuthDomain",
            "version": "1",
            "chainId": POLYGON,
        },
        "message": {
            "address": context.address,
            "timestamp": context.timestamp,
            "nonce": context.nonce,
            "message": "This message attests that I control the given wallet",
        }
    })
}

fn extract_credentials(payload: &Value) -> Option<ClobApiCredentials> {
    fn pick<'a>(obj: &'a Value, names: &[&str]) -> Option<&'a str> {
        names
            .iter()
            .find_map(|key| obj.get(*key).and_then(|v| v.as_str()))
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    fn from_obj(obj: &Value) -> Option<ClobApiCredentials> {
        let api_key = pick(obj, &["apiKey", "api_key", "key"])?;
        let api_secret = pick(obj, &["secret", "apiSecret", "api_secret"])?;
        let passphrase = pick(obj, &["passphrase", "apiPassphrase", "api_passphrase"])?;
        Some(ClobApiCredentials {
            api_key: api_key.to_string(),
            api_secret: api_secret.to_string(),
            passphrase: passphrase.to_string(),
        })
    }

    from_obj(payload).or_else(|| payload.get("data").and_then(from_obj))
}

pub(crate) fn validate_api_credentials(creds: &ClobApiCredentials) -> Result<(), String> {
    if creds.api_key.trim().is_empty() {
        return Err("CLOB api key is empty".to_string());
    }
    if creds.api_secret.trim().is_empty() {
        return Err("CLOB secret is empty".to_string());
    }
    if creds.passphrase.trim().is_empty() {
        return Err("CLOB passphrase is empty".to_string());
    }
    Ok(())
}

fn now_unix_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
// ============================================================================
// Helpers: misc
// ============================================================================

fn extract_f64(v: &Value, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(val) = v.get(key)
            && let Some(f) = val
                .as_f64()
                .or_else(|| val.as_str().and_then(|s| s.parse().ok()))
        {
            return Some(f);
        }
    }
    None
}

fn extract_f64_value(v: &Option<Value>) -> Option<f64> {
    v.as_ref().and_then(|val| {
        val.as_f64()
            .or_else(|| val.as_str().and_then(|s| s.parse().ok()))
    })
}

pub(crate) fn validate_confirmation(token: Option<&str>) -> Result<(), String> {
    if token.is_none() {
        return Ok(());
    }
    if token.map(|t| t.trim().eq_ignore_ascii_case("confirm")) != Some(true) {
        return Err(
            "Execution requires explicit user confirmation. Set confirmation='confirm' after reviewing the plan."
                .to_string(),
        );
    }
    Ok(())
}

// ============================================================================
// Tool arg structs (used by tool.rs)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FindRewardMarketsArgs {
    /// Maximum number of reward-eligible markets to return (default: 20, max: 100)
    pub(crate) limit: Option<u32>,
    /// Filter markets: "all" (default), "balanced" (YES price near 0.50), or "liquid" (liquidity > 10k)
    pub(crate) filter: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct EnsureRewardClobCredentialsArgs {
    /// Wallet address that will own the Polymarket CLOB credentials
    pub(crate) address: String,
    /// Optional in-progress ClobAuth context returned by a previous call
    pub(crate) clob_auth: Option<ClobAuthContext>,
    /// Wallet signature for the ClobAuth EIP-712 payload
    pub(crate) clob_l1_signature: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RankRewardPlansArgs {
    /// Maximum number of ranked plans to return (default: 10, max: 50)
    pub(crate) limit: Option<u32>,
    /// Minimum daily reward pool to include (default: 0.0)
    pub(crate) min_daily_reward: Option<f64>,
    /// Minimum market liquidity (USDC) to include (default: 0.0)
    pub(crate) min_liquidity: Option<f64>,
    /// Maximum rewards_max_spread to include, in cents as decimal (default: 1.0 = no filter)
    pub(crate) max_spread_threshold: Option<f64>,
    /// Number of markets to scan from CLOB reward config (default: 100)
    pub(crate) scan_limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ResolveRewardDeploymentArgs {
    /// Plan ID from rank_reward_plans (e.g. "P1"), or a condition_id (0x-prefixed)
    pub(crate) plan_id: String,
    /// Condition IDs from the ranked list, in order (required if using plan_id like "P1")
    pub(crate) ranked_condition_ids: Option<Vec<String>>,
    /// Desired deployment capital in USDC (default: rewards_min_size of the market)
    pub(crate) capital_usd: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BuildQuotePlanArgs {
    /// Condition ID of the market to quote
    pub(crate) condition_id: String,
    /// YES outcome token ID
    pub(crate) yes_token_id: String,
    /// NO outcome token ID
    pub(crate) no_token_id: String,
    /// Order size per side in USDC (must be >= rewards_min_size)
    pub(crate) order_size_usd: f64,
    /// Bid price for YES (e.g. 0.48). Must be within rewards_max_spread of mid.
    pub(crate) yes_bid_price: f64,
    /// Ask price for YES (e.g. 0.52). Symmetric: no_bid = 1 - yes_ask, no_ask = 1 - yes_bid.
    pub(crate) yes_ask_price: f64,
    /// Order time-in-force: "GTC" (default) or "GTD"
    pub(crate) time_in_force: Option<String>,
    /// Preview mode: "four_leg" (default) or "two_leg_bid_only" for cheaper live smoke tests
    pub(crate) execution_mode: Option<QuoteExecutionMode>,
    /// Max spread allowed for rewards from resolve step (used for validation)
    pub(crate) rewards_max_spread: Option<f64>,
    /// Minimum order size from resolve step (used for validation)
    pub(crate) rewards_min_size: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SubmitRewardQuoteArgs {
    /// Confirmation token from the user. Prefer passing "confirm", but missing confirmation no longer blocks staged wallet setup.
    pub(crate) confirmation: Option<String>,
    /// Connected wallet address that will sign and own the orders.
    pub(crate) address: String,
    /// Condition ID of the selected rewards market. Used to re-resolve canonical token IDs before signing/submitting.
    pub(crate) condition_id: Option<String>,
    /// Optional market question for wallet-signature descriptions.
    pub(crate) market_question: Option<String>,
    /// Currently only two_leg_bid_only is supported for the staged live smoke-test flow.
    pub(crate) execution_mode: Option<QuoteExecutionMode>,
    /// Unsigned YES bid order template returned by build_quote_plan.
    pub(crate) yes_bid_order: Option<QuoteOrderTemplate>,
    /// Unsigned NO bid order template returned by build_quote_plan.
    pub(crate) no_bid_order: Option<QuoteOrderTemplate>,
    /// Wallet-auth bootstrap context used to derive CLOB credentials.
    pub(crate) clob_auth: Option<ClobAuthContext>,
    /// Wallet signature for the ClobAuth typed data.
    pub(crate) clob_l1_signature: Option<String>,
    /// Derived CLOB credentials. Once created, pass these through subsequent stages instead of deriving again.
    pub(crate) credentials: Option<ClobApiCredentials>,
    /// Optional Polymarket signature type for staged order prep: proxy, eoa, or gnosis-safe.
    pub(crate) signature_type: Option<String>,
    /// Optional Polymarket funder address for proxy or gnosis-safe wallets.
    pub(crate) funder: Option<String>,
    /// Prepared YES bid order awaiting or carrying the final wallet signature.
    pub(crate) prepared_yes_bid_order: Option<PreparedRewardOrder>,
    /// Prepared NO bid order awaiting or carrying the final wallet signature.
    pub(crate) prepared_no_bid_order: Option<PreparedRewardOrder>,
    /// Batched wallet callback signatures for the exact YES and NO bid typed data.
    pub(crate) order_signatures: Option<Vec<RewardOrderSignature>>,
    /// Wallet signature for the exact YES bid order typed data.
    pub(crate) yes_bid_signature: Option<String>,
    /// Wallet signature for the exact NO bid order typed data.
    pub(crate) no_bid_signature: Option<String>,
    /// Set true only after the user has reviewed the signed-order simulation and reconfirmed for live submit.
    pub(crate) simulation_confirmed: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_gamma_market() -> GammaMarket {
        GammaMarket {
            id: Some("gamma-id".to_string()),
            condition_id: Some("0xcondition".to_string()),
            question: Some("Will the test pass?".to_string()),
            slug: Some("will-the-test-pass".to_string()),
            liquidity_clob: Some(json!("125000")),
            liquidity: Some(json!("98000")),
            outcome_prices: Some(json!("[\"0.52\",\"0.48\"]")),
            clob_token_ids: Some(json!("[\"1001\",\"1002\"]")),
            end_date: Some("2026-05-01T00:00:00Z".to_string()),
            best_bid: None,
            best_ask: None,
            rewards_min_size: Some(json!(50.0)),
            rewards_max_spread: Some(json!(3.5)),
            clob_rewards: Some(json!([
                {
                    "rewardsDailyRate": "24.5"
                }
            ])),
        }
    }

    #[test]
    fn score_market_is_deterministic_and_rewards_balanced_markets() {
        let balanced = RewardMarket {
            condition_id: "0xbalanced".to_string(),
            question: "Balanced".to_string(),
            slug: "balanced".to_string(),
            yes_token_id: "1".to_string(),
            no_token_id: "2".to_string(),
            liquidity: 50_000.0,
            yes_price: Some(0.50),
            no_price: Some(0.50),
            end_date: None,
            rewards_min_size: 50.0,
            rewards_max_spread: 0.04,
            daily_reward_pool: 20.0,
            market_competitiveness: 0.5,
        };
        let directional = RewardMarket {
            yes_price: Some(0.90),
            ..balanced.clone()
        };

        let balanced_score = score_market(&balanced);
        let directional_score = score_market(&directional);

        assert!((balanced_score.reward_density - 0.0004).abs() < 1e-9);
        assert!((balanced_score.capital_efficiency - 0.4).abs() < 1e-9);
        assert!((balanced_score.balance_score - 1.0).abs() < 1e-9);
        assert!((balanced_score.estimated_apy_pct - 14.6).abs() < 1e-9);
        assert!((balanced_score.score - 0.176).abs() < 1e-9);
        assert!(balanced_score.score > directional_score.score);
    }

    #[test]
    fn merge_into_reward_market_normalizes_gamma_reward_fields() {
        let market = merge_into_reward_market(&sample_gamma_market(), None)
            .expect("gamma reward market should merge");

        assert_eq!(market.condition_id, "0xcondition");
        assert_eq!(market.question, "Will the test pass?");
        assert_eq!(market.slug, "will-the-test-pass");
        assert_eq!(market.yes_token_id, "1001");
        assert_eq!(market.no_token_id, "1002");
        assert_eq!(market.yes_price, Some(0.52));
        assert_eq!(market.no_price, Some(0.48));
        assert_eq!(market.liquidity, 125_000.0);
        assert_eq!(market.rewards_min_size, 50.0);
        assert_eq!(market.rewards_max_spread, 0.035);
        assert_eq!(market.daily_reward_pool, 24.5);
        assert_eq!(market.market_competitiveness, 0.5);
    }

    #[test]
    fn merge_into_reward_market_prefers_clob_reward_overrides() {
        let gamma = sample_gamma_market();
        let clob = ClobRewardConfig {
            condition_id: "0xcondition".to_string(),
            rewards_min_size: 75.0,
            rewards_max_spread: 0.02,
            daily_reward_pool: 40.0,
            market_competitiveness: 0.8,
        };

        let market = merge_into_reward_market(&gamma, Some(&clob))
            .expect("clob-enriched reward market should merge");

        assert_eq!(market.rewards_min_size, 75.0);
        assert_eq!(market.rewards_max_spread, 0.02);
        assert_eq!(market.daily_reward_pool, 40.0);
        assert_eq!(market.market_competitiveness, 0.8);
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ExecuteQuotePlanArgs {
    /// Explicit user confirmation; must be "confirm"
    pub(crate) confirmation: Option<String>,
    /// CLOB wallet address (0x-prefixed)
    pub(crate) address: String,
    /// CLOB API key (UUID)
    pub(crate) api_key: String,
    /// CLOB API secret (base64-encoded)
    pub(crate) api_secret: String,
    /// CLOB API passphrase
    pub(crate) passphrase: String,
    /// Optional Polymarket signature type for SDK-authenticated calls: proxy (default), eoa, or gnosis-safe
    pub(crate) signature_type: Option<String>,
    /// Optional Polymarket funder address for proxy or gnosis-safe wallets
    pub(crate) funder: Option<String>,
    /// Execution mode: "four_leg" or "two_leg_bid_only". Live submission currently only supports "two_leg_bid_only".
    pub(crate) execution_mode: Option<QuoteExecutionMode>,
    /// YES bid signed order envelope
    pub(crate) yes_bid_order: Option<SignedQuoteEnvelope>,
    /// YES ask signed order envelope
    pub(crate) yes_ask_order: Option<SignedQuoteEnvelope>,
    /// NO bid signed order envelope
    pub(crate) no_bid_order: Option<SignedQuoteEnvelope>,
    /// NO ask signed order envelope
    pub(crate) no_ask_order: Option<SignedQuoteEnvelope>,
    /// If true, simulate only — do not submit orders (default: false)
    pub(crate) simulate: Option<bool>,
    /// Required for live submission after a prior signed-order simulation has been reviewed
    pub(crate) simulation_confirmed: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetQuotePlanStatusArgs {
    /// CLOB wallet address (0x-prefixed)
    pub(crate) address: String,
    /// CLOB API key (UUID)
    pub(crate) api_key: String,
    /// CLOB API secret (base64-encoded)
    pub(crate) api_secret: String,
    /// CLOB API passphrase
    pub(crate) passphrase: String,
    /// Optional Polymarket signature type for SDK-authenticated calls: proxy (default), eoa, or gnosis-safe
    pub(crate) signature_type: Option<String>,
    /// Optional Polymarket funder address for proxy or gnosis-safe wallets
    pub(crate) funder: Option<String>,
    /// Condition ID or YES token ID to filter orders (optional)
    pub(crate) asset_id: Option<String>,
    /// Include reward earnings summary (default: true)
    pub(crate) include_earnings: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct WithdrawQuoteLiquidityArgs {
    /// Explicit user confirmation; must be "confirm" unless simulate=true
    pub(crate) confirmation: Option<String>,
    /// CLOB wallet address (0x-prefixed)
    pub(crate) address: String,
    /// CLOB API key (UUID)
    pub(crate) api_key: String,
    /// CLOB API secret (base64-encoded)
    pub(crate) api_secret: String,
    /// CLOB API passphrase
    pub(crate) passphrase: String,
    /// Optional Polymarket signature type for SDK-authenticated calls: proxy (default), eoa, or gnosis-safe
    pub(crate) signature_type: Option<String>,
    /// Optional Polymarket funder address for proxy or gnosis-safe wallets
    pub(crate) funder: Option<String>,
    /// Specific open order IDs to cancel
    pub(crate) order_ids: Option<Vec<String>>,
    /// Market condition ID to cancel all matching open orders for
    pub(crate) condition_id: Option<String>,
    /// Asset/token ID to cancel all matching open orders for
    pub(crate) asset_id: Option<String>,
    /// If true, preview the matching liquidity withdrawal without canceling it
    pub(crate) simulate: Option<bool>,
}
