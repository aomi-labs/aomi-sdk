use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

#[allow(unused_imports)]
pub(crate) use crate::tool::*;

#[derive(Clone, Default)]
pub(crate) struct HyperliquidApp;

// ============================================================================
// Hyperliquid Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_API_URL: &str = "https://api.hyperliquid.xyz";

#[derive(Clone)]
pub(crate) struct HyperliquidClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_url: String,
}

impl HyperliquidClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[hyperliquid] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_url: std::env::var("HYPERLIQUID_API_URL")
                .unwrap_or_else(|_| DEFAULT_API_URL.to_string()),
        })
    }

    pub(crate) fn post_info(&self, body: Value) -> Result<Value, String> {
        let url = format!("{}/info", self.api_url);
        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("[hyperliquid] request failed: {e}"))?;

        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[hyperliquid] request failed ({status}): {text}"));
        }

        serde_json::from_str::<Value>(&text)
            .map_err(|e| format!("[hyperliquid] decode failed: {e}; body: {text}"))
    }

    pub(crate) fn with_source(value: Value) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert(
                    "source".to_string(),
                    Value::String("hyperliquid".to_string()),
                );
                Value::Object(map)
            }
            other => json!({
                "source": "hyperliquid",
                "data": other,
            }),
        }
    }

    pub(crate) fn get_meta(&self) -> Result<Value, String> {
        let body = json!({"type": "meta"});
        let value = self.post_info(body)?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_all_mids(&self) -> Result<Value, String> {
        let body = json!({"type": "allMids"});
        let value = self.post_info(body)?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_l2_book(&self, coin: &str) -> Result<Value, String> {
        let body = json!({"type": "l2Book", "coin": coin});
        let value = self.post_info(body)?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_clearinghouse_state(&self, user: &str) -> Result<Value, String> {
        let body = json!({"type": "clearinghouseState", "user": user});
        let value = self.post_info(body)?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_open_orders(&self, user: &str) -> Result<Value, String> {
        let body = json!({"type": "openOrders", "user": user});
        let value = self.post_info(body)?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_user_fills(&self, user: &str) -> Result<Value, String> {
        let body = json!({"type": "userFills", "user": user});
        let value = self.post_info(body)?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_funding_history(
        &self,
        coin: &str,
        start_time: u64,
        end_time: Option<u64>,
    ) -> Result<Value, String> {
        let mut body = json!({
            "type": "fundingHistory",
            "coin": coin,
            "startTime": start_time,
        });
        if let Some(et) = end_time {
            body.as_object_mut()
                .unwrap()
                .insert("endTime".to_string(), json!(et));
        }
        let value = self.post_info(body)?;
        Ok(Self::with_source(value))
    }

    pub(crate) fn get_candle_snapshot(
        &self,
        coin: &str,
        interval: &str,
        start_time: u64,
        end_time: u64,
    ) -> Result<Value, String> {
        let body = json!({
            "type": "candleSnapshot",
            "req": {
                "coin": coin,
                "interval": interval,
                "startTime": start_time,
                "endTime": end_time,
            }
        });
        let value = self.post_info(body)?;
        Ok(Self::with_source(value))
    }
}

// ============================================================================
// Tool structs
// ============================================================================

pub(crate) struct GetMeta;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetMetaArgs {}

pub(crate) struct GetAllMids;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetAllMidsArgs {}

pub(crate) struct GetL2Book;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetL2BookArgs {
    /// Asset ticker (e.g., "BTC", "ETH", "SOL")
    pub(crate) coin: String,
}

pub(crate) struct GetClearinghouseState;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetClearinghouseStateArgs {
    /// Ethereum-style address (0x...)
    pub(crate) user: String,
}

pub(crate) struct GetOpenOrders;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetOpenOrdersArgs {
    /// Ethereum-style address (0x...)
    pub(crate) user: String,
}

pub(crate) struct GetUserFills;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetUserFillsArgs {
    /// Ethereum-style address (0x...)
    pub(crate) user: String,
}

pub(crate) struct GetFundingHistory;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetFundingHistoryArgs {
    /// Asset ticker (e.g., "BTC", "ETH")
    pub(crate) coin: String,
    /// Start time in milliseconds (Unix epoch)
    pub(crate) start_time: u64,
    /// End time in milliseconds (Unix epoch). Optional -- defaults to now.
    #[serde(default)]
    pub(crate) end_time: Option<u64>,
}

pub(crate) struct GetCandleSnapshot;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetCandleSnapshotArgs {
    /// Asset ticker (e.g., "BTC", "ETH")
    pub(crate) coin: String,
    /// Candle interval: "1m", "5m", "15m", "1h", "4h", "1d"
    pub(crate) interval: String,
    /// Start time in milliseconds (Unix epoch)
    pub(crate) start_time: u64,
    /// End time in milliseconds (Unix epoch)
    pub(crate) end_time: u64,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn client() -> HyperliquidClient {
        HyperliquidClient::new().expect("client should build")
    }

    /// Story: "Open a 5x long on ETH-PERP"
    ///
    /// Walks through the read-only steps needed before placing a leveraged
    /// order: fetch exchange metadata, check mid-prices, and inspect the
    /// order book.  Actual order placement is skipped because this app is
    /// read-only (no signing key).
    #[test]
    fn open_leveraged_long_workflow() {
        let c = client();
        println!("[step 1] Fetching exchange metadata...");

        // Step 1 – get exchange metadata and find ETH in the asset universe
        let meta = c.get_meta().expect("get_meta should succeed");
        assert_eq!(
            meta.get("source").and_then(Value::as_str),
            Some("hyperliquid"),
            "response should carry the hyperliquid source tag"
        );
        let universe = meta
            .get("universe")
            .and_then(Value::as_array)
            .expect("meta should contain a 'universe' array");
        assert!(!universe.is_empty(), "universe should not be empty");
        println!("[step 1] Found {} assets in universe", universe.len());

        // Print the first few asset names for context
        let sample_names: Vec<&str> = universe
            .iter()
            .take(5)
            .filter_map(|a| a.get("name").and_then(Value::as_str))
            .collect();
        println!("[step 1] First few assets: {:?}", sample_names);

        let eth_asset = universe
            .iter()
            .find(|a| a.get("name").and_then(Value::as_str) == Some("ETH"))
            .expect("ETH should be listed in the universe");
        assert!(
            eth_asset.get("szDecimals").is_some(),
            "ETH asset should have szDecimals"
        );
        let sz_decimals = eth_asset.get("szDecimals").unwrap();
        println!("[step 1] ETH szDecimals: {}", sz_decimals);

        // Step 2 – fetch all mid-prices and confirm ETH has one
        println!("[step 2] Fetching all mid-prices...");
        let mids = c.get_all_mids().expect("get_all_mids should succeed");
        assert_eq!(
            mids.get("source").and_then(Value::as_str),
            Some("hyperliquid")
        );

        // Count how many mid-prices we got
        let mid_count = mids
            .as_object()
            .map(|m| m.len().saturating_sub(1)) // subtract 1 for 'source' key
            .unwrap_or(0);
        println!("[step 2] Received mid-prices for {} assets", mid_count);

        let eth_mid = mids
            .get("ETH")
            .and_then(Value::as_str)
            .expect("ETH should have a mid price");
        let eth_mid_price: f64 = eth_mid
            .parse()
            .expect("ETH mid price should be a valid number");
        assert!(
            eth_mid_price > 0.0,
            "ETH mid price should be positive, got {eth_mid_price}"
        );
        println!("[step 2] ETH mid price: ${:.2}", eth_mid_price);

        // Also show BTC mid if available
        if let Some(btc_mid) = mids.get("BTC").and_then(Value::as_str) {
            println!("[step 2] BTC mid price: ${}", btc_mid);
        }

        // Step 3 – inspect the L2 order book for ETH
        println!("[step 3] Fetching L2 order book for ETH...");
        let book = c
            .get_l2_book("ETH")
            .expect("get_l2_book(ETH) should succeed");
        assert_eq!(
            book.get("source").and_then(Value::as_str),
            Some("hyperliquid")
        );
        let levels = book
            .get("levels")
            .and_then(Value::as_array)
            .expect("book should contain 'levels'");
        assert!(
            levels.len() >= 2,
            "levels should have at least bids and asks arrays"
        );
        let bids = levels[0]
            .as_array()
            .expect("first level should be bids array");
        let asks = levels[1]
            .as_array()
            .expect("second level should be asks array");
        assert!(!bids.is_empty(), "bids should not be empty");
        assert!(!asks.is_empty(), "asks should not be empty");
        println!(
            "[step 3] Order book depth: {} bid levels, {} ask levels",
            bids.len(),
            asks.len()
        );

        // Sanity-check that the spread is reasonable (< 1% of mid)
        let best_bid: f64 = bids[0]
            .get("px")
            .and_then(Value::as_str)
            .expect("bid should have px")
            .parse()
            .expect("bid px should be a number");
        let best_ask: f64 = asks[0]
            .get("px")
            .and_then(Value::as_str)
            .expect("ask should have px")
            .parse()
            .expect("ask px should be a number");
        let spread_pct = (best_ask - best_bid) / eth_mid_price * 100.0;
        println!(
            "[step 3] Best bid: ${:.2}, Best ask: ${:.2}, Spread: {:.4}%",
            best_bid, best_ask, spread_pct
        );
        assert!(
            spread_pct < 1.0,
            "spread should be < 1% of mid, got {spread_pct:.4}%"
        );

        // Print top-of-book sizes
        if let Some(bid_sz) = bids[0].get("sz").and_then(Value::as_str) {
            println!("[step 3] Best bid size: {} ETH", bid_sz);
        }
        if let Some(ask_sz) = asks[0].get("sz").and_then(Value::as_str) {
            println!("[step 3] Best ask size: {} ETH", ask_sz);
        }

        // Step 4 – we now have everything needed to construct a limit order:
        //   • asset index from universe
        //   • current mid price for reference
        //   • best bid/ask for limit price selection
        //   • szDecimals for size rounding
        // (Actual placement is skipped — no signing key in read-only mode.)
        println!("[step 4] Order construction summary:");
        println!("[step 4]   ETH mid price: ${:.2}", eth_mid_price);
        println!(
            "[step 4]   Best bid: ${:.2}, Best ask: ${:.2}",
            best_bid, best_ask
        );
        println!("[step 4]   szDecimals: {}", sz_decimals);
        println!("[step 4]   Spread: {:.4}%", spread_pct);
        println!(
            "[step 4] All read-only data gathered. Skipping order placement (no signing key)."
        );
        assert!(
            eth_mid_price > 0.0 && best_bid > 0.0 && best_ask > 0.0,
            "all pricing data required for order construction is available"
        );
    }

    /// Story: "Close my losing positions and cancel stale orders"
    ///
    /// Queries the clearinghouse, open orders, and user fills for an address
    /// to verify the response structures are correct.  Uses the zero address
    /// which will return empty-but-valid results.
    #[test]
    fn close_positions_workflow() {
        let c = client();
        let zero_addr = "0x0000000000000000000000000000000000000000";
        println!("[step 1] Fetching clearinghouse state for {}...", zero_addr);

        // Step 1 – fetch clearinghouse state for the zero address
        let ch = c
            .get_clearinghouse_state(zero_addr)
            .expect("get_clearinghouse_state should succeed for zero address");
        assert_eq!(
            ch.get("source").and_then(Value::as_str),
            Some("hyperliquid"),
            "clearinghouse response should carry hyperliquid source tag"
        );
        // The response should contain margin summary and asset positions,
        // even if they are zeroed out.
        assert!(
            ch.get("marginSummary").is_some(),
            "clearinghouse state should include marginSummary"
        );
        assert!(
            ch.get("assetPositions").is_some(),
            "clearinghouse state should include assetPositions"
        );

        let margin_summary = &ch["marginSummary"];
        println!(
            "[step 1] Margin summary - accountValue: {}, totalMarginUsed: {}",
            margin_summary
                .get("accountValue")
                .and_then(Value::as_str)
                .unwrap_or("N/A"),
            margin_summary
                .get("totalMarginUsed")
                .and_then(Value::as_str)
                .unwrap_or("N/A")
        );
        let positions = ch.get("assetPositions").and_then(Value::as_array);
        let position_count = positions.map(|p| p.len()).unwrap_or(0);
        println!("[step 1] Found {} open positions", position_count);
        if let Some(positions) = positions {
            for pos in positions {
                if let Some(item) = pos.get("position") {
                    let coin = item.get("coin").and_then(Value::as_str).unwrap_or("?");
                    let size = item.get("szi").and_then(Value::as_str).unwrap_or("0");
                    let entry_px = item.get("entryPx").and_then(Value::as_str).unwrap_or("?");
                    let unrealized_pnl = item
                        .get("unrealizedPnl")
                        .and_then(Value::as_str)
                        .unwrap_or("0");
                    println!(
                        "[step 1]   Position: {} size={} entryPx={} unrealizedPnl={}",
                        coin, size, entry_px, unrealized_pnl
                    );
                }
            }
        }

        // Step 2 – fetch open orders
        println!("[step 2] Fetching open orders for {}...", zero_addr);
        let orders = c
            .get_open_orders(zero_addr)
            .expect("get_open_orders should succeed for zero address");
        assert_eq!(
            orders.get("source").and_then(Value::as_str),
            Some("hyperliquid")
        );
        // For the zero address, data should be an empty array
        let orders_data = orders
            .get("data")
            .and_then(Value::as_array)
            .expect("open orders response should wrap an array in 'data'");
        println!("[step 2] Found {} open orders", orders_data.len());
        for order in orders_data {
            let coin = order.get("coin").and_then(Value::as_str).unwrap_or("?");
            let side = order.get("side").and_then(Value::as_str).unwrap_or("?");
            let px = order.get("limitPx").and_then(Value::as_str).unwrap_or("?");
            let sz = order.get("sz").and_then(Value::as_str).unwrap_or("?");
            println!("[step 2]   Order: {} {} {} @ ${}", coin, side, sz, px);
        }
        assert!(
            orders_data.is_empty(),
            "zero address should have no open orders"
        );

        // Step 3 – fetch user fills
        println!("[step 3] Fetching user fills for {}...", zero_addr);
        let fills = c
            .get_user_fills(zero_addr)
            .expect("get_user_fills should succeed for zero address");
        assert_eq!(
            fills.get("source").and_then(Value::as_str),
            Some("hyperliquid")
        );
        let fills_data = fills
            .get("data")
            .and_then(Value::as_array)
            .expect("user fills response should wrap an array in 'data'");
        println!("[step 3] Found {} fills in history", fills_data.len());
        // The zero address may have settlement fills; we just verify the
        // array exists and every entry (if any) has the expected shape.
        for (i, fill) in fills_data.iter().enumerate() {
            assert!(
                fill.get("coin").is_some() && fill.get("px").is_some(),
                "each fill should have at least 'coin' and 'px' fields"
            );
            if i < 3 {
                let coin = fill.get("coin").and_then(Value::as_str).unwrap_or("?");
                let px = fill.get("px").and_then(Value::as_str).unwrap_or("?");
                let sz = fill.get("sz").and_then(Value::as_str).unwrap_or("?");
                let side = fill.get("side").and_then(Value::as_str).unwrap_or("?");
                println!(
                    "[step 3]   Fill {}: {} {} {} @ ${}",
                    i + 1,
                    coin,
                    side,
                    sz,
                    px
                );
            }
        }
        if fills_data.len() > 3 {
            println!("[step 3]   ... and {} more fills", fills_data.len() - 3);
        }

        // Step 4 – we now have the information needed to identify which
        // positions to close (assetPositions), which orders to cancel
        // (openOrders), and recent execution history (userFills).
        println!("[step 4] Close-positions summary:");
        println!("[step 4]   Positions to close: {}", position_count);
        println!("[step 4]   Orders to cancel: {}", orders_data.len());
        println!("[step 4]   Recent fills reviewed: {}", fills_data.len());
        println!(
            "[step 4] All read-only data gathered. Skipping actual close/cancel (no signing key)."
        );
        assert!(
            ch.get("assetPositions").is_some() && orders.get("data").is_some(),
            "all data required to plan position-closing is available"
        );
    }
}
