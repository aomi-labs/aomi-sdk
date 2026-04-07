use aomi_sdk::schemars::JsonSchema;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct OkxApp;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

// ============================================================================
// Client
// ============================================================================

pub(crate) const BASE_URL: &str = "https://www.okx.com/api/v5";

type HmacSha256 = Hmac<Sha256>;

pub(crate) struct OkxClient {
    pub(crate) http: reqwest::blocking::Client,
}

impl OkxClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[okx] failed to build HTTP client: {e}"))?;
        Ok(Self { http })
    }

    /// Generate OKX API signature: HMAC-SHA256(timestamp + method + requestPath + body), base64-encoded.
    fn sign(
        secret_key: &str,
        timestamp: &str,
        method: &str,
        request_path: &str,
        body: &str,
    ) -> Result<String, String> {
        let prehash = format!("{timestamp}{method}{request_path}{body}");
        let mut mac = HmacSha256::new_from_slice(secret_key.as_bytes())
            .map_err(|e| format!("[okx] HMAC key error: {e}"))?;
        mac.update(prehash.as_bytes());
        let result = mac.finalize();
        Ok(BASE64.encode(result.into_bytes()))
    }

    fn iso_timestamp() -> String {
        // OKX expects ISO 8601 timestamp e.g. 2024-01-01T00:00:00.000Z
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let millis = now.subsec_millis();
        // Convert to rough ISO format
        let days_since_epoch = secs / 86400;
        let time_of_day = secs % 86400;
        let hours = time_of_day / 3600;
        let minutes = (time_of_day % 3600) / 60;
        let seconds = time_of_day % 60;

        // Calculate year/month/day from days since epoch (1970-01-01)
        let (year, month, day) = days_to_ymd(days_since_epoch);
        format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}.{millis:03}Z")
    }

    /// Public GET request (no auth).
    pub(crate) fn public_get(&self, path: &str) -> Result<Value, String> {
        let url = format!("{BASE_URL}{path}");
        let resp = self
            .http
            .get(&url)
            .send()
            .map_err(|e| format!("[okx] request failed: {e}"))?;
        Self::parse_response(resp)
    }

    /// Authenticated GET request.
    pub(crate) fn auth_get(
        &self,
        path: &str,
        api_key: &str,
        secret_key: &str,
        passphrase: &str,
    ) -> Result<Value, String> {
        let timestamp = Self::iso_timestamp();
        let sign = Self::sign(secret_key, &timestamp, "GET", path, "")?;
        let url = format!("{BASE_URL}{path}");
        let resp = self
            .http
            .get(&url)
            .header("OK-ACCESS-KEY", api_key)
            .header("OK-ACCESS-SIGN", sign)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .send()
            .map_err(|e| format!("[okx] request failed: {e}"))?;
        Self::parse_response(resp)
    }

    /// Authenticated POST request.
    pub(crate) fn auth_post(
        &self,
        path: &str,
        body: &Value,
        api_key: &str,
        secret_key: &str,
        passphrase: &str,
    ) -> Result<Value, String> {
        let timestamp = Self::iso_timestamp();
        let body_str = serde_json::to_string(body)
            .map_err(|e| format!("[okx] failed to serialize body: {e}"))?;
        let sign = Self::sign(secret_key, &timestamp, "POST", path, &body_str)?;
        let url = format!("{BASE_URL}{path}");
        let resp = self
            .http
            .post(&url)
            .header("OK-ACCESS-KEY", api_key)
            .header("OK-ACCESS-SIGN", sign)
            .header("OK-ACCESS-TIMESTAMP", &timestamp)
            .header("OK-ACCESS-PASSPHRASE", passphrase)
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .map_err(|e| format!("[okx] request failed: {e}"))?;
        Self::parse_response(resp)
    }

    fn parse_response(resp: reqwest::blocking::Response) -> Result<Value, String> {
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[okx] HTTP {status}: {text}"));
        }
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| format!("[okx] failed to parse response: {e}"))?;
        // OKX returns { "code": "0", "msg": "", "data": [...] } on success
        let code = parsed.get("code").and_then(|c| c.as_str()).unwrap_or("");
        if code != "0" {
            let msg = parsed
                .get("msg")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(format!("[okx] API error (code {code}): {msg}"));
        }
        Ok(parsed)
    }
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ============================================================================
// Tool arg structs
// ============================================================================

// Tool 1: GetTickers
pub(crate) struct GetTickers;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetTickersArgs {
    /// Instrument type: SPOT, SWAP, FUTURES, or OPTION
    pub(crate) inst_type: String,
}

// Tool 2: GetOrderBook
pub(crate) struct GetOrderBook;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOrderBookArgs {
    /// Instrument ID, e.g. BTC-USDT, BTC-USDT-SWAP
    pub(crate) inst_id: String,
    /// Order book depth (max 400). Default 1.
    pub(crate) sz: Option<String>,
}

// Tool 3: GetCandles
pub(crate) struct GetCandles;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCandlesArgs {
    /// Instrument ID, e.g. BTC-USDT
    pub(crate) inst_id: String,
    /// Bar size, e.g. 1m, 5m, 15m, 30m, 1H, 4H, 1D, 1W, 1M
    pub(crate) bar: Option<String>,
    /// Pagination: return records newer than this timestamp (ms)
    pub(crate) after: Option<String>,
    /// Pagination: return records older than this timestamp (ms)
    pub(crate) before: Option<String>,
    /// Number of results (max 300, default 100)
    pub(crate) limit: Option<String>,
}

// Tool 4: PlaceOrder
pub(crate) struct PlaceOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PlaceOrderArgs {
    /// OKX API key
    pub(crate) api_key: Option<String>,
    /// OKX API secret key
    pub(crate) secret_key: Option<String>,
    /// OKX API passphrase
    pub(crate) passphrase: Option<String>,
    /// Instrument ID, e.g. BTC-USDT
    pub(crate) inst_id: String,
    /// Trade mode: cash, cross, or isolated
    pub(crate) td_mode: String,
    /// Order side: buy or sell
    pub(crate) side: String,
    /// Order type: market, limit, post_only, fok, ioc
    pub(crate) ord_type: String,
    /// Quantity to trade
    pub(crate) sz: String,
    /// Price (required for limit orders)
    pub(crate) px: Option<String>,
}

// Tool 5: CancelOrder
pub(crate) struct CancelOrder;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CancelOrderArgs {
    /// OKX API key
    pub(crate) api_key: Option<String>,
    /// OKX API secret key
    pub(crate) secret_key: Option<String>,
    /// OKX API passphrase
    pub(crate) passphrase: Option<String>,
    /// Instrument ID, e.g. BTC-USDT
    pub(crate) inst_id: String,
    /// Order ID to cancel
    pub(crate) ord_id: String,
}

// Tool 6: GetBalance
pub(crate) struct GetBalance;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetBalanceArgs {
    /// OKX API key
    pub(crate) api_key: Option<String>,
    /// OKX API secret key
    pub(crate) secret_key: Option<String>,
    /// OKX API passphrase
    pub(crate) passphrase: Option<String>,
    /// Optional comma-separated currency list, e.g. BTC,USDT
    pub(crate) ccy: Option<String>,
}

// Tool 7: GetPositions
pub(crate) struct GetPositions;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetPositionsArgs {
    /// OKX API key
    pub(crate) api_key: Option<String>,
    /// OKX API secret key
    pub(crate) secret_key: Option<String>,
    /// OKX API passphrase
    pub(crate) passphrase: Option<String>,
    /// Instrument type: SPOT, SWAP, FUTURES, OPTION (optional)
    pub(crate) inst_type: Option<String>,
    /// Instrument ID (optional)
    pub(crate) inst_id: Option<String>,
}

// Tool 8: SetLeverage
pub(crate) struct SetLeverage;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SetLeverageArgs {
    /// OKX API key
    pub(crate) api_key: Option<String>,
    /// OKX API secret key
    pub(crate) secret_key: Option<String>,
    /// OKX API passphrase
    pub(crate) passphrase: Option<String>,
    /// Instrument ID, e.g. BTC-USDT-SWAP
    pub(crate) inst_id: String,
    /// Leverage ratio, e.g. "10"
    pub(crate) lever: String,
    /// Margin mode: cross or isolated
    pub(crate) mgn_mode: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Story: "Sell spot ETH and go short on the SWAP"
    ///
    /// We fetch SPOT and SWAP tickers to find the instruments we need,
    /// then pull the order book so we can compare prices before executing.
    #[test]
    fn spot_to_swap_workflow() {
        let client = OkxClient::new().expect("failed to create OkxClient");
        println!("[spot_to_swap] OkxClient created successfully");

        // Step 1 — Get SPOT tickers and locate BTC-USDT
        println!("[spot_to_swap] Step 1: Fetching SPOT tickers...");
        let spot_resp = client
            .public_get("/market/tickers?instType=SPOT")
            .expect("failed to fetch SPOT tickers");
        let spot_data = spot_resp["data"]
            .as_array()
            .expect("SPOT tickers data should be an array");
        println!(
            "[spot_to_swap] Received {} SPOT instruments",
            spot_data.len()
        );
        assert!(!spot_data.is_empty(), "SPOT tickers should not be empty");
        let btc_spot = spot_data
            .iter()
            .find(|t| t["instId"].as_str() == Some("BTC-USDT"))
            .expect("BTC-USDT should exist in SPOT tickers");
        println!(
            "[spot_to_swap] Found BTC-USDT spot ticker: last={}, vol24h={}",
            btc_spot["last"].as_str().unwrap_or("N/A"),
            btc_spot["vol24h"].as_str().unwrap_or("N/A")
        );
        assert!(
            btc_spot["last"].as_str().is_some(),
            "BTC-USDT spot ticker should have a last price"
        );

        // Step 2 — Get SWAP tickers and locate BTC-USDT-SWAP
        println!("[spot_to_swap] Step 2: Fetching SWAP tickers...");
        let swap_resp = client
            .public_get("/market/tickers?instType=SWAP")
            .expect("failed to fetch SWAP tickers");
        let swap_data = swap_resp["data"]
            .as_array()
            .expect("SWAP tickers data should be an array");
        println!(
            "[spot_to_swap] Received {} SWAP instruments",
            swap_data.len()
        );
        assert!(!swap_data.is_empty(), "SWAP tickers should not be empty");
        let btc_swap = swap_data
            .iter()
            .find(|t| t["instId"].as_str() == Some("BTC-USDT-SWAP"))
            .expect("BTC-USDT-SWAP should exist in SWAP tickers");
        println!(
            "[spot_to_swap] Found BTC-USDT-SWAP ticker: last={}, vol24h={}",
            btc_swap["last"].as_str().unwrap_or("N/A"),
            btc_swap["vol24h"].as_str().unwrap_or("N/A")
        );
        assert!(
            btc_swap["last"].as_str().is_some(),
            "BTC-USDT-SWAP ticker should have a last price"
        );

        // Step 3 — Get order book for BTC-USDT to check liquidity
        println!("[spot_to_swap] Step 3: Fetching BTC-USDT order book...");
        let book_resp = client
            .public_get("/market/books?instId=BTC-USDT")
            .expect("failed to fetch BTC-USDT order book");
        let book_data = book_resp["data"]
            .as_array()
            .expect("order book data should be an array");
        assert!(!book_data.is_empty(), "order book data should not be empty");
        let book = &book_data[0];
        let bids = book["bids"]
            .as_array()
            .expect("order book should have bids");
        let asks = book["asks"]
            .as_array()
            .expect("order book should have asks");
        println!(
            "[spot_to_swap] Order book: {} bid levels, {} ask levels",
            bids.len(),
            asks.len()
        );
        if let Some(best_bid) = bids.first() {
            println!(
                "[spot_to_swap] Best bid: price={}, size={}",
                best_bid[0].as_str().unwrap_or("N/A"),
                best_bid[1].as_str().unwrap_or("N/A")
            );
        }
        if let Some(best_ask) = asks.first() {
            println!(
                "[spot_to_swap] Best ask: price={}, size={}",
                best_ask[0].as_str().unwrap_or("N/A"),
                best_ask[1].as_str().unwrap_or("N/A")
            );
        }
        assert!(!bids.is_empty(), "bids should not be empty");
        assert!(!asks.is_empty(), "asks should not be empty");

        // Step 4 — We now have both spot and swap prices to compare
        println!("[spot_to_swap] Step 4: Comparing spot vs swap prices...");
        let spot_price = btc_spot["last"].as_str().expect("spot last price missing");
        let swap_price = btc_swap["last"].as_str().expect("swap last price missing");
        let spot_f: f64 = spot_price.parse().expect("spot price should parse as f64");
        let swap_f: f64 = swap_price.parse().expect("swap price should parse as f64");
        let spread = swap_f - spot_f;
        let spread_pct = (spread / spot_f) * 100.0;
        println!("[spot_to_swap] BTC spot price: {spot_f:.2}");
        println!("[spot_to_swap] BTC swap price: {swap_f:.2}");
        println!("[spot_to_swap] Spread: {spread:.2} ({spread_pct:.4}%)");
        assert!(spot_f > 0.0, "spot price should be positive");
        assert!(swap_f > 0.0, "swap price should be positive");
        println!("[spot_to_swap] Workflow complete -- spot and swap data verified");
    }

    /// Story: "Tighten leverage on all my open positions to reduce liquidation risk"
    ///
    /// We gather market data across swap instruments — tickers, candles, and
    /// the order book — so we can evaluate position risk before adjusting leverage.
    #[test]
    fn tighten_leverage_workflow() {
        let client = OkxClient::new().expect("failed to create OkxClient");
        println!("[tighten_leverage] OkxClient created successfully");

        // Step 1 — Get SWAP tickers; expect multiple instruments
        println!("[tighten_leverage] Step 1: Fetching SWAP tickers...");
        let swap_resp = client
            .public_get("/market/tickers?instType=SWAP")
            .expect("failed to fetch SWAP tickers");
        let swap_data = swap_resp["data"]
            .as_array()
            .expect("SWAP tickers data should be an array");
        println!(
            "[tighten_leverage] Received {} SWAP instruments",
            swap_data.len()
        );
        // Print a few sample instruments
        for ticker in swap_data.iter().take(5) {
            println!(
                "[tighten_leverage]   {} last={}",
                ticker["instId"].as_str().unwrap_or("N/A"),
                ticker["last"].as_str().unwrap_or("N/A")
            );
        }
        assert!(
            swap_data.len() > 1,
            "should have multiple SWAP instruments, got {}",
            swap_data.len()
        );

        // Step 2 — Get candles for BTC-USDT-SWAP to assess recent volatility
        println!("[tighten_leverage] Step 2: Fetching BTC-USDT-SWAP candles...");
        let candle_resp = client
            .public_get("/market/candles?instId=BTC-USDT-SWAP")
            .expect("failed to fetch BTC-USDT-SWAP candles");
        let candle_data = candle_resp["data"]
            .as_array()
            .expect("candle data should be an array");
        println!("[tighten_leverage] Received {} candles", candle_data.len());
        assert!(
            !candle_data.is_empty(),
            "should have at least one candle for BTC-USDT-SWAP"
        );
        // Each candle is an array: [ts, o, h, l, c, vol, volCcy, volCcyQuote, confirm]
        let first_candle = candle_data[0]
            .as_array()
            .expect("each candle should be an array");
        println!(
            "[tighten_leverage] First candle has {} fields",
            first_candle.len()
        );
        println!(
            "[tighten_leverage] First candle: ts={}, open={}, high={}, low={}, close={}",
            first_candle
                .first()
                .map(|v| v.as_str().unwrap_or("N/A"))
                .unwrap_or("N/A"),
            first_candle
                .get(1)
                .map(|v| v.as_str().unwrap_or("N/A"))
                .unwrap_or("N/A"),
            first_candle
                .get(2)
                .map(|v| v.as_str().unwrap_or("N/A"))
                .unwrap_or("N/A"),
            first_candle
                .get(3)
                .map(|v| v.as_str().unwrap_or("N/A"))
                .unwrap_or("N/A"),
            first_candle
                .get(4)
                .map(|v| v.as_str().unwrap_or("N/A"))
                .unwrap_or("N/A")
        );
        if let Some(last_candle) = candle_data.last().and_then(|c| c.as_array()) {
            println!(
                "[tighten_leverage] Last candle: ts={}, open={}, high={}, low={}, close={}",
                last_candle
                    .first()
                    .map(|v| v.as_str().unwrap_or("N/A"))
                    .unwrap_or("N/A"),
                last_candle
                    .get(1)
                    .map(|v| v.as_str().unwrap_or("N/A"))
                    .unwrap_or("N/A"),
                last_candle
                    .get(2)
                    .map(|v| v.as_str().unwrap_or("N/A"))
                    .unwrap_or("N/A"),
                last_candle
                    .get(3)
                    .map(|v| v.as_str().unwrap_or("N/A"))
                    .unwrap_or("N/A"),
                last_candle
                    .get(4)
                    .map(|v| v.as_str().unwrap_or("N/A"))
                    .unwrap_or("N/A")
            );
        }
        assert!(
            first_candle.len() >= 5,
            "candle should have at least 5 elements (ts, o, h, l, c)"
        );

        // Step 3 — Get order book for ETH-USDT-SWAP to check liquidity
        println!("[tighten_leverage] Step 3: Fetching ETH-USDT-SWAP order book...");
        let book_resp = client
            .public_get("/market/books?instId=ETH-USDT-SWAP")
            .expect("failed to fetch ETH-USDT-SWAP order book");
        let book_data = book_resp["data"]
            .as_array()
            .expect("order book data should be an array");
        assert!(
            !book_data.is_empty(),
            "ETH-USDT-SWAP order book should not be empty"
        );
        let book = &book_data[0];
        let bids = book["bids"]
            .as_array()
            .expect("order book should have bids");
        let asks = book["asks"]
            .as_array()
            .expect("order book should have asks");
        println!(
            "[tighten_leverage] ETH-USDT-SWAP book: {} bid levels, {} ask levels",
            bids.len(),
            asks.len()
        );
        if let Some(best_bid) = bids.first() {
            println!(
                "[tighten_leverage] Best bid: price={}, size={}",
                best_bid[0].as_str().unwrap_or("N/A"),
                best_bid[1].as_str().unwrap_or("N/A")
            );
        }
        if let Some(best_ask) = asks.first() {
            println!(
                "[tighten_leverage] Best ask: price={}, size={}",
                best_ask[0].as_str().unwrap_or("N/A"),
                best_ask[1].as_str().unwrap_or("N/A")
            );
        }
        assert!(!bids.is_empty(), "ETH-USDT-SWAP bids should not be empty");
        assert!(!asks.is_empty(), "ETH-USDT-SWAP asks should not be empty");

        // Step 4 — Confirm we have all the market data needed for risk evaluation
        println!("[tighten_leverage] Step 4: Verifying all market data collected");
        println!(
            "[tighten_leverage] Summary: {} swap instruments, {} candles, {} bid levels, {} ask levels",
            swap_data.len(),
            candle_data.len(),
            bids.len(),
            asks.len()
        );
        assert!(
            swap_data.len() > 1 && !candle_data.is_empty() && !bids.is_empty() && !asks.is_empty(),
            "should have swap tickers, candle history, and order book depth to evaluate position risk"
        );
        println!("[tighten_leverage] Workflow complete -- all risk data verified");
    }
}
