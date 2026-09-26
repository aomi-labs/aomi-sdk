//! Curated tool layer for Across Protocol. Hand-written from the
//! progenitor-generated client at `aomi_ext::across::Client` — see
//! ext/specs/across.yaml for the full surface.
//!
//! Across is unauthenticated; the client just takes a base URL.

use aomi_ext::across::Client as AcrossClient;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub(crate) struct AcrossApp;

const BASE_URL: &str = "https://app.across.to/api";

// ============================================================================
// Helpers
// ============================================================================

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[across] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("across".to_string()));
            Value::Object(map)
        }
        other => serde_json::json!({ "source": "across", "data": other }),
    })
}

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[across] runtime: {e}"))
}

fn client() -> AcrossClient {
    let base = std::env::var("ACROSS_API_ENDPOINT").unwrap_or_else(|_| BASE_URL.to_string());
    AcrossClient::new(&base)
}

// ============================================================================
// AcrossListRoutes — discovery
// ============================================================================

pub(crate) struct AcrossListRoutes;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AcrossListRoutesArgs {
    /// Filter by origin chain ID (numeric, e.g. 1 for Ethereum, 42161 for Arbitrum). Optional.
    #[serde(default)]
    pub origin_chain_id: Option<i64>,
    /// Filter by destination chain ID. Optional.
    #[serde(default)]
    pub destination_chain_id: Option<i64>,
    /// Filter by origin ERC-20 address (0x..., checksummed). Optional.
    #[serde(default)]
    pub origin_token: Option<String>,
    /// Filter by destination ERC-20 address. Optional.
    #[serde(default)]
    pub destination_token: Option<String>,
}

impl DynAomiTool for AcrossListRoutes {
    type App = AcrossApp;
    type Args = AcrossListRoutesArgs;
    const NAME: &'static str = "across_list_routes";
    const DESCRIPTION: &'static str = "Use to discover supported Across bridge routes (which token on which origin chain can be bridged to which destination). Returns route entries with origin/destination chain IDs and token addresses. Filter by any combination of chain or token to narrow results.";

    fn run(_app: &AcrossApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let routes = client()
                .get_available_routes(
                    args.destination_chain_id,
                    args.destination_token.as_deref(),
                    args.origin_chain_id,
                    args.origin_token.as_deref(),
                )
                .await
                .map_err(|e| format!("[across] available routes: {e}"))?
                .into_inner();
            ok(routes)
        })
    }
}

// ============================================================================
// AcrossGetLimits
// ============================================================================

pub(crate) struct AcrossGetLimits;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AcrossGetLimitsArgs {
    /// Origin-chain ERC-20 address (input token).
    pub input_token: String,
    /// Destination-chain ERC-20 address (output token).
    pub output_token: String,
    /// Origin chain ID (numeric).
    pub origin_chain_id: i64,
    /// Destination chain ID (numeric).
    pub destination_chain_id: i64,
}

impl DynAomiTool for AcrossGetLimits {
    type App = AcrossApp;
    type Args = AcrossGetLimitsArgs;
    const NAME: &'static str = "across_get_limits";
    const DESCRIPTION: &'static str = "Use before quoting a bridge to check that the user's amount is within bounds. Returns minDeposit, maxDeposit, and recommended instant-fill caps for the given origin token / destination token / chain pair, in the input token's smallest unit.";

    fn run(_app: &AcrossApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let limits = client()
                .get_limits(
                    args.destination_chain_id,
                    args.input_token.as_str(),
                    args.origin_chain_id,
                    args.output_token.as_str(),
                )
                .await
                .map_err(|e| format!("[across] limits: {e}"))?
                .into_inner();
            ok(limits)
        })
    }
}

// ============================================================================
// AcrossGetBridgeQuote — fee + output amount + relayer params
// ============================================================================

pub(crate) struct AcrossGetBridgeQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AcrossGetBridgeQuoteArgs {
    /// Origin-chain ERC-20 address (input token).
    pub input_token: String,
    /// Destination-chain ERC-20 address (output token).
    pub output_token: String,
    /// Origin chain ID (e.g. 1, 10, 137, 8453, 42161).
    pub origin_chain_id: i64,
    /// Destination chain ID.
    pub destination_chain_id: i64,
    /// Input amount in the input token's smallest unit (1 USDC = "1000000").
    pub amount: String,
    /// Recipient address on the destination chain. Defaults to depositor when omitted.
    #[serde(default)]
    pub recipient: Option<String>,
    /// Optional cross-chain message hex (for atomic actions on destination).
    #[serde(default)]
    pub message: Option<String>,
}

impl DynAomiTool for AcrossGetBridgeQuote {
    type App = AcrossApp;
    type Args = AcrossGetBridgeQuoteArgs;
    const NAME: &'static str = "across_get_bridge_quote";
    const DESCRIPTION: &'static str = "Use when the user wants to bridge a token via Across. Returns `outputAmount`, the fee breakdown (totalRelayFee, lpFee, gasFee, capitalFee), `estimatedFillTimeSec`, and the relayer parameters needed to call SpokePool. To execute, the host must call `depositV3` (or `deposit`) on the origin-chain SpokePool with these parameters; this tool does NOT return raw calldata.";

    fn run(_app: &AcrossApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let fees = client()
                .get_suggested_fees(
                    args.amount.as_str(),
                    args.destination_chain_id,
                    args.input_token.as_str(),
                    args.message.as_deref(),
                    args.origin_chain_id,
                    args.output_token.as_str(),
                    args.recipient.as_deref(),
                )
                .await
                .map_err(|e| format!("[across] suggested fees: {e}"))?
                .into_inner();
            ok(fees)
        })
    }
}

// ============================================================================
// AcrossBridge — quote + (optional) approval + depositV3, routed via host wallet
// ============================================================================
//
// Closes the quote→execute gap left by `across_get_bridge_quote`. Fetches the
// raw suggested-fees JSON (so we keep relayer params + `spokePoolAddress` that
// the typed `AcrossSuggestedFees` struct drops), then routes one or two
// `host::StageTx`es through the wallet: an optional ERC-20 approval against the
// origin-chain SpokePool, followed by the SpokePool `depositV3` call. The
// destination-chain fill is observed separately via `across_get_deposit_status`.

const NATIVE_SENTINEL: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";
const DEPOSIT_V3_SIGNATURE: &str = "depositV3(address,address,address,address,uint256,uint256,uint256,address,uint32,uint32,uint32,bytes)";

fn is_native(addr: &str) -> bool {
    addr.eq_ignore_ascii_case(NATIVE_SENTINEL)
}

pub(crate) struct AcrossBridge;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AcrossBridgeArgs {
    /// Origin-chain ERC-20 address (input token). Use 0xEeeE...EEeE only when bridging the chain's native asset (e.g. ETH wrapped via WETH spoke); SpokePool itself wraps when `msg.value` matches `inputAmount`.
    pub input_token: String,
    /// Destination-chain ERC-20 address (output token).
    pub output_token: String,
    /// Origin chain ID (e.g. 1, 10, 137, 8453, 42161).
    pub origin_chain_id: i64,
    /// Destination chain ID.
    pub destination_chain_id: i64,
    /// Input amount in the input token's smallest unit (1 USDC = "1000000").
    pub amount: String,
    /// Depositor address on the origin chain (the wallet signing the deposit tx).
    pub depositor: String,
    /// Recipient address on the destination chain. Defaults to `depositor` when omitted.
    #[serde(default)]
    pub recipient: Option<String>,
    /// Optional cross-chain message hex (defaults to "0x"). Pass as `0x...` if the receiver needs a payload.
    #[serde(default)]
    pub message: Option<String>,
}

/// Hit `/suggested-fees` directly so we keep every field — the typed
/// `AcrossSuggestedFees` struct in `aomi_ext::across` drops `spokePoolAddress`,
/// `exclusiveRelayer`, `exclusivityDeadline`, `timestamp`, `fillDeadline`,
/// etc. because they aren't declared as schema properties.
fn fetch_suggested_fees_raw(args: &AcrossBridgeArgs) -> Result<Value, String> {
    let base = std::env::var("ACROSS_API_ENDPOINT").unwrap_or_else(|_| BASE_URL.to_string());
    let url = format!("{base}/suggested-fees");
    let recipient = args
        .recipient
        .clone()
        .unwrap_or_else(|| args.depositor.clone());

    let mut query: Vec<(&str, String)> = vec![
        ("amount", args.amount.clone()),
        ("originChainId", args.origin_chain_id.to_string()),
        ("destinationChainId", args.destination_chain_id.to_string()),
        ("inputToken", args.input_token.clone()),
        ("outputToken", args.output_token.clone()),
        ("recipient", recipient),
    ];
    if let Some(m) = args.message.as_ref() {
        query.push(("message", m.clone()));
    }

    let runtime = rt()?;
    runtime.block_on(async move {
        let resp = reqwest::Client::new()
            .get(&url)
            .header("api-version", "1")
            .header(reqwest::header::ACCEPT, "application/json")
            .query(&query)
            .send()
            .await
            .map_err(|e| format!("[across] suggested-fees request: {e}"))?;
        let status = resp.status();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| format!("[across] suggested-fees decode: {e}"))?;
        if !status.is_success() {
            return Err(format!("[across] suggested-fees HTTP {status}: {body}"));
        }
        Ok(body)
    })
}

fn require_str<'a>(obj: &'a Value, key: &str) -> Result<&'a str, String> {
    obj.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("[across] suggested-fees response missing string `{key}`"))
}

/// Pull a uint32 field from the response. The Across API returns these as
/// numbers (`timestamp`, `fillDeadline`, `exclusivityDeadline`) but occasionally
/// as strings — accept either and normalize to a decimal string for the host's
/// `data.encode` (uint32 args).
fn parse_uint32(obj: &Value, key: &str) -> Result<String, String> {
    let v = obj
        .get(key)
        .ok_or_else(|| format!("[across] suggested-fees response missing `{key}`"))?;
    if let Some(n) = v.as_u64() {
        return Ok(n.to_string());
    }
    if let Some(s) = v.as_str() {
        return Ok(s.to_string());
    }
    Err(format!(
        "[across] suggested-fees `{key}` is neither a number nor a string: {v}"
    ))
}

impl DynAomiTool for AcrossBridge {
    type App = AcrossApp;
    type Args = AcrossBridgeArgs;
    const NAME: &'static str = "across_bridge";
    const DESCRIPTION: &'static str = "USE THIS to execute any Across bridge. Single-call composite: fetches a fresh suggested-fees quote (carries the relayer params + origin-chain SpokePool address), then routes — through the host wallet — an optional ERC-20 approval for the SpokePool, followed by the SpokePool `depositV3` call (host ABI-encodes via `data.encode`). DO NOT call `evm_stage_tx`, `simulate_batch`, `evm_commit_txs`, or `across_get_bridge_quote` first; this tool re-quotes internally and the route handles simulate + commit. DO NOT web-search SpokePool addresses or depositV3 ABIs — the tool already has them. After commit, the host returns the deposit tx hash; poll the destination-chain fill with `across_get_deposit_status`.";

    fn run_with_routes(
        _app: &AcrossApp,
        args: Self::Args,
        _ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        let quote = fetch_suggested_fees_raw(&args)?;
        let quote_obj = quote
            .as_object()
            .ok_or_else(|| "[across] suggested-fees response was not a JSON object".to_string())?;

        let spoke_pool = require_str(&quote, "spokePoolAddress")
            .map_err(|_| {
                "[across] suggested-fees response missing `spokePoolAddress`. Across publishes \
                 this per call; refusing to fall back to a hardcoded list to avoid signing into \
                 a stale SpokePool."
                    .to_string()
            })?
            .to_string();
        let output_amount = require_str(&quote, "outputAmount")?.to_string();
        let quote_timestamp = parse_uint32(&quote, "timestamp")?;
        let fill_deadline = parse_uint32(&quote, "fillDeadline")?;
        let exclusivity_deadline =
            parse_uint32(&quote, "exclusivityDeadline").unwrap_or_else(|_| "0".to_string());
        let exclusive_relayer = quote_obj
            .get("exclusiveRelayer")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(ZERO_ADDRESS)
            .to_string();

        let recipient = args
            .recipient
            .clone()
            .unwrap_or_else(|| args.depositor.clone());
        let message = args.message.clone().unwrap_or_else(|| "0x".to_string());
        let native_input = is_native(&args.input_token);

        // depositV3 args, in the order the SpokePool ABI declares them. Let the
        // host's `data.encode` ABI-encode rather than hand-rolling calldata.
        let deposit_args = json!([
            args.depositor,
            recipient,
            args.input_token,
            args.output_token,
            args.amount,
            output_amount,
            args.destination_chain_id.to_string(),
            exclusive_relayer,
            quote_timestamp,
            fill_deadline,
            exclusivity_deadline,
            message,
        ]);

        let deposit_value = if native_input {
            args.amount.clone()
        } else {
            "0".to_string()
        };

        let mut stage_args: Vec<Value> = Vec::new();
        if !native_input {
            // Unconditional approval: there's no `host::EthCall` target in the
            // SDK to read the current allowance, and the task spec explicitly
            // allows unconditional approve when a pre-check isn't available.
            stage_args.push(json!({
                "to": args.input_token,
                "description": format!(
                    "Across ERC-20 approval for SpokePool {} on chain {} (input token {})",
                    spoke_pool, args.origin_chain_id, args.input_token
                ),
                "data": {
                    "encode": {
                        "signature": "approve(address,uint256)",
                        "args": [spoke_pool.clone(), args.amount.clone()],
                    }
                },
                "value": "0",
                "kind": "erc20_approve",
            }));
        }
        stage_args.push(json!({
            "to": spoke_pool,
            "description": format!(
                "Across depositV3: bridge {} of {} from chain {} to {} on chain {} (recipient {})",
                args.amount,
                args.input_token,
                args.origin_chain_id,
                args.output_token,
                args.destination_chain_id,
                recipient
            ),
            "data": {
                "encode": {
                    "signature": DEPOSIT_V3_SIGNATURE,
                    "args": deposit_args,
                }
            },
            "value": deposit_value,
            "kind": "bridge_deposit",
        }));
        let last_index = stage_args.len() - 1;

        let preview = json!({
            "status": "awaiting_wallet",
            "origin_chain_id": args.origin_chain_id,
            "destination_chain_id": args.destination_chain_id,
            "spoke_pool": spoke_pool,
            "needs_approval": !native_input,
            "tx_count": stage_args.len(),
            "quote": quote,
        });

        ToolReturn::route(ok(preview)?)
            .next(|next| {
                for (i, a) in stage_args.iter().enumerate() {
                    let step = next.add::<host::StageTx>(a.clone());
                    if i == last_index {
                        step.note(
                            "Follow the host-owned route continuation using compatible resources actually issued for this operation. Preserve source restrictions and prerequisite ordering; do not copy opaque bytes into model arguments. Simulate and commit the same complete ordered staged-resource cohort. Full raw payloads and callback fields remain unchanged in host routes.",
                        )
                        .enforce(EnforcementPolicy::Continue, |enforce| {
                            enforce.add::<host::SimulateBatch>(json!({}));
                            enforce
                                .add::<host::CommitTxs>(json!({ "aa_preference": "auto" }))
                                .bind_as("transaction_hash");
                        });
                    } else {
                        step.note(
                            "Follow the host-owned route continuation using compatible resources actually issued for this operation. Preserve source restrictions and prerequisite ordering; do not copy opaque bytes into model arguments. Simulate and commit the same complete ordered staged-resource cohort. Full raw payloads and callback fields remain unchanged in host routes.",
                        );
                    }
                }
            })
            // No `.after::<>` — the destination-chain fill is async and observed
            // via `across_get_deposit_status`, which the user polls separately.
            // The bound `transaction_hash` ends the route.
            .try_build()
            .map_err(|e| format!("[across] route build: {e}"))
    }
}

// ============================================================================
// AcrossGetDepositStatus
// ============================================================================

pub(crate) struct AcrossGetDepositStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AcrossGetDepositStatusArgs {
    /// Origin chain ID where the deposit was made.
    pub origin_chain_id: i64,
    /// Numeric deposit ID emitted by the origin SpokePool when the deposit tx confirmed.
    pub deposit_id: i64,
}

impl DynAomiTool for AcrossGetDepositStatus {
    type App = AcrossApp;
    type Args = AcrossGetDepositStatusArgs;
    const NAME: &'static str = "across_get_deposit_status";
    const DESCRIPTION: &'static str = "Use to track an Across bridge deposit. Returns the fill status (PENDING / FILLED / etc.) and, when filled, the destination-chain fill tx hash. Poll while the user is waiting (typical fill is under 30 seconds for instant-eligible amounts).";

    fn run(_app: &AcrossApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let runtime = rt()?;
        runtime.block_on(async move {
            let status = client()
                .get_deposit_status(args.deposit_id, args.origin_chain_id)
                .await
                .map_err(|e| format!("[across] deposit status: {e}"))?
                .into_inner();
            ok(status)
        })
    }
}
