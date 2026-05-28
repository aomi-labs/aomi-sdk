//! Curated tool layer for Krexa. Hand-written from the progenitor-generated
//! client in `crate::client` (see `apps/krexa/openapi.yaml`).
//!
//! User story: an autonomous trading agent ("Krex") that *borrows* USDC from
//! the Krexa credit vault to execute trades on other Aomi apps
//! (Hyperliquid, 1inch, …) and *pays* for the data / LLM calls that drive
//! those trades through Pay.sh. Onboarding is assumed to have happened
//! out-of-band; these tools support the steady-state loop only.
//!
//! 23 endpoints in the spec collapse into 10 agent-facing tools:
//!
//!   PLAN
//!     * `krexa_get_score`               — GET  /solana/score/{agent}
//!     * `krexa_check_credit_eligibility`— GET  /solana/credit/{agent}/eligibility
//!     * `krexa_discover_api_pricing`    — POST /solana/paysh/{agent}/discover
//!   ACT
//!     * `krexa_borrow_usdc`             — POST /solana/oracle/sign-credit
//!     * `krexa_pay_api_call`            — POST /solana/paysh/{agent}/call
//!     * `krexa_set_budget`              — POST /solana/paysh/{agent}/budget
//!   MONITOR
//!     * `krexa_get_balance`             — GET  /solana/paysh/{agent}/balance
//!     * `krexa_get_credit_line`         — GET  /solana/credit/{agent}/line
//!   SCOUT
//!     * `krexa_search_agents`           — GET  /kya/search?q=
//!     * `krexa_lookup_agent`            — GET  /kya/{input}/quick

use crate::auth;
use crate::client::Client as GenClient;
use crate::client::types::{
    DeployAgentRequest, InviteChallengeRequest, PayshBudgetUpdate, PayshCallRequest,
    PayshDiscoverRequest, PayshProxyQuoteRequest, RedeemInviteRequest, RequestCreditRequest,
    SignCreditRequest,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Default)]
pub(crate) struct KrexaApp;

const DEFAULT_BASE_URL: &str = "https://api.krexa.xyz/api/v1";
const BASE_URL_ENV: &str = "KREXA_API_URL";

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value).map_err(|e| format!("[krexa] serialize: {e}"))?;
    Ok(match value {
        Value::Object(mut m) => {
            m.insert("source".into(), Value::String("krexa".into()));
            Value::Object(m)
        }
        other => serde_json::json!({ "source": "krexa", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[krexa] runtime: {e}"))
}

fn base_url() -> String {
    std::env::var(BASE_URL_ENV).unwrap_or_else(|_| DEFAULT_BASE_URL.to_string())
}

fn client() -> GenClient {
    GenClient::new(&base_url())
}

// ============================================================================
// PLAN
// ============================================================================

pub(crate) struct GetScore;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetScoreArgs {
    /// Solana agent public key (base58). Returns the Krexit Score (200–850)
    /// plus the 5 weighted components and the registered/preview flag.
    pub agent: String,
}

impl DynAomiTool for GetScore {
    type App = KrexaApp;
    type Args = GetScoreArgs;
    const NAME: &'static str = "krexa_get_score";
    const DESCRIPTION: &'static str = "Read your own Krexit Score (200–850) and its 5 components (repayment, profit, behavior, usage, age). Use to gauge whether you've earned a higher credit level before requesting a larger draw. Unregistered wallets receive a preview score derived from wallet age, tx count, and SOL balance.";

    fn run(_app: &KrexaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client().get_score(args.agent.as_str()).await })
            .map_err(|e| format!("[krexa] get_score: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct CheckCreditEligibility;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CheckCreditEligibilityArgs {
    /// Solana agent public key (base58).
    pub agent: String,
}

impl DynAomiTool for CheckCreditEligibility {
    type App = KrexaApp;
    type Args = CheckCreditEligibilityArgs;
    const NAME: &'static str = "krexa_check_credit_eligibility";
    const DESCRIPTION: &'static str = "Check borrowing capacity right now. Returns `eligible`, `creditLevel` (1–4), `maxCredit`, `currentDebt`, `availableCredit`, and `rateBps`. Call this BEFORE borrow_usdc to size your draw — `availableCredit` is the hard ceiling for the next borrow.";

    fn run(_app: &KrexaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client().get_credit_eligibility(args.agent.as_str()).await })
            .map_err(|e| format!("[krexa] check_credit_eligibility: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct DiscoverApiPricing;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct DiscoverApiPricingArgs {
    /// Solana agent public key (base58).
    pub agent: String,
    /// The third-party API URL to probe for x402 pricing (e.g.
    /// `https://gemini.paysh.dev/v1/generate`).
    pub target_url: String,
    /// HTTP method the eventual call will use. Defaults to "GET".
    pub method: Option<String>,
}

impl DynAomiTool for DiscoverApiPricing {
    type App = KrexaApp;
    type Args = DiscoverApiPricingArgs;
    const NAME: &'static str = "krexa_discover_api_pricing";
    const DESCRIPTION: &'static str = "Probe a Pay.sh-compatible URL for its x402 price WITHOUT committing to a payment. Returns `pricing.amount` (USDC base units), `payTo`, `network`, plus an `affordability` block that says whether the wallet can afford it and whether a credit draw would be needed. Call before pay_api_call to budget the next loop.";

    fn run(_app: &KrexaApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = auth::api_key(&ctx)?;
        let body = PayshDiscoverRequest {
            method: args.method.unwrap_or_else(|| "GET".to_string()),
            target_url: args.target_url,
        };
        let runtime = rt()?;
        let result = runtime
            .block_on(async move {
                client()
                    .paysh_discover(args.agent.as_str(), api_key.as_str(), &body)
                    .await
            })
            .map_err(|e| format!("[krexa] discover_api_pricing: {e}"))?;
        ok(result.into_inner())
    }
}

// ============================================================================
// ACT
// ============================================================================

pub(crate) struct RequestCredit;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RequestCreditArgs {
    /// The agent that will hold the resulting credit line. Path param.
    pub agent: String,
    /// Owner wallet pubkey (base58). For self-custodied agents this is
    /// the same as `agent`. Embedded into the signed challenge.
    pub owner_pubkey: String,
    /// Requested USDC amount in 6-decimal base units, as a string of
    /// digits. `"500000000"` is $500. Must fit under the level's cap
    /// (L1=$500, L2=$20K, L3=$50K, L4=$500K).
    pub amount: String,
    /// Requested credit level (1–4). Capped by the agent's Krexit Score.
    pub credit_level: u8,
    /// Optional override for `ownerSignature` when the caller signs
    /// externally. If omitted, the tool signs internally — the **owner
    /// keypair** (KREXA_OWNER_SECRET_KEY) signs the raw 32 bytes of the
    /// **agent** pubkey, and the result is base64-encoded. Hidden from
    /// the LLM tool schema — the LLM never has a valid signature to
    /// pass; this slot is for operator/test injection only.
    #[schemars(skip)]
    pub owner_signature: Option<String>,
}

impl DynAomiTool for RequestCredit {
    type App = KrexaApp;
    type Args = RequestCreditArgs;
    const NAME: &'static str = "krexa_request_credit";
    const DESCRIPTION: &'static str = "Submit a credit request for oracle review. This is the first step of borrowing — the oracle must see an approved credit request before it will co-sign a draw. The owner keypair Ed25519-signs the raw 32 bytes of the agent's pubkey (NOT a challenge string), and the base64 signature goes into `ownerSignature`. The tool signs internally when `KREXA_OWNER_SECRET_KEY` is set. After this returns success, call `borrow_usdc` with the same amount + level.";

    fn run(_app: &KrexaApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = auth::api_key(&ctx)?;
        if !(1..=4).contains(&args.credit_level) {
            return Err("[krexa] request_credit: credit_level must be 1..=4".to_string());
        }
        let owner_signature = match args.owner_signature {
            Some(sig) => sig,
            None => auth::sign_credit_request(&ctx, &args.agent)?,
        };
        let body = RequestCreditRequest {
            amount: args.amount,
            credit_level: std::num::NonZeroU64::new(args.credit_level as u64)
                .expect("credit_level non-zero by range check"),
            owner_pubkey: args.owner_pubkey,
            owner_signature,
        };
        let runtime = rt()?;
        let result = runtime
            .block_on(async move {
                client()
                    .request_credit(args.agent.as_str(), api_key.as_str(), &body)
                    .await
            })
            .map_err(|e| format!("[krexa] request_credit: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct BorrowUsdc;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct BorrowUsdcArgs {
    /// The agent's Solana public key (base58). The credit line is keyed on
    /// this pubkey.
    pub agent_pubkey: String,
    /// The pubkey that will add the second signature and submit the tx —
    /// usually the agent itself, but can be the owner wallet for AA flows.
    pub agent_or_owner_pubkey: String,
    /// Amount to borrow, in USDC base units (6 decimals), encoded as a
    /// STRING of digits. `"500000000"` means $500. Sending a JSON number
    /// here will be rejected by the oracle.
    pub amount: String,
    /// Optional credit level hint (1–4). The oracle will validate against
    /// the agent's score; pass the level returned by check_credit_eligibility
    /// to avoid surprises.
    pub credit_level: Option<u8>,
    /// Optional annual rate in basis points. Defaults to the level's posted
    /// rate when omitted.
    pub rate_bps: Option<i64>,
    /// USDC value of any posted collateral, as a string. Defaults to "0".
    pub collateral_value_usdc: Option<String>,
}

impl DynAomiTool for BorrowUsdc {
    type App = KrexaApp;
    type Args = BorrowUsdcArgs;
    const NAME: &'static str = "krexa_borrow_usdc";
    const DESCRIPTION: &'static str = "Draw USDC against your Krexa credit line. The oracle verifies your score and CO-SIGNS a borrow transaction; this tool returns the resulting base64-encoded, partially-signed Solana transaction. The agent host MUST add the second signature and submit to Solana — this tool does NOT broadcast. Use check_credit_eligibility first to confirm `availableCredit ≥ amount`.";

    fn run(_app: &KrexaApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        // The progenitor-generated `credit_level` is `Option<NonZeroU64>`
        // because the spec sets minimum:1. Convert from a friendly u8.
        let credit_level = match args.credit_level {
            Some(n) if (1..=4).contains(&n) => {
                Some(std::num::NonZeroU64::new(n as u64).expect("non-zero by range check"))
            }
            Some(_) => return Err("[krexa] borrow_usdc: credit_level must be 1..=4".to_string()),
            None => None,
        };
        let api_key = auth::api_key(&ctx)?;
        let body = SignCreditRequest {
            agent_pubkey: args.agent_pubkey,
            agent_or_owner_pubkey: args.agent_or_owner_pubkey,
            amount: args.amount,
            collateral_value_usdc: args
                .collateral_value_usdc
                .unwrap_or_else(|| "0".to_string()),
            credit_level,
            rate_bps: args.rate_bps,
        };
        let runtime = rt()?;
        let result = runtime
            .block_on(async move {
                client()
                    .sign_credit_transaction(api_key.as_str(), &body)
                    .await
            })
            .map_err(|e| format!("[krexa] borrow_usdc: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct PayApiCall;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PayApiCallArgs {
    /// Solana agent public key (base58). Identifies the Pay.sh wallet that
    /// will fund the call.
    pub agent: String,
    /// The third-party API URL to invoke (e.g.
    /// `https://gemini.paysh.dev/v1/generate`).
    pub target_url: String,
    /// HTTP method for the underlying call. Defaults to "GET".
    pub method: Option<String>,
    /// JSON body for POST/PUT/PATCH. Ignored for GET/DELETE.
    pub body: Option<serde_json::Map<String, Value>>,
    /// Optional custom headers. Authorization, Cookie, and X-API-Key are
    /// stripped before proxying.
    pub headers: Option<std::collections::HashMap<String, String>>,
    /// The owner wallet pubkey (fee payer for the USDC transfer).
    pub owner_address: String,
    /// Hard cap on USDC the agent is willing to pay for this single call.
    /// Omit to use the budget's per-call max.
    pub max_payment_usdc: Option<f64>,
    /// If true and the Pay.sh wallet is underfunded, auto-draw the deficit
    /// from the credit line before paying. Default false.
    pub use_credit: Option<bool>,
}

impl DynAomiTool for PayApiCall {
    type App = KrexaApp;
    type Args = PayApiCallArgs;
    const NAME: &'static str = "krexa_pay_api_call";
    const DESCRIPTION: &'static str = "Build a Pay.sh payment transaction for an x402-priced API call. Probes the target URL, checks the budget and funding, and returns a base64 unsigned Solana tx (`paymentRequired=true, funded=true, transaction=...`). When the wallet is underfunded and `use_credit=true`, instead returns `autoCreditDraw` instructions to call borrow_usdc first — re-invoke this tool after the credit-draw lands. The agent host signs + submits the returned tx, then calls Pay.sh's confirm endpoint out-of-band.";

    fn run(_app: &KrexaApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = auth::api_key(&ctx)?;
        let body = PayshCallRequest {
            body: args.body.unwrap_or_default(),
            headers: args.headers.unwrap_or_default(),
            max_payment_usdc: args.max_payment_usdc,
            method: args.method.unwrap_or_else(|| "GET".to_string()),
            owner_address: args.owner_address,
            target_url: args.target_url,
            use_credit: args.use_credit.unwrap_or(false),
        };
        let runtime = rt()?;
        let result = runtime
            .block_on(async move {
                client()
                    .paysh_call(args.agent.as_str(), api_key.as_str(), &body)
                    .await
            })
            .map_err(|e| format!("[krexa] pay_api_call: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct SetBudget;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SetBudgetArgs {
    /// Solana agent public key (base58).
    pub agent: String,
    /// New daily spend cap, in USDC.
    pub daily_limit_usdc: Option<f64>,
    /// Hard cap on USDC per single Pay.sh call.
    pub per_call_max_usdc: Option<f64>,
    /// Rolling 30-day spend cap, in USDC.
    pub monthly_cap_usdc: Option<f64>,
    /// Emit an alert when usage crosses this percent of the current cap
    /// (0–100).
    pub alert_at_pct: Option<f64>,
    /// If true, freeze all outbound Pay.sh calls immediately. Useful as a
    /// kill switch when the strategy mis-behaves.
    pub is_paused: Option<bool>,
}

impl DynAomiTool for SetBudget {
    type App = KrexaApp;
    type Args = SetBudgetArgs;
    const NAME: &'static str = "krexa_set_budget";
    const DESCRIPTION: &'static str = "Update Pay.sh spending guardrails (daily / per-call / monthly caps, alert threshold, pause switch). All fields are optional — only the ones provided are updated. Returns the new budget state with current usage. The agent should call this once at startup, then adjust if the strategy expands or after large drawdowns.";

    fn run(_app: &KrexaApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = auth::api_key(&ctx)?;
        let body = PayshBudgetUpdate {
            alert_at_pct: args.alert_at_pct,
            daily_limit_usdc: args.daily_limit_usdc,
            is_paused: args.is_paused,
            monthly_cap_usdc: args.monthly_cap_usdc,
            per_call_max_usdc: args.per_call_max_usdc,
        };
        let runtime = rt()?;
        let result = runtime
            .block_on(async move {
                client()
                    .paysh_set_budget(args.agent.as_str(), api_key.as_str(), &body)
                    .await
            })
            .map_err(|e| format!("[krexa] set_budget: {e}"))?;
        ok(result.into_inner())
    }
}

// ============================================================================
// MONITOR
// ============================================================================

pub(crate) struct GetBalance;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetBalanceArgs {
    /// Solana agent public key (base58).
    pub agent: String,
}

impl DynAomiTool for GetBalance {
    type App = KrexaApp;
    type Args = GetBalanceArgs;
    const NAME: &'static str = "krexa_get_balance";
    const DESCRIPTION: &'static str = "One-shot view of liquidity: Pay.sh wallet USDC balance, available credit (limit / used / remaining), total spendable, and frozen state. Cheaper than calling check_credit_eligibility + get_credit_line separately when you only need a go/no-go signal.";

    fn run(_app: &KrexaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client().paysh_balance(args.agent.as_str()).await })
            .map_err(|e| format!("[krexa] get_balance: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct GetCreditLine;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCreditLineArgs {
    /// Solana agent public key (base58).
    pub agent: String,
}

impl DynAomiTool for GetCreditLine {
    type App = KrexaApp;
    type Args = GetCreditLineArgs;
    const NAME: &'static str = "krexa_get_credit_line";
    const DESCRIPTION: &'static str = "Details on the agent's ACTIVE credit line: principal, accrued interest, total owed, annual rate (bps), `openedAt`, and `healthFactor` (bps; 15000 = 150%). Use to decide whether to repay, top up collateral, or pause new borrows. All amounts are USDC base units (6 decimals).";

    fn run(_app: &KrexaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client().get_credit_line(args.agent.as_str()).await })
            .map_err(|e| format!("[krexa] get_credit_line: {e}"))?;
        ok(result.into_inner())
    }
}

// ============================================================================
// SCOUT
// ============================================================================

pub(crate) struct SearchAgents;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SearchAgentsArgs {
    /// Wallet prefix, `.sol` domain fragment, or agent-name fragment.
    /// Returns up to 20 matches with name + score + trust tier.
    pub q: String,
}

impl DynAomiTool for SearchAgents {
    type App = KrexaApp;
    type Args = SearchAgentsArgs;
    const NAME: &'static str = "krexa_search_agents";
    const DESCRIPTION: &'static str = "Autocomplete-style search across Krexa's KYA registry. Returns up to 20 hits with `name`, `wallet`, `score`, and `trustTier`. Use when scouting counterparties or copy-trade targets — pair with lookup_agent for a fuller profile on a specific match.";

    fn run(_app: &KrexaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client().search_kya_agents(args.q.as_str()).await })
            .map_err(|e| format!("[krexa] search_agents: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct LookupAgent;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct LookupAgentArgs {
    /// Wallet address, `.sol` domain, or agent name to look up.
    pub input: String,
}

impl DynAomiTool for LookupAgent {
    type App = KrexaApp;
    type Args = LookupAgentArgs;
    const NAME: &'static str = "krexa_lookup_agent";
    const DESCRIPTION: &'static str = "Get a quick KYA snapshot for ONE agent (by wallet, `.sol` domain, or name): score, trust tier, level, `onTimeRate`, `operatingDays`, `defaultCount`, `verified`. Designed for counterparty evaluation — cheaper than the full credential and rich enough to decide whether to copy-trade or extend trust.";

    fn run(_app: &KrexaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client().get_kya_quick(args.input.as_str()).await })
            .map_err(|e| format!("[krexa] lookup_agent: {e}"))?;
        ok(result.into_inner())
    }
}

// ============================================================================
// ONBOARD
// ============================================================================

pub(crate) struct RedeemInvite;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RedeemInviteArgs {
    /// Invite code in `KREXA-XXXX-XXXX` format. One code per wallet; the
    /// binding is permanent after redemption.
    pub code: String,
    /// Agent wallet pubkey (base58) being activated. The same wallet's
    /// keypair (`KREXA_AGENT_SECRET_KEY`) must sign the challenge — the
    /// tool fetches the challenge from `/access/challenge` and signs it
    /// internally before posting `/access/redeem`.
    pub wallet: String,
}

impl DynAomiTool for RedeemInvite {
    type App = KrexaApp;
    type Args = RedeemInviteArgs;
    const NAME: &'static str = "krexa_redeem_invite";
    const DESCRIPTION: &'static str = "Activate a wallet on Krexa by redeeming an invite code. Composite: fetches the challenge string from `/access/challenge`, signs it with the agent keypair (KREXA_AGENT_SECRET_KEY, base58), then posts `/access/redeem`. Returns `{ activated: true }` on success. Required before any gated endpoint (borrow, Pay.sh writes) will accept the wallet. One code per wallet — the binding is permanent.";

    fn run(_app: &KrexaApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        // Two-step composite: ask the server for the canonical challenge,
        // then sign+submit. We could format the challenge locally (it's
        // deterministic), but the server is the source of truth and the
        // round-trip catches typos before signing.
        let signature = auth::sign_invite_challenge(&ctx, &args.code, &args.wallet)?;
        let challenge_body = InviteChallengeRequest {
            code: args.code.clone(),
            wallet: args.wallet.clone(),
        };
        let redeem_body = RedeemInviteRequest {
            code: args.code,
            wallet: args.wallet,
            signature,
        };
        let result = runtime
            .block_on(async move {
                let c = client();
                // Touch /access/challenge so a code-typo surfaces here
                // (clearer 4xx) rather than as an opaque signature error
                // at /access/redeem.
                let _challenge = c.get_invite_challenge(&challenge_body).await?;
                c.redeem_invite(&redeem_body).await
            })
            .map_err(|e| format!("[krexa] redeem_invite: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct DeployAgent;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct DeployAgentArgs {
    /// Agent's Solana pubkey (base58).
    pub agent: String,
    /// Owner wallet pubkey (base58). Same as `agent` for self-custodied
    /// agents.
    pub owner: String,
    /// Display name. Defaults to `aomi-agent` if omitted.
    pub name: Option<String>,
    /// 0 = Trader, 1 = Service, 2 = Hybrid. Defaults to Trader.
    pub agent_type: Option<u8>,
}

impl DynAomiTool for DeployAgent {
    type App = KrexaApp;
    type Args = DeployAgentArgs;
    const NAME: &'static str = "krexa_deploy_agent";
    const DESCRIPTION: &'static str = "Build the on-chain agent-deploy transaction. Idempotent: returns `{ status: \"exists\" }` if the agent is already deployed, otherwise `{ status: \"ready\", transaction: \"<base64 partial-signed tx>\" }`. The oracle's KYA signature is already attached; the host adds the agent + owner signatures (typically the same keypair) and submits to Solana RPC. Required after `redeem_invite` and BEFORE `request_credit` — credit endpoints 404 until the on-chain deploy lands. Needs ~0.01 SOL on the agent wallet for rent.";

    fn run(_app: &KrexaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let body = DeployAgentRequest {
            agent: args.agent,
            owner: args.owner,
            name: Some(args.name.unwrap_or_else(|| "aomi-agent".to_string())),
            agent_type: args.agent_type.map(|n| n as i64),
        };
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client().deploy_agent(&body).await })
            .map_err(|e| format!("[krexa] deploy_agent: {e}"))?;
        ok(result.into_inner())
    }
}

// ============================================================================
// PAY.SH PROXY (provider-managed paid API calls)
// ============================================================================

pub(crate) struct ListProxyProviders;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ListProxyProvidersArgs {}

impl DynAomiTool for ListProxyProviders {
    type App = KrexaApp;
    type Args = ListProxyProvidersArgs;
    const NAME: &'static str = "krexa_list_proxy_providers";
    const DESCRIPTION: &'static str = "List the Pay.sh proxy providers (OpenAI, Anthropic, etc.) with their available routes and per-route USDC pricing. Each entry carries a `proxyAvailable: true` flag. Replaces the legacy `paysh_catalog` whose URLs were placeholders. Call this first when you don't already know which provider/route you want.";

    fn run(_app: &KrexaApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client().paysh_proxy_list_providers().await })
            .map_err(|e| format!("[krexa] list_proxy_providers: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct GetProxyQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetProxyQuoteArgs {
    /// Provider id from `list_proxy_providers` (e.g. `openai`,
    /// `anthropic`).
    pub provider: String,
    /// Upstream route to call, including leading slash (e.g.
    /// `/v1/chat/completions`).
    pub route: String,
}

impl DynAomiTool for GetProxyQuote {
    type App = KrexaApp;
    type Args = GetProxyQuoteArgs;
    const NAME: &'static str = "krexa_get_proxy_quote";
    const DESCRIPTION: &'static str = "Get the USDC price + gateway wallet (`payTo`) for a single proxy call. Returns `{ priceUsdc, payTo, priceBaseUnits }`. After this, the agent transfers `priceBaseUnits` USDC to `payTo` on Solana, captures the tx signature, and calls `proxy_call` with `X-PAYMENT` set to that signature.";

    fn run(_app: &KrexaApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let api_key = auth::api_key(&ctx)?;
        let body = PayshProxyQuoteRequest {
            provider: args.provider,
            route: args.route,
        };
        let runtime = rt()?;
        let result = runtime
            .block_on(async move { client().paysh_proxy_quote(api_key.as_str(), &body).await })
            .map_err(|e| format!("[krexa] get_proxy_quote: {e}"))?;
        ok(result.into_inner())
    }
}

pub(crate) struct ProxyCall;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ProxyCallArgs {
    /// Provider id from `list_proxy_providers` (e.g. `openai`).
    pub provider: String,
    /// Upstream route, including leading slash (e.g. `/v1/chat/completions`).
    pub route: String,
    /// Solana transaction signature of the agent's USDC payment to the
    /// gateway `payTo` returned by `get_proxy_quote`. Goes into
    /// `X-PAYMENT`.
    pub payment_tx_signature: String,
    /// Agent's Solana pubkey (base58). Goes into `X-AGENT-PUBKEY`.
    pub agent_pubkey: String,
    /// Upstream request body in the provider's native format (e.g.
    /// `{ model, messages, ... }` for OpenAI chat completions).
    pub body: serde_json::Map<String, Value>,
}

impl DynAomiTool for ProxyCall {
    type App = KrexaApp;
    type Args = ProxyCallArgs;
    const NAME: &'static str = "krexa_proxy_call";
    const DESCRIPTION: &'static str = "Make a Pay.sh proxy call after the USDC payment has landed on-chain. Body is the provider's native request shape (OpenAI / Anthropic / etc.); response is the raw upstream reply. The agent's USDC transfer must precede this call — pass the resulting tx signature as `payment_tx_signature`. Wildcard route path is constructed at runtime, so this tool is hand-rolled with reqwest rather than going through the generated client.";

    fn run(_app: &KrexaApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        // Hand-rolled because the upstream route is a wildcard
        // path component progenitor doesn't model cleanly.
        let api_key = auth::api_key(&ctx)?;
        let route = if args.route.starts_with('/') {
            args.route
        } else {
            format!("/{}", args.route)
        };
        let url = format!(
            "{}/solana/paysh/proxy/{}{}",
            base_url(),
            args.provider,
            route
        );
        let body_value = Value::Object(args.body);

        let runtime = rt()?;
        let result = runtime
            .block_on(async move {
                let http = reqwest::Client::new();
                let response = http
                    .post(&url)
                    .header("X-API-Key", api_key)
                    .header("X-PAYMENT", &args.payment_tx_signature)
                    .header("X-AGENT-PUBKEY", &args.agent_pubkey)
                    .json(&body_value)
                    .send()
                    .await
                    .map_err(|e| format!("send: {e}"))?;
                let status = response.status();
                let text = response
                    .text()
                    .await
                    .map_err(|e| format!("read body: {e}"))?;
                if !status.is_success() {
                    return Err(format!("upstream {status}: {text}"));
                }
                // Pass the upstream JSON through. Fall back to a
                // string-shaped value if the body isn't JSON.
                let parsed: Value = serde_json::from_str(&text).unwrap_or(Value::String(text));
                Ok(parsed)
            })
            .map_err(|e: String| format!("[krexa] proxy_call: {e}"))?;
        ok(result)
    }
}
