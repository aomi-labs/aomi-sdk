use crate::client::*;
use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

impl DynAomiTool for CreateQuote {
    type App = DeltaApp;
    type Args = CreateQuoteArgs;
    const NAME: &'static str = "delta_create_quote";
    const DESCRIPTION: &'static str = "Create a new RFQ quote from natural language. The backend compiles the text into machine-checkable 'Local Laws' that protect against invalid fills.";

    fn run(_app: &DeltaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DeltaClient::new()?;
        let body = json!({
            "text": args.text,
            "maker_owner_id": args.maker_owner_id,
            "maker_shard": args.maker_shard,
        });
        let quote: Quote = client.post("/quotes", &body)?;
        Ok(json!({
            "quote_id": quote.id,
            "text": quote.text,
            "status": quote.status,
            "asset": quote.asset,
            "direction": quote.direction,
            "size": quote.size,
            "price_limit": quote.price_limit,
            "currency": quote.currency,
            "expires_at": quote.expires_at,
            "created_at": quote.created_at,
            "local_law": quote.local_law,
            "constraints_summary": quote.constraints_summary,
            "message": quote.message,
        }))
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
        let client = DeltaClient::new()?;
        let quotes: Vec<Quote> = client.get("/quotes")?;
        let formatted: Vec<Value> = quotes
            .iter()
            .map(|q| {
                json!({
                    "id": q.id, "text": q.text, "status": q.status,
                    "asset": q.asset, "direction": q.direction, "size": q.size,
                    "price_limit": q.price_limit, "currency": q.currency,
                    "expires_at": q.expires_at, "created_at": q.created_at,
                })
            })
            .collect();
        Ok(json!({ "quotes_count": formatted.len(), "quotes": formatted }))
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
        let client = DeltaClient::new()?;
        let quote: Quote = client.get(&format!("/quotes/{}", args.quote_id))?;
        Ok(json!({
            "id": quote.id, "text": quote.text, "status": quote.status,
            "asset": quote.asset, "direction": quote.direction, "size": quote.size,
            "price_limit": quote.price_limit, "currency": quote.currency,
            "expires_at": quote.expires_at, "local_law": quote.local_law,
        }))
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
    feed_evidence: Vec<FeedEvidenceArg>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct FeedEvidenceArg {
    /// Price feed source name
    source: String,
    /// Asset the price is for
    asset: String,
    /// Price from this feed
    price: f64,
    /// Unix timestamp of the price
    timestamp: i64,
    /// Cryptographic signature
    signature: String,
}

impl DynAomiTool for FillQuote {
    type App = DeltaApp;
    type Args = FillQuoteArgs;
    const NAME: &'static str = "delta_fill_quote";
    const DESCRIPTION: &'static str = "Attempt to fill a quote with price feed evidence. The fill will only succeed if it satisfies the quote's Local Law constraints.";

    fn run(_app: &DeltaApp, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        let client = DeltaClient::new()?;
        let body = json!({
            "taker_owner_id": args.taker_owner_id,
            "taker_shard": args.taker_shard,
            "size": args.size,
            "price": args.price,
            "feed_evidence": args.feed_evidence,
        });
        let resp: FillResponse = client.post(&format!("/quotes/{}/fill", args.quote_id), &body)?;
        if resp.success {
            Ok(json!({
                "success": true, "fill_id": resp.fill_id, "quote_id": resp.quote_id,
                "message": resp.message, "receipt": resp.receipt, "proof": resp.proof,
            }))
        } else {
            Ok(json!({
                "success": false, "fill_id": resp.fill_id, "quote_id": resp.quote_id,
                "message": resp.message, "error": resp.error,
            }))
        }
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
        let client = DeltaClient::new()?;
        let receipts: Vec<Receipt> = client.get(&format!("/quotes/{}/receipts", args.quote_id))?;
        let formatted: Vec<Value> = receipts
            .iter()
            .map(|r| {
                json!({
                    "id": r.id, "quote_id": r.quote_id, "success": r.success,
                    "status": r.status, "taker_owner_id": r.taker_owner_id,
                    "taker_shard": r.taker_shard, "size": r.size, "price": r.price,
                    "attempted_at": r.attempted_at, "error_code": r.error_code,
                    "error_message": r.error_message, "sdl_hash": r.sdl_hash,
                })
            })
            .collect();
        Ok(json!({ "receipts_count": formatted.len(), "receipts": formatted }))
    }
}
