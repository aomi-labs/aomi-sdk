use crate::client::{
    CreateTradeRequest, CurrencyAmount, FundRequest, FundingPresignRequest, QuoteRequest,
    QuoteResponse, StableFxClient,
};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

const ARC_TESTNET_CHAIN_ID: u64 = 5_042_002;
const PERMIT2_ADDRESS: &str = "0x000000000022D473030F116dDEE9F6B43aC78BA3";
const STABLEFX_ESCROW_ADDRESS: &str = "0x867650F5eAe8df91445971f14d89fd84F0C9a9f8";

#[derive(Clone, Default)]
pub(crate) struct StableFxApp;

fn rt() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new().map_err(|e| format!("[stablefx] runtime: {e}"))
}

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value =
        serde_json::to_value(value).map_err(|e| format!("[stablefx] serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".into(), Value::String("circle_stablefx".into()));
            Value::Object(map)
        }
        other => json!({ "source": "circle_stablefx", "data": other }),
    })
}

fn require_arc(ctx: &DynToolCallCtx) -> Result<(), String> {
    let chain_id = ctx
        .attribute_u64(&["domain", "evm", "chain_id"])
        .ok_or_else(|| {
            "[stablefx] Arc Testnet must be selected before using StableFX".to_string()
        })?;
    if chain_id != ARC_TESTNET_CHAIN_ID {
        return Err(format!(
            "[stablefx] StableFX is only available on Arc Testnet (chainId {ARC_TESTNET_CHAIN_ID}); selected chainId is {chain_id}"
        ));
    }
    Ok(())
}

fn client(ctx: &DynToolCallCtx) -> Result<StableFxClient, String> {
    let api_key = resolve_secret_value(
        ctx,
        None,
        "STABLEFX_API_KEY",
        "[stablefx] add a Circle StableFX API key in package settings before using StableFX",
    )?;
    StableFxClient::new(&api_key)
}

fn connected_wallet(ctx: &DynToolCallCtx) -> Result<String, String> {
    let wallet = ctx
        .attribute_string(&["domain", "evm", "address"])
        .ok_or_else(|| "[stablefx] no EVM wallet is connected".to_string())?;
    validate_address(&wallet)?;
    Ok(wallet)
}

fn validate_address(address: &str) -> Result<(), String> {
    if address.len() == 42
        && address.starts_with("0x")
        && address[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err(format!("[stablefx] invalid EVM address: {address}"))
    }
}

fn normalize_currency(currency: String) -> Result<String, String> {
    let currency = currency.trim().to_ascii_uppercase();
    if (3..=8).contains(&currency.len()) && currency.chars().all(|c| c.is_ascii_alphanumeric()) {
        Ok(currency)
    } else {
        Err("[stablefx] currency must be a 3-8 character symbol".to_string())
    }
}

fn validate_pair(from: &str, to: &str) -> Result<(), String> {
    if from == to {
        return Err("[stablefx] source and destination currencies must differ".to_string());
    }
    if from != "USDC" && to != "USDC" {
        return Err("[stablefx] one side of every StableFX pair must be USDC".to_string());
    }
    Ok(())
}

fn validate_amount(amount: &str) -> Result<String, String> {
    let amount = amount.trim();
    let mut pieces = amount.split('.');
    let whole = pieces.next().unwrap_or_default();
    let fractional = pieces.next();
    let valid = !whole.is_empty()
        && whole.chars().all(|c| c.is_ascii_digit())
        && fractional
            .map(|part| {
                !part.is_empty() && part.len() <= 6 && part.chars().all(|c| c.is_ascii_digit())
            })
            .unwrap_or(true)
        && pieces.next().is_none()
        && amount.chars().any(|c| ('1'..='9').contains(&c));
    if valid {
        Ok(amount.to_string())
    } else {
        Err(
            "[stablefx] amount must be a positive decimal string with at most 6 fractional digits"
                .to_string(),
        )
    }
}

fn validate_tenor(tenor: Option<String>) -> Result<String, String> {
    let tenor = tenor
        .unwrap_or_else(|| "instant".to_string())
        .to_ascii_lowercase();
    match tenor.as_str() {
        "instant" | "hourly" | "daily" => Ok(tenor),
        _ => Err("[stablefx] tenor must be instant, hourly, or daily".to_string()),
    }
}

fn quote_request(
    from_currency: String,
    to_currency: String,
    amount: String,
    tenor: Option<String>,
    quote_type: &str,
    recipient: Option<String>,
) -> Result<QuoteRequest, String> {
    let from_currency = normalize_currency(from_currency)?;
    let to_currency = normalize_currency(to_currency)?;
    validate_pair(&from_currency, &to_currency)?;
    let amount = validate_amount(&amount)?;
    if let Some(address) = recipient.as_deref() {
        validate_address(address)?;
    }
    Ok(QuoteRequest {
        from: CurrencyAmount {
            currency: from_currency,
            amount: Some(amount),
        },
        to: CurrencyAmount {
            currency: to_currency,
            amount: None,
        },
        tenor: validate_tenor(tenor)?,
        quote_type: quote_type.to_string(),
        recipient_address: recipient,
    })
}

fn typed_data(quote: &QuoteResponse) -> Result<Value, String> {
    quote.typed_data.clone().ok_or_else(|| {
        "[stablefx] tradable quote did not include EIP-712 typedData; do not create a trade"
            .to_string()
    })
}

fn typed_message(typed_data: &Value) -> Result<Value, String> {
    typed_data
        .get("message")
        .cloned()
        .ok_or_else(|| "[stablefx] typedData is missing message".to_string())
}

fn validate_arc_typed_data(typed_data: &Value) -> Result<(), String> {
    let chain_id = typed_data
        .pointer("/domain/chainId")
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        })
        .ok_or_else(|| "[stablefx] typedData domain is missing a numeric chainId".to_string())?;
    if chain_id != ARC_TESTNET_CHAIN_ID {
        return Err(format!(
            "[stablefx] refusing non-Arc typedData: expected chainId {ARC_TESTNET_CHAIN_ID}, got {chain_id}"
        ));
    }
    let domain_name = typed_data
        .pointer("/domain/name")
        .and_then(Value::as_str)
        .ok_or_else(|| "[stablefx] typedData domain is missing name".to_string())?;
    if domain_name != "Permit2" {
        return Err(format!(
            "[stablefx] refusing unexpected EIP-712 domain {domain_name}; expected Permit2"
        ));
    }
    for (pointer, label, expected) in [
        (
            "/domain/verifyingContract",
            "EIP-712 verifying contract",
            PERMIT2_ADDRESS,
        ),
        (
            "/message/spender",
            "Permit2 spender",
            STABLEFX_ESCROW_ADDRESS,
        ),
    ] {
        let address = typed_data
            .pointer(pointer)
            .and_then(Value::as_str)
            .ok_or_else(|| format!("[stablefx] typedData is missing {label}"))?;
        if !address.eq_ignore_ascii_case(expected) {
            return Err(format!(
                "[stablefx] refusing unexpected {label} {address}; expected {expected}"
            ));
        }
    }
    let primary_type = typed_data
        .get("primaryType")
        .and_then(Value::as_str)
        .ok_or_else(|| "[stablefx] typedData is missing primaryType".to_string())?;
    if primary_type != "PermitWitnessTransferFrom" {
        return Err(format!(
            "[stablefx] refusing unexpected EIP-712 primaryType {primary_type}"
        ));
    }
    if typed_data.get("types").and_then(Value::as_object).is_none()
        || typed_data
            .get("message")
            .and_then(Value::as_object)
            .is_none()
    {
        return Err("[stablefx] typedData is incomplete".to_string());
    }
    Ok(())
}

fn validate_uuid(label: &str, value: &str) -> Result<(), String> {
    Uuid::parse_str(value)
        .map(|_| ())
        .map_err(|_| format!("[stablefx] {label} must be a UUID"))
}

fn validate_signature(signature: Option<String>) -> Result<String, String> {
    let signature = signature.ok_or_else(|| {
        "[stablefx] wallet signature is missing; this continuation must run from the routed signing step"
            .to_string()
    })?;
    if signature.len() >= 4
        && signature.starts_with("0x")
        && signature[2..].len().is_multiple_of(2)
        && signature[2..].chars().all(|c| c.is_ascii_hexdigit())
    {
        Ok(signature)
    } else {
        Err("[stablefx] wallet returned an invalid hex signature".to_string())
    }
}

// ============================================================================
// Indicative quote
// ============================================================================

pub(crate) struct Quote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct QuoteArgs {
    pub from_currency: String,
    pub to_currency: String,
    /// Human-unit decimal amount of the source currency.
    pub amount: String,
    /// Settlement schedule: instant (default), hourly, or daily.
    #[serde(default)]
    pub tenor: Option<String>,
}

impl DynAomiTool for Quote {
    type App = StableFxApp;
    type Args = QuoteArgs;
    const NAME: &'static str = "stablefx_quote";
    const DESCRIPTION: &'static str = "Get an indicative Circle StableFX rate without creating a trade or asking the wallet to sign. Amount is a human-unit decimal string. One side of the pair must be USDC.";

    fn run(_app: &StableFxApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        require_arc(&ctx)?;
        let request = quote_request(
            args.from_currency,
            args.to_currency,
            args.amount,
            args.tenor,
            "reference",
            None,
        )?;
        let runtime = rt()?;
        let response = runtime.block_on(client(&ctx)?.quote(&request))?;
        ok(response)
    }
}

// ============================================================================
// Tradable quote -> wallet signature -> create trade
// ============================================================================

pub(crate) struct AcceptQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AcceptQuoteArgs {
    pub from_currency: String,
    pub to_currency: String,
    /// Human-unit decimal amount of the source currency.
    pub amount: String,
    #[serde(default)]
    pub tenor: Option<String>,
    /// Recipient of the destination currency. Defaults to the connected wallet.
    #[serde(default)]
    pub recipient: Option<String>,
}

impl DynAomiTool for AcceptQuote {
    type App = StableFxApp;
    type Args = AcceptQuoteArgs;
    const NAME: &'static str = "stablefx_accept_quote";
    const DESCRIPTION: &'static str = "Execute a StableFX taker RFQ. Fetches a fresh tradable quote and routes Circle's exact Permit2 EIP-712 payload to the connected wallet; the continuation creates the trade automatically. Use only after the user has asked to execute.";

    fn run_with_routes(
        _app: &StableFxApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        require_arc(&ctx)?;
        let wallet = connected_wallet(&ctx)?;
        let recipient = args.recipient.unwrap_or_else(|| wallet.clone());
        let request = quote_request(
            args.from_currency,
            args.to_currency,
            args.amount,
            args.tenor,
            "tradable",
            Some(recipient),
        )?;
        let runtime = rt()?;
        let quote = runtime.block_on(client(&ctx)?.quote(&request))?;
        let typed_data = typed_data(&quote)?;
        validate_arc_typed_data(&typed_data)?;
        let message = typed_message(&typed_data)?;

        let submit_template = json!({
            "idempotency_key": Uuid::new_v4().to_string(),
            "quote_id": quote.id,
            "wallet": wallet,
            "message": message,
            "signature": null,
        });
        let wallet_request = json!({
            "typed_data": typed_data,
            "description": format!(
                "Accept Circle StableFX quote {} (expires {})",
                quote.id,
                quote.expires_at.as_deref().unwrap_or("at the time shown by Circle")
            ),
        });
        let preview = ok(json!({
            "status": "awaiting_wallet_signature",
            "quote": quote,
            "network": "Arc Testnet",
            "chain_id": ARC_TESTNET_CHAIN_ID,
        }))?;

        Ok(ToolReturn::route(preview)
            .next(|next| {
                next.add::<host::EvmCommitMessage>(wallet_request)
                    .bind_as("signature")
                    .note("Sign Circle's quote EIP-712 payload byte-for-byte. Do not modify the typed data.");
            })
            .after::<CreateTrade>(submit_template)
            .awaits("signature")
            .note("Wallet signed the quote — create the StableFX trade before the quote expires.")
            .build())
    }
}

pub(crate) struct CreateTrade;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct CreateTradeArgs {
    pub idempotency_key: String,
    pub quote_id: String,
    pub wallet: String,
    /// Exact `typedData.message` returned with the quote.
    pub message: Value,
    /// Filled by the routed wallet-signing step.
    #[serde(default)]
    pub signature: Option<String>,
}

impl DynAomiTool for CreateTrade {
    type App = StableFxApp;
    type Args = CreateTradeArgs;
    const NAME: &'static str = "stablefx_create_trade";
    const DESCRIPTION: &'static str = "Routed continuation of stablefx_accept_quote. It submits the pinned quote message and wallet signature to Circle. Do not invoke directly.";

    fn run(_app: &StableFxApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        require_arc(&ctx)?;
        let connected_wallet = connected_wallet(&ctx)?;
        validate_address(&args.wallet)?;
        if !args.wallet.eq_ignore_ascii_case(&connected_wallet) {
            return Err(
                "[stablefx] routed trade wallet does not match the connected wallet".to_string(),
            );
        }
        validate_uuid("idempotency_key", &args.idempotency_key)?;
        validate_uuid("quote_id", &args.quote_id)?;
        let signature = validate_signature(args.signature)?;
        let request = CreateTradeRequest {
            idempotency_key: args.idempotency_key,
            quote_id: args.quote_id,
            address: args.wallet,
            message: args.message,
            signature,
        };
        let runtime = rt()?;
        let trade = runtime.block_on(client(&ctx)?.create_trade(&request))?;
        ok(json!({
            "trade": trade,
            "next": "Poll stablefx_trade_status until pending_settlement, then call stablefx_prepare_funding.",
        }))
    }
}

// ============================================================================
// Funding presign -> wallet signature -> API-relayed funding
// ============================================================================

pub(crate) struct PrepareFunding;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PrepareFundingArgs {
    /// StableFX API trade UUID returned by stablefx_create_trade.
    pub trade_id: String,
}

impl DynAomiTool for PrepareFunding {
    type App = StableFxApp;
    type Args = PrepareFundingArgs;
    const NAME: &'static str = "stablefx_prepare_funding";
    const DESCRIPTION: &'static str = "Prepare and authorize taker funding after a StableFX trade reaches pending_settlement. Fetches Circle's exact Permit2 payload and routes it to the wallet; stablefx_fund_trade runs automatically after signing.";

    fn run_with_routes(
        _app: &StableFxApp,
        args: Self::Args,
        ctx: DynToolCallCtx,
    ) -> Result<ToolReturn, String> {
        require_arc(&ctx)?;
        validate_uuid("trade_id", &args.trade_id)?;
        let client = client(&ctx)?;
        let runtime = rt()?;
        let trade = runtime.block_on(client.trade(&args.trade_id))?;
        if trade.status != "pending_settlement" {
            return Err(format!(
                "[stablefx] trade {} is {}, not pending_settlement; poll status before funding",
                trade.id, trade.status
            ));
        }
        let contract_trade_id = trade.contract_trade_id.clone().ok_or_else(|| {
            "[stablefx] trade is missing contractTradeId; it is not ready for funding".to_string()
        })?;
        let presign = runtime.block_on(client.funding_presign(&FundingPresignRequest {
            contract_trade_ids: vec![contract_trade_id],
            trader_type: "taker".to_string(),
        }))?;
        validate_arc_typed_data(&presign.typed_data)?;
        let permit2 = typed_message(&presign.typed_data)?;

        let submit_template = json!({
            "trade_id": trade.id,
            "permit2": permit2,
            "signature": null,
        });
        let wallet_request = json!({
            "typed_data": presign.typed_data,
            "description": format!("Fund StableFX trade {} as taker", trade.id),
        });
        let preview = ok(json!({
            "status": "awaiting_funding_signature",
            "trade_id": trade.id,
            "contract_trade_id": trade.contract_trade_id,
            "deliverables": presign.deliverables,
            "receivables": presign.receivables,
            "network": "Arc Testnet",
            "chain_id": ARC_TESTNET_CHAIN_ID,
            "prerequisite": "The source token must already have sufficient ERC-20 allowance to Permit2.",
        }))?;

        Ok(ToolReturn::route(preview)
            .next(|next| {
                next.add::<host::EvmCommitMessage>(wallet_request)
                    .bind_as("signature")
                    .note("Sign Circle's funding Permit2 EIP-712 payload byte-for-byte. Do not modify the permit message.");
            })
            .after::<FundTrade>(submit_template)
            .awaits("signature")
            .note("Wallet authorized Permit2 funding — relay the signed request through Circle StableFX.")
            .build())
    }
}

pub(crate) struct FundTrade;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub(crate) struct FundTradeArgs {
    pub trade_id: String,
    /// Exact funding `typedData.message` returned by Circle.
    pub permit2: Value,
    /// Filled by the routed wallet-signing step.
    #[serde(default)]
    pub signature: Option<String>,
}

impl DynAomiTool for FundTrade {
    type App = StableFxApp;
    type Args = FundTradeArgs;
    const NAME: &'static str = "stablefx_fund_trade";
    const DESCRIPTION: &'static str = "Routed continuation of stablefx_prepare_funding. Relays the pinned Permit2 message and signature to Circle, then reads back the trade. Do not invoke directly.";

    fn run(_app: &StableFxApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        require_arc(&ctx)?;
        validate_uuid("trade_id", &args.trade_id)?;
        let signature = validate_signature(args.signature)?;
        let client = client(&ctx)?;
        let runtime = rt()?;
        runtime.block_on(client.fund(&FundRequest {
            trader_type: "taker".to_string(),
            signature,
            permit2: args.permit2,
        }))?;
        let trade = runtime.block_on(client.trade(&args.trade_id))?;
        ok(json!({
            "funding_submitted": true,
            "trade": trade,
            "next": "Poll stablefx_trade_status until taker_funded and final settlement.",
        }))
    }
}

// ============================================================================
// Status
// ============================================================================

pub(crate) struct TradeStatus;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct TradeStatusArgs {
    pub trade_id: String,
}

impl DynAomiTool for TradeStatus {
    type App = StableFxApp;
    type Args = TradeStatusArgs;
    const NAME: &'static str = "stablefx_trade_status";
    const DESCRIPTION: &'static str = "Get the current Circle StableFX trade state and settlement transaction hash. Poll at a moderate cadence; call stablefx_prepare_funding only when the state is pending_settlement.";

    fn run(_app: &StableFxApp, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        require_arc(&ctx)?;
        validate_uuid("trade_id", &args.trade_id)?;
        let runtime = rt()?;
        let trade = runtime.block_on(client(&ctx)?.trade(&args.trade_id))?;
        ok(trade)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(chain_id: Option<u64>) -> DynToolCallCtx {
        DynToolCallCtx {
            session_id: "test".to_string(),
            tool_name: "stablefx_quote".to_string(),
            call_id: "call".to_string(),
            state_attributes: chain_id
                .map(|chain_id| {
                    serde_json::from_value(json!({
                        "domain": { "evm": { "chain_id": chain_id } }
                    }))
                    .unwrap()
                })
                .unwrap_or_default(),
            secrets: Default::default(),
        }
    }

    #[test]
    fn requires_arc_thread_context() {
        assert!(require_arc(&ctx(Some(ARC_TESTNET_CHAIN_ID))).is_ok());
        assert!(require_arc(&ctx(Some(1))).is_err());
        assert!(require_arc(&ctx(None)).is_err());
    }

    #[test]
    fn resolves_api_key_from_the_account_secret_context() {
        let mut context = ctx(Some(ARC_TESTNET_CHAIN_ID));
        context
            .secrets
            .insert("STABLEFX_API_KEY".to_string(), "TEST_KEY".to_string());
        assert!(client(&context).is_ok());
    }

    #[test]
    fn validates_amounts_without_float_rounding() {
        assert_eq!(validate_amount("1000.250000").unwrap(), "1000.250000");
        assert!(validate_amount("0").is_err());
        assert!(validate_amount("1e3").is_err());
        assert!(validate_amount("1.0000001").is_err());
    }

    #[test]
    fn rejects_non_arc_typed_data() {
        let wrong_chain = json!({
            "domain": {
                "name": "Permit2",
                "chainId": 1,
                "verifyingContract": PERMIT2_ADDRESS,
            },
            "types": {},
            "primaryType": "PermitWitnessTransferFrom",
            "message": { "spender": STABLEFX_ESCROW_ADDRESS },
        });
        assert!(validate_arc_typed_data(&wrong_chain).is_err());
    }

    #[test]
    fn validates_arc_stablefx_signing_scope() {
        let mut arc = json!({
            "domain": {
                "name": "Permit2",
                "chainId": "5042002",
                "verifyingContract": PERMIT2_ADDRESS.to_ascii_lowercase(),
            },
            "types": {},
            "primaryType": "PermitWitnessTransferFrom",
            "message": { "spender": STABLEFX_ESCROW_ADDRESS },
        });
        assert!(validate_arc_typed_data(&arc).is_ok());

        arc["domain"]["verifyingContract"] = json!("0x0000000000000000000000000000000000000001");
        assert!(validate_arc_typed_data(&arc).is_err());

        arc["domain"]["verifyingContract"] = json!(PERMIT2_ADDRESS);
        arc["message"]["spender"] = json!("0x0000000000000000000000000000000000000001");
        assert!(validate_arc_typed_data(&arc).is_err());
    }
}
