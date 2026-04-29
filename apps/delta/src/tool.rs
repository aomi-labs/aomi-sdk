use crate::client::*;
use crate::types::{CreateQuoteRequest, FeedEvidence, FillQuoteRequest};
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn ok<T: Serialize>(value: T) -> Result<Value, String> {
    let value = serde_json::to_value(value)
        .map_err(|e| format!("[delta] failed to serialize response: {e}"))?;
    Ok(match value {
        Value::Object(mut map) => {
            map.insert("source".to_string(), Value::String("delta".to_string()));
            Value::Object(map)
        }
        other => serde_json::json!({ "source": "delta", "data": other }),
    })
}

impl DynAomiTool for CreateQuote {
    type App = DeltaApp;
    type Args = CreateQuoteArgs;
    const NAME: &'static str = "delta_create_quote";
    const DESCRIPTION: &'static str = "Create a new RFQ quote from natural language. The backend compiles the text into machine-checkable 'Local Laws' that protect against invalid fills.";

    fn run(_app: &DeltaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let body = CreateQuoteRequest {
            text: &args.text,
            maker_owner_id: &args.maker_owner_id,
            maker_shard: args.maker_shard,
        };
        let quote: Quote = DeltaClient::new()?.post("/quotes", &body)?;
        ok(quote)
    }
}

// ============================================================================
// Tool 2: ListQuotes
// ============================================================================

pub(crate) struct ListQuotes;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ListQuotesArgs {}

impl DynAomiTool for ListQuotes {
    type App = DeltaApp;
    type Args = ListQuotesArgs;
    const NAME: &'static str = "delta_list_quotes";
    const DESCRIPTION: &'static str = "List all active quotes in the Delta RFQ Arena.";

    fn run(_app: &DeltaApp, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let quotes: Vec<Quote> = DeltaClient::new()?.get("/quotes")?;
        ok(serde_json::json!({ "quotes_count": quotes.len(), "quotes": quotes }))
    }
}

// ============================================================================
// Tool 3: GetQuote
// ============================================================================

pub(crate) struct GetQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetQuoteArgs {
    /// Quote ID to retrieve
    quote_id: String,
}

impl DynAomiTool for GetQuote {
    type App = DeltaApp;
    type Args = GetQuoteArgs;
    const NAME: &'static str = "delta_get_quote";
    const DESCRIPTION: &'static str =
        "Get detailed information about a specific quote, including its compiled Local Law.";

    fn run(_app: &DeltaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let quote: Quote = DeltaClient::new()?.get(&format!("/quotes/{}", args.quote_id))?;
        ok(quote)
    }
}

// ============================================================================
// Tool 4: FillQuote
// ============================================================================

pub(crate) struct FillQuote;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FillQuoteArgs {
    /// Quote ID to fill
    quote_id: String,
    /// Taker's owner ID
    taker_owner_id: String,
    /// Taker's shard number
    taker_shard: u64,
    /// Size to fill
    size: f64,
    /// Price at which to fill
    price: f64,
    /// Price feed evidence array
    feed_evidence: Vec<FeedEvidence>,
}

impl DynAomiTool for FillQuote {
    type App = DeltaApp;
    type Args = FillQuoteArgs;
    const NAME: &'static str = "delta_fill_quote";
    const DESCRIPTION: &'static str = "Attempt to fill a quote with price feed evidence. The fill will only succeed if it satisfies the quote's Local Law constraints.";

    fn run(_app: &DeltaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let body = FillQuoteRequest {
            taker_owner_id: &args.taker_owner_id,
            taker_shard: args.taker_shard,
            size: args.size,
            price: args.price,
            feed_evidence: &args.feed_evidence,
        };
        let resp: FillResponse =
            DeltaClient::new()?.post(&format!("/quotes/{}/fill", args.quote_id), &body)?;
        ok(resp)
    }
}

// ============================================================================
// Tool 5: GetReceipts
// ============================================================================

pub(crate) struct GetReceipts;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetReceiptsArgs {
    /// Quote ID to get receipts for
    quote_id: String,
}

impl DynAomiTool for GetReceipts {
    type App = DeltaApp;
    type Args = GetReceiptsArgs;
    const NAME: &'static str = "delta_get_receipts";
    const DESCRIPTION: &'static str =
        "Get all fill receipts for a quote. Each receipt contains fill details and ZK proof.";

    fn run(_app: &DeltaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let receipts: Vec<Receipt> =
            DeltaClient::new()?.get(&format!("/quotes/{}/receipts", args.quote_id))?;
        ok(serde_json::json!({ "receipts_count": receipts.len(), "receipts": receipts }))
    }
}
