//! byreal spot / CLMM / RFQ client (Solana).
//!
//! Wraps the `api2.byreal.io` endpoints documented in `byreal-cli`'s
//! `src/api/endpoints.ts`. All write tools follow the same shape:
//!
//!   1. `get_swap_quote` returns an unsigned base64 versioned tx + a
//!      `routerType` ("AMM" or "RFQ") + opaque continuation fields
//!      (quoteId/orderId).
//!   2. The `build_swap` tool routes the unsigned tx through
//!      `host::SvmStageTx` → `host::SvmCommitTx` (venue-broadcast).
//!   3. Once signed, `submit_swap` calls either [`SpotClient::execute_swap_amm`]
//!      or [`SpotClient::execute_swap_rfq`] depending on the routerType.
//!
//! The byreal envelope (retCode / retMsg / result.data) is unwrapped here;
//! tool layer sees plain JSON.

use crate::client::{BYREAL_API_BASE, byreal_get, byreal_post, http_client};
use serde_json::{Value, json};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const PATH_POOLS_LIST: &str = "/byreal/api/dex/v2/pools/info/list";
const PATH_POOL_DETAILS: &str = "/byreal/api/dex/v2/pools/details";
const PATH_POOL_KLINES: &str = "/byreal/api/dex/v2/kline/query-ui";
const PATH_TOKENS_LIST: &str = "/byreal/api/dex/v2/mint/list";
const PATH_TOKEN_PRICE: &str = "/byreal/api/dex/v2/mint/price";
const PATH_OVERVIEW_GLOBAL: &str = "/byreal/api/dex/v2/overview/global";
const PATH_AUTO_FEE: &str = "/byreal/api/dex/v2/main/auto-fee";
const PATH_SWAP_QUOTE: &str = "/byreal/api/router/v1/router-service/swap";
const PATH_SWAP_EXECUTE_AMM: &str = "/byreal/api/dex/v2/send-swap-tx";
const PATH_SWAP_EXECUTE_RFQ: &str = "/byreal/api/rfq/v1/swap";

const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
const DEFAULT_FEE_AMOUNT_LAMPORTS: &str = "10000000"; // 0.01 SOL — matches byreal-cli default
const DEFAULT_BROADCAST_MODE: &str = "priority";
const DEFAULT_FEE_TYPE: &str = "maxCap";

static SPOT_CLIENT: OnceLock<Result<SpotClient, String>> = OnceLock::new();

pub(crate) fn spot_client() -> Result<&'static SpotClient, String> {
    SPOT_CLIENT
        .get_or_init(SpotClient::new)
        .as_ref()
        .map_err(|e| e.clone())
}

pub(crate) struct SpotClient {
    http: reqwest::blocking::Client,
    base_url: String,
}

impl SpotClient {
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            http: http_client()?,
            base_url: std::env::var("BYREAL_API_URL")
                .unwrap_or_else(|_| BYREAL_API_BASE.to_string()),
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    // ---------------------------------------------------------------
    // Reads
    // ---------------------------------------------------------------

    /// `/dex/v2/pools/info/list` — paginated pool catalog with TVL/APR/vol.
    #[allow(clippy::too_many_arguments)] // 1:1 mapping of byreal's query params
    pub(crate) fn list_pools(
        &self,
        page: u32,
        page_size: u32,
        sort_field: &str,
        sort_type: &str,
        category: Option<&str>,
        status: Option<&str>,
        pool_address: Option<&str>,
    ) -> Result<Value, String> {
        let mut q = vec![
            ("page", page.to_string()),
            ("pageSize", page_size.to_string()),
            ("sortField", sort_field.to_string()),
            ("sortType", sort_type.to_string()),
        ];
        if let Some(c) = category {
            q.push(("category", c.to_string()));
        }
        if let Some(s) = status {
            q.push(("status", s.to_string()));
        }
        if let Some(a) = pool_address {
            q.push(("poolAddress", a.to_string()));
        }
        byreal_get(&self.http, &self.url(PATH_POOLS_LIST), &q)
    }

    /// `/dex/v2/pools/details` — single pool deep dive.
    pub(crate) fn get_pool(&self, pool_address: &str) -> Result<Value, String> {
        byreal_get(
            &self.http,
            &self.url(PATH_POOL_DETAILS),
            &[("poolAddress", pool_address.to_string())],
        )
    }

    /// `/dex/v2/kline/query-ui` — OHLCV per pool.
    pub(crate) fn get_klines(
        &self,
        pool_address: &str,
        kline_type: &str,
        start_time: u64,
        end_time: u64,
        token_address: Option<&str>,
    ) -> Result<Value, String> {
        let mut q = vec![
            ("poolAddress", pool_address.to_string()),
            ("klineType", kline_type.to_string()),
            ("startTime", start_time.to_string()),
            ("endTime", end_time.to_string()),
        ];
        if let Some(t) = token_address {
            q.push(("tokenAddress", t.to_string()));
        }
        byreal_get(&self.http, &self.url(PATH_POOL_KLINES), &q)
    }

    /// `/dex/v2/mint/list` — paginated token catalog.
    #[allow(clippy::too_many_arguments)] // 1:1 mapping of byreal's query params
    pub(crate) fn list_tokens(
        &self,
        page: u32,
        page_size: u32,
        sort_field: &str,
        sort: &str,
        search_key: Option<&str>,
        category: Option<&str>,
        status: Option<&str>,
    ) -> Result<Value, String> {
        let mut q = vec![
            ("page", page.to_string()),
            ("pageSize", page_size.to_string()),
            ("sortField", sort_field.to_string()),
            ("sort", sort.to_string()),
        ];
        if let Some(s) = search_key {
            q.push(("searchKey", s.to_string()));
        }
        if let Some(c) = category {
            q.push(("category", c.to_string()));
        }
        if let Some(s) = status {
            q.push(("status", s.to_string()));
        }
        byreal_get(&self.http, &self.url(PATH_TOKENS_LIST), &q)
    }

    /// `/dex/v2/mint/price` — spot prices for one or more mint addresses.
    pub(crate) fn get_token_prices(&self, mints: &[String]) -> Result<Value, String> {
        byreal_get(
            &self.http,
            &self.url(PATH_TOKEN_PRICE),
            &[("mints", mints.join(","))],
        )
    }

    /// `/dex/v2/overview/global` — DEX-wide TVL / volume / incentive stats.
    pub(crate) fn get_global_overview(&self) -> Result<Value, String> {
        byreal_get(&self.http, &self.url(PATH_OVERVIEW_GLOBAL), &[])
    }

    /// `/dex/v2/main/auto-fee` — current recommended compute-unit price for tx
    /// reliability. Used internally before getting a swap quote so the priority
    /// fee in the unsigned tx matches byreal's frontend behavior.
    pub(crate) fn get_auto_fee(&self) -> Result<Value, String> {
        byreal_get(&self.http, &self.url(PATH_AUTO_FEE), &[])
    }

    // ---------------------------------------------------------------
    // Swap: quote → sign → submit
    // ---------------------------------------------------------------

    /// `/router/v1/router-service/swap` — RFQ + AMM hybrid swap quote.
    /// Returns the unsigned base64 versioned tx in the `transaction` field
    /// alongside `routerType` and (for RFQ) `quoteId` + `orderId`.
    ///
    /// `swap_mode` is `"in"` (you specify input amount, get an output estimate)
    /// or `"out"` (you specify output target, get an input estimate).
    pub(crate) fn get_swap_quote(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: &str,
        swap_mode: &str,
        slippage_bps: u32,
        user_public_key: &str,
    ) -> Result<Value, String> {
        // Pull the recommended compute-unit price first (matches frontend).
        let cu_price = self
            .get_auto_fee()
            .ok()
            .and_then(|v| v.get("default").cloned())
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        let mut body = json!({
            "inputMint": input_mint,
            "outputMint": output_mint,
            "amount": amount,
            "swapMode": swap_mode,
            "slippageBps": slippage_bps.to_string(),
            "userPublicKey": user_public_key,
            "broadcastMode": DEFAULT_BROADCAST_MODE,
            "feeType": DEFAULT_FEE_TYPE,
            "feeAmount": DEFAULT_FEE_AMOUNT_LAMPORTS,
            "cuPrice": cu_price.to_string(),
        });
        // SOL wrapping: ask the router to add a WSOL ATA-create instruction
        // when one side of the swap is native SOL.
        if input_mint == SOL_MINT {
            body["createInputAta"] = json!(true);
        }
        if output_mint == SOL_MINT {
            body["createOutputAta"] = json!(true);
        }
        byreal_post(&self.http, &self.url(PATH_SWAP_QUOTE), &body)
    }

    /// `/dex/v2/send-swap-tx` — submit a signed AMM swap tx. The `pre_data`
    /// array is the original unsigned tx(s) returned by `get_swap_quote`; the
    /// `signed_data` array contains the corresponding signed versions.
    pub(crate) fn execute_swap_amm(
        &self,
        pre_data: &[String],
        signed_data: &[String],
    ) -> Result<Value, String> {
        let body = json!({
            "preData": pre_data,
            "data": signed_data,
            "userSignTime": now_ms(),
        });
        byreal_post(&self.http, &self.url(PATH_SWAP_EXECUTE_AMM), &body)
    }

    /// `/rfq/v1/swap` — submit a signed RFQ swap tx.
    pub(crate) fn execute_swap_rfq(
        &self,
        quote_id: &str,
        request_id: &str,
        signed_tx: &str,
    ) -> Result<Value, String> {
        let body = json!({
            "quoteId": quote_id,
            "requestId": request_id,
            "transaction": signed_tx,
        });
        byreal_post(&self.http, &self.url(PATH_SWAP_EXECUTE_RFQ), &body)
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_concatenation() {
        let c = SpotClient::new().expect("client should build");
        assert!(c.url(PATH_POOLS_LIST).ends_with(PATH_POOLS_LIST));
        assert!(c.url(PATH_SWAP_QUOTE).contains("api2.byreal.io"));
    }

    #[test]
    fn now_ms_is_recent() {
        let t = now_ms();
        // sanity: somewhere between 2025-01-01 and 2030-01-01
        assert!(t > 1_735_689_600_000 && t < 1_893_456_000_000);
    }
}
