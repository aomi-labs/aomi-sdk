use aomi_sdk::schemars::JsonSchema;
use hmac::{Hmac, Mac};
use serde::{Deserialize, de::DeserializeOwned};
use sha2::Sha256;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Clone, Default)]
pub(crate) struct BinanceApp;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

// ============================================================================
// HMAC-SHA256 Signing
// ============================================================================

type HmacSha256 = Hmac<Sha256>;

/// Compute HMAC-SHA256 signature over query string, returned as a hex string.
pub(crate) fn sign(secret_key: &str, query_string: &str) -> Result<String, String> {
    let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
        .map_err(|e| format!("[binance] failed to create HMAC key: {e}"))?;
    mac.update(query_string.as_bytes());
    let result = mac.finalize();
    Ok(hex_encode(&result.into_bytes()))
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn current_timestamp_ms() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .map_err(|e| format!("[binance] failed to get timestamp: {e}"))
}

// ============================================================================
// HTTP Client
// ============================================================================

pub(crate) const SPOT_BASE_URL: &str = "https://api.binance.com/api/v3";
#[allow(dead_code)]
pub(crate) const FUTURES_BASE_URL: &str = "https://fapi.binance.com/fapi/v1";

pub(crate) struct BinanceClient {
    pub(crate) http: reqwest::blocking::Client,
}

impl BinanceClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[binance] failed to build HTTP client: {e}"))?;
        Ok(Self { http })
    }

    /// Public GET request (no auth required).
    pub(crate) fn public_get<T: DeserializeOwned>(
        &self,
        base_url: &str,
        path: &str,
        query: &str,
    ) -> Result<T, String> {
        let url = if query.is_empty() {
            format!("{base_url}{path}")
        } else {
            format!("{base_url}{path}?{query}")
        };
        let resp = self
            .http
            .get(&url)
            .send()
            .map_err(|e| format!("[binance] request failed: {e}"))?;
        Self::parse_response(resp, "public_get")
    }

    /// Signed GET request (HMAC-SHA256 auth).
    pub(crate) fn signed_get<T: DeserializeOwned>(
        &self,
        base_url: &str,
        path: &str,
        api_key: &str,
        secret_key: &str,
        query: &str,
    ) -> Result<T, String> {
        let full_query = Self::build_signed_query(secret_key, query)?;
        let url = format!("{base_url}{path}?{full_query}");
        let resp = self
            .http
            .get(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .map_err(|e| format!("[binance] signed_get failed: {e}"))?;
        Self::parse_response(resp, "signed_get")
    }

    /// Signed POST request (HMAC-SHA256 auth).
    pub(crate) fn signed_post<T: DeserializeOwned>(
        &self,
        base_url: &str,
        path: &str,
        api_key: &str,
        secret_key: &str,
        query: &str,
    ) -> Result<T, String> {
        let full_query = Self::build_signed_query(secret_key, query)?;
        let url = format!("{base_url}{path}?{full_query}");
        let resp = self
            .http
            .post(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .map_err(|e| format!("[binance] signed_post failed: {e}"))?;
        Self::parse_response(resp, "signed_post")
    }

    /// Signed DELETE request (HMAC-SHA256 auth).
    pub(crate) fn signed_delete<T: DeserializeOwned>(
        &self,
        base_url: &str,
        path: &str,
        api_key: &str,
        secret_key: &str,
        query: &str,
    ) -> Result<T, String> {
        let full_query = Self::build_signed_query(secret_key, query)?;
        let url = format!("{base_url}{path}?{full_query}");
        let resp = self
            .http
            .delete(&url)
            .header("X-MBX-APIKEY", api_key)
            .send()
            .map_err(|e| format!("[binance] signed_delete failed: {e}"))?;
        Self::parse_response(resp, "signed_delete")
    }

    // --- Helpers ---

    /// Append timestamp, compute HMAC-SHA256 signature, return the full signed query string.
    fn build_signed_query(secret_key: &str, query: &str) -> Result<String, String> {
        let timestamp = current_timestamp_ms()?;
        let query_with_ts = if query.is_empty() {
            format!("timestamp={timestamp}")
        } else {
            format!("{query}&timestamp={timestamp}")
        };
        let signature = sign(secret_key, &query_with_ts)?;
        Ok(format!("{query_with_ts}&signature={signature}"))
    }

    fn parse_response<T: DeserializeOwned>(
        resp: reqwest::blocking::Response,
        op: &str,
    ) -> Result<T, String> {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[binance] {op} failed: HTTP {status}: {text}"));
        }
        serde_json::from_str(&text)
            .map_err(|e| format!("[binance] {op} failed: could not parse response: {e}"))
    }
}

// ============================================================================
// Tool arg structs
// ============================================================================

// --- Public tools ---

pub(crate) struct GetPrice;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPriceArgs {
    /// Trading pair symbol (e.g., "BTCUSDT", "ETHUSDT"). If omitted, returns prices for all symbols.
    pub(crate) symbol: Option<String>,
}

pub(crate) struct GetDepth;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetDepthArgs {
    /// Trading pair symbol (e.g., "BTCUSDT")
    pub(crate) symbol: String,
    /// Number of price levels to return (5, 10, 20, 50, 100, 500, 1000, 5000). Default 100.
    pub(crate) limit: Option<u32>,
}

pub(crate) struct GetKlines;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetKlinesArgs {
    /// Trading pair symbol (e.g., "BTCUSDT")
    pub(crate) symbol: String,
    /// Kline/candlestick interval (e.g., "1m", "5m", "15m", "1h", "4h", "1d", "1w", "1M")
    pub(crate) interval: String,
    /// Start time in milliseconds (optional)
    pub(crate) start_time: Option<u64>,
    /// End time in milliseconds (optional)
    pub(crate) end_time: Option<u64>,
    /// Number of candles to return (default 500, max 1000)
    pub(crate) limit: Option<u32>,
}

pub(crate) struct Get24hrStats;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct Get24hrStatsArgs {
    /// Trading pair symbol (e.g., "BTCUSDT"). If omitted, returns stats for all symbols.
    pub(crate) symbol: Option<String>,
}

// --- Signed spot tools ---

pub(crate) struct PlaceOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceOrderArgs {
    /// Binance API key
    pub(crate) api_key: Option<String>,
    /// Binance secret key for request signing
    pub(crate) secret_key: Option<String>,
    /// Trading pair symbol (e.g., "BTCUSDT")
    pub(crate) symbol: String,
    /// Order side: "BUY" or "SELL"
    pub(crate) side: String,
    /// Order type: "LIMIT", "MARKET", "STOP_LOSS_LIMIT", "TAKE_PROFIT_LIMIT"
    pub(crate) order_type: String,
    /// Time in force: "GTC", "IOC", or "FOK". Required for LIMIT orders.
    pub(crate) time_in_force: Option<String>,
    /// Order quantity
    pub(crate) quantity: Option<String>,
    /// Order price (required for LIMIT orders)
    pub(crate) price: Option<String>,
}

pub(crate) struct CancelOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CancelOrderArgs {
    /// Binance API key
    pub(crate) api_key: Option<String>,
    /// Binance secret key for request signing
    pub(crate) secret_key: Option<String>,
    /// Trading pair symbol (e.g., "BTCUSDT")
    pub(crate) symbol: String,
    /// Order ID to cancel
    pub(crate) order_id: Option<u64>,
    /// Original client order ID to cancel (alternative to order_id)
    pub(crate) orig_client_order_id: Option<String>,
}

pub(crate) struct GetAccount;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAccountArgs {
    /// Binance API key
    pub(crate) api_key: Option<String>,
    /// Binance secret key for request signing
    pub(crate) secret_key: Option<String>,
}

pub(crate) struct GetTrades;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTradesArgs {
    /// Binance API key
    pub(crate) api_key: Option<String>,
    /// Binance secret key for request signing
    pub(crate) secret_key: Option<String>,
    /// Trading pair symbol (e.g., "BTCUSDT")
    pub(crate) symbol: String,
    /// Trade ID to fetch from (optional)
    pub(crate) from_id: Option<u64>,
    /// Start time in milliseconds (optional)
    pub(crate) start_time: Option<u64>,
    /// End time in milliseconds (optional)
    pub(crate) end_time: Option<u64>,
    /// Number of trades to return (default 500, max 1000)
    pub(crate) limit: Option<u32>,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Binance24hrStatsResponse, BinanceDepthResponse, BinanceKlineResponse, BinancePriceResponse,
    };

    /// Story: "Buy 2 ETH on Binance with a limit order below market"
    ///
    /// Exercises public endpoints to gather the data needed for a limit buy,
    /// then validates we could construct the order parameters — without ever
    /// hitting a signed endpoint.
    #[test]
    fn buy_eth_workflow() {
        println!("=== Buy ETH Workflow ===");
        let client = BinanceClient::new().expect("failed to create BinanceClient");
        println!("[Step 1] Fetching current ETHUSDT price...");

        // Step 1 — Get current ETH price
        let price_resp: BinancePriceResponse = client
            .public_get(SPOT_BASE_URL, "/ticker/price", "symbol=ETHUSDT")
            .expect("GET /ticker/price for ETHUSDT failed");
        let price_str = price_resp
            .first()
            .map(|ticker| ticker.price.as_str())
            .expect("price field missing or not a string");
        let price: f64 = price_str.parse().expect("could not parse price as f64");
        println!("[Step 1] ETHUSDT price: {price} (raw: \"{price_str}\")");
        assert!(price > 0.0, "ETH price must be positive, got {price}");

        // Step 2 — Get order-book depth
        println!("[Step 2] Fetching order-book depth (limit=5)...");
        let depth: BinanceDepthResponse = client
            .public_get(SPOT_BASE_URL, "/depth", "symbol=ETHUSDT&limit=5")
            .expect("GET /depth for ETHUSDT failed");
        let bids = depth.bids;
        let asks = depth.asks;
        println!(
            "[Step 2] Order book: {} bid levels, {} ask levels",
            bids.len(),
            asks.len()
        );
        for (i, bid) in bids.iter().enumerate() {
            println!(
                "[Step 2]   bid[{i}]: price={}, qty={}",
                bid.price().unwrap_or("?"),
                bid.qty().unwrap_or("?")
            );
        }
        for (i, ask) in asks.iter().enumerate() {
            println!(
                "[Step 2]   ask[{i}]: price={}, qty={}",
                ask.price().unwrap_or("?"),
                ask.qty().unwrap_or("?")
            );
        }
        assert!(!bids.is_empty(), "bids array must not be empty");
        assert!(!asks.is_empty(), "asks array must not be empty");

        // Step 3 — Compute limit price 1 % below the mid price
        let best_bid: f64 = bids[0]
            .price()
            .expect("best bid price not a string")
            .parse()
            .expect("could not parse best bid as f64");
        let best_ask: f64 = asks[0]
            .price()
            .expect("best ask price not a string")
            .parse()
            .expect("could not parse best ask as f64");
        let spread = best_ask - best_bid;
        let spread_pct = (spread / best_bid) * 100.0;
        let mid = (best_bid + best_ask) / 2.0;
        let limit_price = mid * 0.99; // 1 % below mid
        println!("[Step 3] Best bid: {best_bid}, Best ask: {best_ask}");
        println!("[Step 3] Spread: {spread:.4} ({spread_pct:.4}%)");
        println!("[Step 3] Mid price: {mid:.4}");
        println!("[Step 3] Limit price (1% below mid): {limit_price:.4}");
        assert!(
            limit_price > 0.0,
            "limit price must be positive, got {limit_price}"
        );

        // Step 4 — (Skip actual place_order — needs real API keys)
        println!("[Step 4] Skipping actual order placement (no API keys)");

        // Step 5 — Assert we have enough data to construct the order params
        let quantity = 2.0_f64;
        let order_symbol = "ETHUSDT";
        let order_side = "BUY";
        let order_type = "LIMIT";
        let time_in_force = "GTC";
        println!(
            "[Step 5] Order params: symbol={order_symbol}, side={order_side}, type={order_type}, tif={time_in_force}, qty={quantity}, price={limit_price:.2}"
        );
        assert!(
            !order_symbol.is_empty()
                && !order_side.is_empty()
                && !order_type.is_empty()
                && !time_in_force.is_empty()
                && quantity > 0.0
                && limit_price > 0.0,
            "all order params must be valid"
        );

        // Verify the signing helper works with these params
        let query = format!(
            "symbol={order_symbol}&side={order_side}&type={order_type}\
             &timeInForce={time_in_force}&quantity={quantity}&price={limit_price:.2}"
        );
        let sig = sign("test_secret_key", &query).expect("sign() failed");
        println!("[Step 5] Signed query: {query}");
        println!("[Step 5] HMAC-SHA256 signature: {sig}");
        assert_eq!(sig.len(), 64, "HMAC-SHA256 hex signature must be 64 chars");
        println!("=== Buy ETH Workflow PASSED ===");
    }

    /// Story: "Set up a 10x BTC short with stop-loss on futures"
    ///
    /// Uses only public spot endpoints (price, klines, 24hr stats) to gather
    /// market data, then validates we can derive entry, stop-loss, and
    /// position-size parameters — without calling any signed futures endpoint.
    #[test]
    fn btc_short_with_stoploss_workflow() {
        println!("=== BTC Short with Stop-Loss Workflow ===");
        let client = BinanceClient::new().expect("failed to create BinanceClient");

        // Step 1 — Get current BTC price
        println!("[Step 1] Fetching current BTCUSDT price...");
        let price_resp: BinancePriceResponse = client
            .public_get(SPOT_BASE_URL, "/ticker/price", "symbol=BTCUSDT")
            .expect("GET /ticker/price for BTCUSDT failed");
        let price_str = price_resp
            .first()
            .map(|ticker| ticker.price.as_str())
            .expect("price field missing or not a string");
        let btc_price: f64 = price_str.parse().expect("could not parse BTC price as f64");
        println!("[Step 1] BTCUSDT price: {btc_price} (raw: \"{price_str}\")");
        assert!(
            btc_price > 0.0,
            "BTC price must be positive, got {btc_price}"
        );

        // Step 2 — Get 24 hourly klines
        println!("[Step 2] Fetching 24 hourly klines for BTCUSDT...");
        let klines: BinanceKlineResponse = client
            .public_get(
                SPOT_BASE_URL,
                "/klines",
                "symbol=BTCUSDT&interval=1h&limit=24",
            )
            .expect("GET /klines for BTCUSDT failed");
        let candles = klines;
        println!("[Step 2] Received {} candles", candles.len());
        assert!(
            !candles.is_empty(),
            "klines must return at least one candle"
        );
        let first_candle = &candles[0];
        assert!(
            first_candle.len() >= 6,
            "candle must have at least 6 elements, got {}",
            first_candle.len()
        );
        println!(
            "[Step 2] First candle: open={}, high={}, low={}, close={}, volume={}",
            first_candle.open().unwrap_or("?"),
            first_candle.high().unwrap_or("?"),
            first_candle.low().unwrap_or("?"),
            first_candle.close().unwrap_or("?"),
            first_candle.volume().unwrap_or("?")
        );
        let last_candle = candles.last().expect("last candle missing");
        println!(
            "[Step 2] Last candle:  open={}, high={}, low={}, close={}, volume={}",
            last_candle.open().unwrap_or("?"),
            last_candle.high().unwrap_or("?"),
            last_candle.low().unwrap_or("?"),
            last_candle.close().unwrap_or("?"),
            last_candle.volume().unwrap_or("?")
        );

        // Step 3 — Get 24hr ticker stats
        println!("[Step 3] Fetching 24hr ticker stats for BTCUSDT...");
        let stats: Binance24hrStatsResponse = client
            .public_get(SPOT_BASE_URL, "/ticker/24hr", "symbol=BTCUSDT")
            .expect("GET /ticker/24hr for BTCUSDT failed");
        let stats = stats.first().expect("ticker stats missing");
        let volume = stats
            .volume
            .as_deref()
            .expect("volume field missing or not a string");
        let volume_f: f64 = volume.parse().expect("could not parse volume as f64");
        assert!(
            volume_f > 0.0,
            "24hr volume must be positive, got {volume_f}"
        );

        let price_change_pct = stats
            .price_change_percent
            .as_deref()
            .expect("priceChangePercent field missing or not a string");
        let _pct: f64 = price_change_pct
            .parse()
            .expect("could not parse priceChangePercent as f64");
        println!("[Step 3] 24hr volume: {volume_f} BTC");
        println!("[Step 3] 24hr price change: {price_change_pct}%");
        println!(
            "[Step 3] 24hr high: {}, low: {}",
            stats.high_price.as_deref().unwrap_or("?"),
            stats.low_price.as_deref().unwrap_or("?")
        );

        // Step 4 — (Skip actual futures order — needs real API keys)
        println!("[Step 4] Skipping actual futures order placement (no API keys)");

        // Step 5 — Derive entry, stop-loss, and position size
        let leverage = 10.0_f64;
        let entry_price = btc_price; // short at current market
        let stop_loss = entry_price * 1.02; // 2 % above entry
        let notional_usd = 10_000.0_f64; // example capital
        let position_size_btc = notional_usd / entry_price;
        let margin_required = notional_usd / leverage;

        println!("[Step 5] --- Position Parameters ---");
        println!("[Step 5] Entry price (short): {entry_price:.2}");
        println!("[Step 5] Stop-loss (2% above): {stop_loss:.2}");
        println!(
            "[Step 5] Stop-loss distance: {:.2} ({:.2}%)",
            stop_loss - entry_price,
            ((stop_loss - entry_price) / entry_price) * 100.0
        );
        println!("[Step 5] Leverage: {leverage:.0}x");
        println!("[Step 5] Notional value: ${notional_usd:.2}");
        println!("[Step 5] Position size: {position_size_btc:.5} BTC");
        println!("[Step 5] Margin required: ${margin_required:.2}");

        assert!(
            entry_price > 0.0
                && stop_loss > entry_price
                && position_size_btc > 0.0
                && margin_required > 0.0,
            "derived trading params must all be valid"
        );
        assert!(
            (stop_loss - entry_price) / entry_price > 0.019,
            "stop-loss must be at least ~2 % above entry"
        );

        // Verify signing helper works with futures-style params
        let query = format!(
            "symbol=BTCUSDT&side=SELL&type=LIMIT&timeInForce=GTC\
             &quantity={position_size_btc:.5}&price={entry_price:.2}&leverage={leverage:.0}"
        );
        let sig = sign("test_secret_key", &query).expect("sign() failed");
        println!("[Step 5] Signed query: {query}");
        println!("[Step 5] HMAC-SHA256 signature: {sig}");
        assert_eq!(sig.len(), 64, "HMAC-SHA256 hex signature must be 64 chars");
        println!("=== BTC Short with Stop-Loss Workflow PASSED ===");
    }
}
