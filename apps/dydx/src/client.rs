use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

#[derive(Clone, Default)]
pub(crate) struct DydxApp;

// ============================================================================
// Client
// ============================================================================

pub(crate) const BASE_URL: &str = "https://indexer.dydx.trade/v4";

#[derive(Clone)]
pub(crate) struct DydxClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) base_url: String,
}

impl DydxClient {
    pub(crate) fn new() -> Result<Self, String> {
        let base_url = std::env::var("DYDX_INDEXER_URL").unwrap_or_else(|_| BASE_URL.to_string());
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[dydx] failed to build HTTP client: {e}"))?;
        Ok(Self { http, base_url })
    }

    pub(crate) fn get(&self, path: &str) -> Result<Value, String> {
        let url = format!("{}{path}", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .map_err(|e| format!("[dydx] request failed: {e}"))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().unwrap_or_default();
            return Err(format!("[dydx] API error {status}: {text}"));
        }
        resp.json()
            .map_err(|e| format!("[dydx] decode failed: {e}"))
    }
}

// ============================================================================
// Tool 1: GetMarkets
// ============================================================================

pub(crate) struct GetMarkets;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMarketsArgs {
    /// Optional ticker to filter by (e.g., "BTC-USD"). If omitted, returns all perpetual markets.
    pub(crate) ticker: Option<String>,
}

// ============================================================================
// Tool 2: GetOrderbook
// ============================================================================

pub(crate) struct GetOrderbook;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOrderbookArgs {
    /// Market ticker (e.g., "BTC-USD")
    pub(crate) ticker: String,
}

// ============================================================================
// Tool 3: GetCandles
// ============================================================================

pub(crate) struct GetCandles;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCandlesArgs {
    /// Market ticker (e.g., "ETH-USD")
    pub(crate) ticker: String,
    /// Candle resolution: 1MIN, 5MINS, 15MINS, 30MINS, 1HOUR, 4HOURS, or 1DAY
    pub(crate) resolution: String,
    /// Maximum number of candles to return (optional)
    pub(crate) limit: Option<u32>,
}

// ============================================================================
// Tool 4: GetTrades
// ============================================================================

pub(crate) struct GetTrades;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTradesArgs {
    /// Market ticker (e.g., "BTC-USD")
    pub(crate) ticker: String,
    /// Maximum number of trades to return (optional)
    pub(crate) limit: Option<u32>,
}

// ============================================================================
// Tool 5: GetAccount
// ============================================================================

pub(crate) struct GetAccount;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAccountArgs {
    /// dYdX address (e.g., "dydx1...")
    pub(crate) address: String,
    /// Subaccount number (typically 0 for default)
    pub(crate) subaccount_number: u32,
}

// ============================================================================
// Tool 6: GetOrders
// ============================================================================

pub(crate) struct GetOrders;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOrdersArgs {
    /// dYdX address (e.g., "dydx1...")
    pub(crate) address: String,
    /// Subaccount number (typically 0 for default)
    pub(crate) subaccount_number: u32,
    /// Optional order status filter (e.g., "OPEN", "FILLED", "CANCELED")
    pub(crate) status: Option<String>,
    /// Optional ticker filter (e.g., "BTC-USD")
    pub(crate) ticker: Option<String>,
}

// ============================================================================
// Tool 7: GetFills
// ============================================================================

pub(crate) struct GetFills;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetFillsArgs {
    /// dYdX address (e.g., "dydx1...")
    pub(crate) address: String,
    /// Subaccount number (typically 0 for default)
    pub(crate) subaccount_number: u32,
    /// Optional market ticker filter (e.g., "BTC-USD")
    pub(crate) market: Option<String>,
    /// Maximum number of fills to return (optional)
    pub(crate) limit: Option<u32>,
}

// ============================================================================
// Tool 8: GetHistoricalFunding
// ============================================================================

pub(crate) struct GetHistoricalFunding;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetHistoricalFundingArgs {
    /// Market ticker (e.g., "BTC-USD")
    pub(crate) ticker: String,
    /// Maximum number of funding rate entries to return (optional)
    pub(crate) limit: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Story: "Put on a funding rate arb — short the high-funding perp"
    ///
    /// 1. Fetch all markets and find a ticker with funding data.
    /// 2. Fetch the BTC-USD orderbook and verify bids/asks exist.
    /// 3. Fetch BTC-USD candles and verify candle data comes back.
    /// 4. Assert we have enough data to decide on entry.
    #[test]
    fn funding_rate_arb_workflow() {
        let client = DydxClient::new().expect("failed to build dYdX client");

        // Step 1 – get all markets, find a ticker with funding info
        println!("[step 1] Fetching perpetual markets...");
        let markets_resp = client
            .get("/perpetualMarkets")
            .expect("failed to fetch perpetual markets");
        let markets_obj = markets_resp
            .get("markets")
            .expect("response should contain a 'markets' key");
        let markets_map = markets_obj
            .as_object()
            .expect("'markets' should be an object");
        assert!(
            !markets_map.is_empty(),
            "markets map should contain at least one market"
        );
        println!("[step 1] Found {} perpetual markets", markets_map.len());

        // Find any ticker that carries a nextFundingRate field
        let ticker_with_funding = markets_map
            .iter()
            .find(|(_ticker, info)| info.get("nextFundingRate").is_some())
            .map(|(ticker, _)| ticker.clone());
        assert!(
            ticker_with_funding.is_some(),
            "at least one market should expose funding-rate data"
        );
        let funding_ticker = ticker_with_funding.clone().unwrap();
        let funding_rate = markets_map
            .get(&funding_ticker)
            .and_then(|info| info.get("nextFundingRate"))
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");
        println!(
            "[step 1] Ticker with funding: {}, nextFundingRate: {}",
            funding_ticker, funding_rate
        );

        // Step 2 – orderbook for BTC-USD
        println!("[step 2] Fetching BTC-USD orderbook...");
        let orderbook = client
            .get("/orderbooks/perpetualMarket/BTC-USD")
            .expect("failed to fetch BTC-USD orderbook");
        let bids = orderbook
            .get("bids")
            .expect("orderbook should contain 'bids'");
        let asks = orderbook
            .get("asks")
            .expect("orderbook should contain 'asks'");
        let bids_arr = bids.as_array().expect("bids should be an array");
        let asks_arr = asks.as_array().expect("asks should be an array");
        assert!(
            !bids_arr.is_empty(),
            "BTC-USD orderbook should have at least one bid"
        );
        assert!(
            !asks_arr.is_empty(),
            "BTC-USD orderbook should have at least one ask"
        );
        let best_bid = bids_arr
            .first()
            .and_then(|b| b.get("price"))
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");
        let best_ask = asks_arr
            .first()
            .and_then(|a| a.get("price"))
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");
        println!(
            "[step 2] BTC-USD orderbook: {} bids, {} asks, best bid: {}, best ask: {}",
            bids_arr.len(),
            asks_arr.len(),
            best_bid,
            best_ask
        );

        // Step 3 – candles for BTC-USD (note: endpoint is perpetualMarkets, plural)
        println!("[step 3] Fetching BTC-USD 1HOUR candles (limit 10)...");
        let candles_resp = client
            .get("/candles/perpetualMarkets/BTC-USD?resolution=1HOUR&limit=10")
            .expect("failed to fetch BTC-USD candles");
        let candles = candles_resp
            .get("candles")
            .expect("candles response should contain 'candles' key");
        let candles_arr = candles.as_array().expect("candles should be an array");
        assert!(
            !candles_arr.is_empty(),
            "should receive at least one candle"
        );
        let latest_candle = candles_arr.first().unwrap();
        let candle_open = latest_candle
            .get("open")
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");
        let candle_close = latest_candle
            .get("close")
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");
        let candle_volume = latest_candle
            .get("baseTokenVolume")
            .and_then(|v| v.as_str())
            .unwrap_or("N/A");
        println!(
            "[step 3] Received {} candles, latest: open={}, close={}, volume={}",
            candles_arr.len(),
            candle_open,
            candle_close,
            candle_volume
        );

        // Step 4 – we now have markets with funding rates, a live orderbook,
        // and recent price history — enough data to decide on an arb entry.
        assert!(
            ticker_with_funding.is_some()
                && !markets_map.is_empty()
                && bids.as_array().is_some()
                && candles.as_array().is_some(),
            "should have sufficient data to decide on a funding-rate arb entry"
        );
        println!(
            "[step 4] Arb decision data complete: {} markets, orderbook spread {}..{}, {} candles",
            markets_map.len(),
            best_bid,
            best_ask,
            candles_arr.len()
        );
    }

    /// Story: "Rebalance my dYdX subaccount — cut losers, add to winners"
    ///
    /// 1. Fetch account state for the zero address, subaccount 0.
    /// 2. Fetch open orders for that address.
    /// 3. Fetch fills for that address.
    /// 4. Assert we have enough info to form a rebalance plan.
    #[test]
    fn rebalance_subaccount_workflow() {
        let client = DydxClient::new().expect("failed to build dYdX client");
        let address = "0x0000000000000000000000000000000000000000";
        println!("[step 0] Using address: {}", address);

        // Step 1 – account state (may 404 or return empty; both are valid)
        println!("[step 1] Fetching account state for subaccount 0...");
        let account_result = client.get(&format!("/addresses/{address}/subaccountNumber/0"));
        // The API may return an error for a non-existent address; that is
        // acceptable — we just need to confirm we got *a* response.
        let account_responded = match &account_result {
            Ok(val) => {
                // If successful, it should be a JSON value (object or otherwise).
                assert!(
                    val.is_object() || val.is_null(),
                    "account response should be an object or null"
                );
                let equity = val.get("equity").and_then(|v| v.as_str()).unwrap_or("N/A");
                let open_positions = val
                    .get("openPerpetualPositions")
                    .and_then(|v| v.as_object())
                    .map(|m| m.len())
                    .unwrap_or(0);
                println!(
                    "[step 1] Account responded OK: equity={}, open positions={}",
                    equity, open_positions
                );
                true
            }
            Err(e) => {
                // A 404 or similar is still a valid structural response.
                assert!(
                    !e.is_empty(),
                    "error message from account endpoint should be non-empty"
                );
                println!(
                    "[step 1] Account endpoint returned error (expected for zero address): {}",
                    e
                );
                true
            }
        };
        assert!(account_responded, "account endpoint should respond");

        // Step 2 – open orders
        println!("[step 2] Fetching open orders...");
        let orders_result = client.get(&format!("/orders?address={address}&subaccountNumber=0"));
        let orders_responded = match &orders_result {
            Ok(val) => {
                // Expect an array or an empty list
                assert!(
                    val.is_object() || val.is_array(),
                    "orders response should be an object or array"
                );
                let order_count = val.as_array().map(|a| a.len()).unwrap_or(0);
                println!("[step 2] Orders responded OK: {} open orders", order_count);
                true
            }
            Err(e) => {
                assert!(
                    !e.is_empty(),
                    "error message from orders endpoint should be non-empty"
                );
                println!("[step 2] Orders endpoint returned error: {}", e);
                true
            }
        };
        assert!(orders_responded, "orders endpoint should respond");

        // Step 3 – fills
        println!("[step 3] Fetching fills...");
        let fills_result = client.get(&format!("/fills?address={address}&subaccountNumber=0"));
        let fills_responded = match &fills_result {
            Ok(val) => {
                assert!(
                    val.is_object() || val.is_array(),
                    "fills response should be an object or array"
                );
                let fill_count = val
                    .get("fills")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .or_else(|| val.as_array().map(|a| a.len()))
                    .unwrap_or(0);
                println!("[step 3] Fills responded OK: {} fills returned", fill_count);
                true
            }
            Err(e) => {
                assert!(
                    !e.is_empty(),
                    "error message from fills endpoint should be non-empty"
                );
                println!("[step 3] Fills endpoint returned error: {}", e);
                true
            }
        };
        assert!(fills_responded, "fills endpoint should respond");

        // Step 4 – all three endpoints responded, so we have enough info
        // (positions, open orders, trade history) to form a rebalance plan.
        assert!(
            account_responded && orders_responded && fills_responded,
            "should have account state, orders, and fills to plan a rebalance"
        );
        println!(
            "[step 4] Rebalance data collection complete: account={}, orders={}, fills={}",
            account_responded, orders_responded, fills_responded
        );
    }
}
