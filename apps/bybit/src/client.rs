use aomi_sdk::schemars::JsonSchema;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::Sha256;
use std::time::Duration;

use crate::types::BybitResponse;

#[derive(Clone, Default)]
pub(crate) struct BybitApp;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

// ============================================================================
// Client
// ============================================================================

pub(crate) const BASE_URL: &str = "https://api.bybit.com/v5";
const RECV_WINDOW: &str = "5000";

type HmacSha256 = Hmac<Sha256>;

pub(crate) struct BybitClient {
    pub(crate) http: reqwest::blocking::Client,
}

impl BybitClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[bybit] failed to build HTTP client: {e}"))?;
        Ok(Self { http })
    }

    /// Generate HMAC-SHA256 signature for Bybit V5 API.
    pub(crate) fn sign(timestamp: &str, api_key: &str, secret_key: &str, params: &str) -> String {
        let sign_str = format!("{timestamp}{api_key}{RECV_WINDOW}{params}");
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(sign_str.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Public GET (no auth).
    pub(crate) fn public_get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &str,
    ) -> Result<BybitResponse<T>, String> {
        let url = if query.is_empty() {
            format!("{BASE_URL}{path}")
        } else {
            format!("{BASE_URL}{path}?{query}")
        };
        let resp = self
            .http
            .get(&url)
            .send()
            .map_err(|e| format!("[bybit] request failed: {e}"))?;
        Self::handle_response(resp)
    }

    /// Authenticated GET.
    pub(crate) fn auth_get<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &str,
        api_key: &str,
        secret_key: &str,
    ) -> Result<BybitResponse<T>, String> {
        let timestamp = Self::timestamp_ms();
        let signature = Self::sign(&timestamp, api_key, secret_key, query);
        let url = if query.is_empty() {
            format!("{BASE_URL}{path}")
        } else {
            format!("{BASE_URL}{path}?{query}")
        };
        let resp = self
            .http
            .get(&url)
            .header("X-BAPI-API-KEY", api_key)
            .header("X-BAPI-TIMESTAMP", &timestamp)
            .header("X-BAPI-SIGN", &signature)
            .header("X-BAPI-RECV-WINDOW", RECV_WINDOW)
            .send()
            .map_err(|e| format!("[bybit] request failed: {e}"))?;
        Self::handle_response(resp)
    }

    /// Authenticated POST with JSON body.
    pub(crate) fn auth_post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
        api_key: &str,
        secret_key: &str,
    ) -> Result<BybitResponse<T>, String> {
        let timestamp = Self::timestamp_ms();
        let body_str = serde_json::to_string(body)
            .map_err(|e| format!("[bybit] failed to serialize body: {e}"))?;
        let signature = Self::sign(&timestamp, api_key, secret_key, &body_str);
        let url = format!("{BASE_URL}{path}");
        let resp = self
            .http
            .post(&url)
            .header("X-BAPI-API-KEY", api_key)
            .header("X-BAPI-TIMESTAMP", &timestamp)
            .header("X-BAPI-SIGN", &signature)
            .header("X-BAPI-RECV-WINDOW", RECV_WINDOW)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .map_err(|e| format!("[bybit] request failed: {e}"))?;
        Self::handle_response(resp)
    }

    fn timestamp_ms() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before UNIX epoch")
            .as_millis()
            .to_string()
    }

    fn handle_response<T: DeserializeOwned>(
        resp: reqwest::blocking::Response,
    ) -> Result<BybitResponse<T>, String> {
        let status = resp.status();
        let text = resp
            .text()
            .map_err(|e| format!("[bybit] failed to read response body: {e}"))?;
        if !status.is_success() {
            return Err(format!("[bybit] API HTTP error {status}: {text}"));
        }
        let val: BybitResponse<T> =
            serde_json::from_str(&text).map_err(|e| format!("[bybit] JSON decode failed: {e}"))?;
        // Bybit returns retCode != 0 for logical errors even on HTTP 200
        if val.ret_code != 0 {
            return Err(format!(
                "[bybit] API error (retCode={}): {}",
                val.ret_code, val.ret_msg
            ));
        }
        Ok(val)
    }
}

// We need hex encoding for HMAC output but don't want another dep.
// Inline a tiny hex encoder.
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes.as_ref().iter().fold(String::new(), |mut acc, b| {
            use std::fmt::Write;
            let _ = write!(acc, "{b:02x}");
            acc
        })
    }
}

// ============================================================================
// Arg structs
// ============================================================================

// --- Public endpoints ---

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTickersArgs {
    /// Product category: "spot", "linear", "inverse", or "option"
    pub(crate) category: String,
    /// Trading pair symbol (e.g. "BTCUSDT"). Optional — omit to get all tickers for the category.
    pub(crate) symbol: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOrderbookArgs {
    /// Product category: "spot", "linear", "inverse", or "option"
    pub(crate) category: String,
    /// Trading pair symbol (e.g. "BTCUSDT")
    pub(crate) symbol: String,
    /// Depth limit (e.g. 1, 25, 50, 200). Optional.
    pub(crate) limit: Option<u32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetKlineArgs {
    /// Product category: "spot", "linear", "inverse", or "option"
    pub(crate) category: String,
    /// Trading pair symbol (e.g. "BTCUSDT")
    pub(crate) symbol: String,
    /// Kline interval: 1,3,5,15,30,60,120,240,360,720,D,M,W
    pub(crate) interval: String,
    /// Start timestamp in milliseconds. Optional.
    pub(crate) start: Option<u64>,
    /// End timestamp in milliseconds. Optional.
    pub(crate) end: Option<u64>,
}

// --- Authenticated endpoints ---

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CreateOrderArgs {
    /// Bybit API key
    pub(crate) api_key: Option<String>,
    /// Bybit API secret
    pub(crate) secret_key: Option<String>,
    /// Product category: "spot", "linear", "inverse", or "option"
    pub(crate) category: String,
    /// Trading pair symbol (e.g. "BTCUSDT")
    pub(crate) symbol: String,
    /// Order side: "Buy" or "Sell"
    pub(crate) side: String,
    /// Order type: "Limit" or "Market"
    pub(crate) order_type: String,
    /// Order quantity (string)
    pub(crate) qty: String,
    /// Order price (string). Required for Limit orders.
    pub(crate) price: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CancelOrderArgs {
    /// Bybit API key
    pub(crate) api_key: Option<String>,
    /// Bybit API secret
    pub(crate) secret_key: Option<String>,
    /// Product category: "spot", "linear", "inverse", or "option"
    pub(crate) category: String,
    /// Trading pair symbol (e.g. "BTCUSDT")
    pub(crate) symbol: String,
    /// Order ID to cancel
    pub(crate) order_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AmendOrderArgs {
    /// Bybit API key
    pub(crate) api_key: Option<String>,
    /// Bybit API secret
    pub(crate) secret_key: Option<String>,
    /// Product category: "spot", "linear", "inverse", or "option"
    pub(crate) category: String,
    /// Trading pair symbol (e.g. "BTCUSDT")
    pub(crate) symbol: String,
    /// Order ID to amend
    pub(crate) order_id: String,
    /// New quantity (string). Optional.
    pub(crate) qty: Option<String>,
    /// New price (string). Optional.
    pub(crate) price: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPositionsArgs {
    /// Bybit API key
    pub(crate) api_key: Option<String>,
    /// Bybit API secret
    pub(crate) secret_key: Option<String>,
    /// Product category: "linear" or "inverse"
    pub(crate) category: String,
    /// Trading pair symbol (e.g. "BTCUSDT"). Optional.
    pub(crate) symbol: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetWalletBalanceArgs {
    /// Bybit API key
    pub(crate) api_key: Option<String>,
    /// Bybit API secret
    pub(crate) secret_key: Option<String>,
    /// Account type: "UNIFIED" or "CONTRACT"
    pub(crate) account_type: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SetLeverageArgs {
    /// Bybit API key
    pub(crate) api_key: Option<String>,
    /// Bybit API secret
    pub(crate) secret_key: Option<String>,
    /// Product category: "linear" or "inverse"
    pub(crate) category: String,
    /// Trading pair symbol (e.g. "BTCUSDT")
    pub(crate) symbol: String,
    /// Buy leverage (string, e.g. "10")
    pub(crate) buy_leverage: String,
    /// Sell leverage (string, e.g. "10")
    pub(crate) sell_leverage: String,
}

// ============================================================================
// Tool structs (implementations in tool.rs)
// ============================================================================

pub(crate) struct GetTickers;
pub(crate) struct GetOrderbook;
pub(crate) struct GetKline;
pub(crate) struct CreateOrder;
pub(crate) struct CancelOrder;
pub(crate) struct AmendOrder;
pub(crate) struct GetPositions;
pub(crate) struct GetWalletBalance;
pub(crate) struct SetLeverage;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BybitKlineResult, BybitOrderbookResult, BybitResponse, BybitTickerResult};

    fn client() -> BybitClient {
        BybitClient::new().expect("client should build")
    }

    /// Scalp ETH on Bybit — open, set TP/SL, close when done.
    #[test]
    fn scalp_eth_workflow() {
        let c = client();
        println!("=== scalp_eth_workflow ===");

        // 1. Get tickers for ETHUSDT linear — assert we get price data
        println!("[step 1] Fetching ETHUSDT linear ticker...");
        let tickers: BybitResponse<BybitTickerResult> = c
            .public_get("/market/tickers", "category=linear&symbol=ETHUSDT")
            .expect("should get ETHUSDT ticker");
        let ticker_list = tickers.result.list;
        assert!(!ticker_list.is_empty(), "should have at least one ticker");
        let last_price = ticker_list[0]
            .last_price
            .as_deref()
            .expect("ticker should have lastPrice");
        println!("[step 1] ETHUSDT lastPrice (raw): {last_price}");
        let last_price: f64 = last_price.parse().expect("lastPrice should parse as f64");
        println!("[step 1] ETHUSDT lastPrice (f64): {last_price:.2}");
        assert!(last_price > 0.0, "lastPrice should be positive");

        // 2. Get orderbook for ETHUSDT linear — assert bids/asks exist
        println!("[step 2] Fetching ETHUSDT linear orderbook (depth=25)...");
        let book: BybitResponse<BybitOrderbookResult> = c
            .public_get(
                "/market/orderbook",
                "category=linear&symbol=ETHUSDT&limit=25",
            )
            .expect("should get ETHUSDT orderbook");
        let bids = book.result.b;
        let asks = book.result.a;
        assert!(!bids.is_empty(), "bids should not be empty");
        assert!(!asks.is_empty(), "asks should not be empty");
        println!(
            "[step 2] Orderbook: {} bid levels, {} ask levels",
            bids.len(),
            asks.len()
        );
        for (i, level) in bids.iter().take(3).enumerate() {
            println!(
                "[step 2]   bid[{i}]: price={}, qty={}",
                level.price().unwrap_or("?"),
                level.qty().unwrap_or("?")
            );
        }
        for (i, level) in asks.iter().take(3).enumerate() {
            println!(
                "[step 2]   ask[{i}]: price={}, qty={}",
                level.price().unwrap_or("?"),
                level.qty().unwrap_or("?")
            );
        }

        // 3. Get kline for ETHUSDT linear 5min — assert we get candles
        println!("[step 3] Fetching ETHUSDT linear 5m kline...");
        let kline: BybitResponse<BybitKlineResult> = c
            .public_get("/market/kline", "category=linear&symbol=ETHUSDT&interval=5")
            .expect("should get ETHUSDT 5m kline");
        let candles = kline.result.list;
        assert!(!candles.is_empty(), "should have at least one candle");
        println!("[step 3] Received {} candles", candles.len());
        for (i, candle) in candles.iter().take(3).enumerate() {
            println!(
                "[step 3]   candle[{i}]: ts={}, open={}, high={}, low={}, close={}, vol={}",
                candle.open_time().unwrap_or("?"),
                candle.open().unwrap_or("?"),
                candle.high().unwrap_or("?"),
                candle.low().unwrap_or("?"),
                candle.close().unwrap_or("?"),
                candle.volume().unwrap_or("?")
            );
        }

        // 4. Assert we have enough data to pick entry, TP, and SL levels
        println!("[step 4] Computing entry, TP, and SL levels...");
        let best_bid: f64 = bids[0]
            .price()
            .expect("bid price")
            .parse()
            .expect("bid price f64");
        let best_ask: f64 = asks[0]
            .price()
            .expect("ask price")
            .parse()
            .expect("ask price f64");
        assert!(best_ask > best_bid, "ask should be above bid");
        println!(
            "[step 4] Best bid: {best_bid:.2}, best ask: {best_ask:.2}, spread: {:.2}",
            best_ask - best_bid
        );

        let entry = (best_bid + best_ask) / 2.0;
        let tp = entry * 1.01; // +1% take-profit
        let sl = entry * 0.99; // -1% stop-loss
        println!("[step 4] Entry (mid): {entry:.2}");
        println!("[step 4] TP (+1%):    {tp:.2}");
        println!("[step 4] SL (-1%):    {sl:.2}");
        assert!(tp > entry, "TP should be above entry");
        assert!(sl < entry, "SL should be below entry");
        assert!(sl > 0.0, "SL should be positive");
        println!("=== scalp_eth_workflow PASSED ===");
    }

    /// Move my stop-loss to breakeven on all profitable positions.
    #[test]
    fn move_stoploss_workflow() {
        let c = client();
        println!("=== move_stoploss_workflow ===");

        // 1. Get tickers for linear (no symbol filter) — assert multiple tickers
        println!("[step 1] Fetching all linear tickers...");
        let tickers: BybitResponse<BybitTickerResult> = c
            .public_get("/market/tickers", "category=linear")
            .expect("should get linear tickers");
        let ticker_list = tickers.result.list;
        assert!(ticker_list.len() > 1, "should have multiple linear tickers");
        println!("[step 1] Received {} linear tickers", ticker_list.len());
        for (i, t) in ticker_list.iter().take(5).enumerate() {
            println!(
                "[step 1]   ticker[{i}]: symbol={}, lastPrice={}, volume24h={}",
                t.symbol,
                t.last_price.as_deref().unwrap_or("?"),
                t.volume24h.as_deref().unwrap_or("?")
            );
        }

        // 2. Get orderbook for BTCUSDT linear — assert spread data
        println!("[step 2] Fetching BTCUSDT linear orderbook (depth=25)...");
        let book: BybitResponse<BybitOrderbookResult> = c
            .public_get(
                "/market/orderbook",
                "category=linear&symbol=BTCUSDT&limit=25",
            )
            .expect("should get BTCUSDT orderbook");
        let bids = book.result.b;
        let asks = book.result.a;
        assert!(!bids.is_empty(), "bids should not be empty");
        assert!(!asks.is_empty(), "asks should not be empty");
        let best_bid: f64 = bids[0]
            .price()
            .expect("bid price")
            .parse()
            .expect("bid price f64");
        let best_ask: f64 = asks[0]
            .price()
            .expect("ask price")
            .parse()
            .expect("ask price f64");
        let spread = best_ask - best_bid;
        assert!(spread >= 0.0, "spread should be non-negative");
        println!(
            "[step 2] Orderbook: {} bid levels, {} ask levels",
            bids.len(),
            asks.len()
        );
        println!("[step 2] Best bid: {best_bid:.2}, best ask: {best_ask:.2}, spread: {spread:.2}");
        for (i, level) in bids.iter().take(3).enumerate() {
            println!(
                "[step 2]   bid[{i}]: price={}, qty={}",
                level.price().unwrap_or("?"),
                level.qty().unwrap_or("?")
            );
        }
        for (i, level) in asks.iter().take(3).enumerate() {
            println!(
                "[step 2]   ask[{i}]: price={}, qty={}",
                level.price().unwrap_or("?"),
                level.qty().unwrap_or("?")
            );
        }

        // 3. Get kline for BTCUSDT linear 15min — assert candle data
        println!("[step 3] Fetching BTCUSDT linear 15m kline...");
        let kline: BybitResponse<BybitKlineResult> = c
            .public_get(
                "/market/kline",
                "category=linear&symbol=BTCUSDT&interval=15",
            )
            .expect("should get BTCUSDT 15m kline");
        let candles = kline.result.list;
        assert!(!candles.is_empty(), "should have at least one candle");
        println!("[step 3] Received {} candles", candles.len());
        for (i, candle) in candles.iter().take(3).enumerate() {
            println!(
                "[step 3]   candle[{i}]: ts={}, open={}, high={}, low={}, close={}, vol={}",
                candle.open_time().unwrap_or("?"),
                candle.open().unwrap_or("?"),
                candle.high().unwrap_or("?"),
                candle.low().unwrap_or("?"),
                candle.close().unwrap_or("?"),
                candle.volume().unwrap_or("?")
            );
        }

        // 4. Assert we can compute breakeven levels from market data.
        //    Simulate: entry was at the open of the latest candle, current price
        //    is the last traded price. If profitable, breakeven = entry.
        println!("[step 4] Computing breakeven levels...");
        let latest_candle = &candles[0];
        let open: f64 = latest_candle
            .open()
            .expect("candle open")
            .parse()
            .expect("candle open f64");
        let current: f64 = (best_bid + best_ask) / 2.0;
        let breakeven = open; // entry price is the breakeven level
        assert!(breakeven > 0.0, "breakeven should be positive");
        // Verify we can determine profit/loss direction
        let pnl = current - open;
        let _is_profitable = pnl > 0.0;
        println!("[step 4] Latest candle open (simulated entry): {open:.2}");
        println!("[step 4] Current mid price: {current:.2}");
        println!("[step 4] Breakeven level: {breakeven:.2}");
        println!(
            "[step 4] Simulated PnL: {pnl:.2} (profitable: {})",
            pnl > 0.0
        );
        // Whether profitable or not, we confirmed we can compute the levels
        assert!(
            current > 0.0 && open > 0.0,
            "both current and open prices should be positive"
        );
        println!("=== move_stoploss_workflow PASSED ===");
    }
}
