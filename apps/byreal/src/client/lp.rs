//! byreal LP / Copy Farming client (Solana).
//!
//! The marquee surface here is **Copy Farming** — a structurally unique
//! dataset on Solana that ranks LP wallets by performance and exposes the
//! winners' tick-range strategies for replication.
//!
//! Wraps these `api2.byreal.io` endpoints:
//!   * `/dex/v2/copyfarmer/top-positions` — leaderboard
//!   * `/dex/v2/copyfarmer/providerOverview` — per-LP deep dive
//!   * `/dex/v2/copyfarmer/epoch-bonus` — current incentive epoch
//!   * `/dex/v2/position/list` — a wallet's CLMM positions
//!   * `/dex/v2/position/unclaimed-data` — pending fees + incentives
//!   * `/dex/v2/incentive/encode-v2` — get unsigned reward-claim tx
//!   * `/dex/v2/incentive/order-v2` — submit signed reward order
//!
//! Reward claims follow the same build/submit pattern as spot swaps but the
//! encode endpoint may return multiple unsigned txs (one per position cluster).
//! v1 supports the single-tx case explicitly; multi-tx batches are left for
//! the LLM to drive by claiming positions in smaller groups.

use crate::client::{BYREAL_API_BASE, byreal_get, byreal_post, http_client};
use serde_json::{Value, json};
use std::sync::OnceLock;

const PATH_POSITIONS_LIST: &str = "/byreal/api/dex/v2/position/list";
const PATH_UNCLAIMED_DATA: &str = "/byreal/api/dex/v2/position/unclaimed-data";
const PATH_TOP_POSITIONS: &str = "/byreal/api/dex/v2/copyfarmer/top-positions";
const PATH_EPOCH_BONUS: &str = "/byreal/api/dex/v2/copyfarmer/epoch-bonus";
const PATH_PROVIDER_OVERVIEW: &str = "/byreal/api/dex/v2/copyfarmer/providerOverview";
const PATH_REWARD_ENCODE: &str = "/byreal/api/dex/v2/incentive/encode-v2";
const PATH_REWARD_ORDER: &str = "/byreal/api/dex/v2/incentive/order-v2";

static LP_CLIENT: OnceLock<Result<LpClient, String>> = OnceLock::new();

pub(crate) fn lp_client() -> Result<&'static LpClient, String> {
    LP_CLIENT
        .get_or_init(LpClient::new)
        .as_ref()
        .map_err(|e| e.clone())
}

pub(crate) struct LpClient {
    http: reqwest::blocking::Client,
    base_url: String,
}

impl LpClient {
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

    #[allow(clippy::too_many_arguments)] // 1:1 mapping of byreal's query params
    pub(crate) fn list_positions(
        &self,
        user_address: &str,
        page: u32,
        page_size: u32,
        sort_field: Option<&str>,
        sort_type: Option<&str>,
        pool_address: Option<&str>,
        status: Option<&str>,
    ) -> Result<Value, String> {
        let mut q = vec![
            ("userAddress", user_address.to_string()),
            ("page", page.to_string()),
            ("pageSize", page_size.to_string()),
        ];
        if let Some(s) = sort_field {
            q.push(("sortField", s.to_string()));
        }
        if let Some(s) = sort_type {
            q.push(("sortType", s.to_string()));
        }
        if let Some(p) = pool_address {
            q.push(("poolAddress", p.to_string()));
        }
        if let Some(s) = status {
            q.push(("status", s.to_string()));
        }
        byreal_get(&self.http, &self.url(PATH_POSITIONS_LIST), &q)
    }

    pub(crate) fn get_unclaimed_data(&self, user_address: &str) -> Result<Value, String> {
        byreal_get(
            &self.http,
            &self.url(PATH_UNCLAIMED_DATA),
            &[("userAddress", user_address.to_string())],
        )
    }

    pub(crate) fn list_top_positions(
        &self,
        pool_address: Option<&str>,
        page: u32,
        page_size: u32,
        sort_field: &str,
        sort_type: &str,
        status: Option<i32>,
    ) -> Result<Value, String> {
        let mut body = json!({
            "page": page,
            "pageSize": page_size,
            "sortField": sort_field,
            "sortType": sort_type,
            "status": status.unwrap_or(0),
        });
        if let Some(addr) = pool_address {
            body["poolAddress"] = json!(addr);
        }
        byreal_post(&self.http, &self.url(PATH_TOP_POSITIONS), &body)
    }

    pub(crate) fn get_epoch_bonus(
        &self,
        wallet_address: &str,
        bonus_type: i32,
    ) -> Result<Value, String> {
        byreal_get(
            &self.http,
            &self.url(PATH_EPOCH_BONUS),
            &[
                ("walletAddress", wallet_address.to_string()),
                ("type", bonus_type.to_string()),
            ],
        )
    }

    pub(crate) fn get_provider_overview(&self, provider_address: &str) -> Result<Value, String> {
        byreal_get(
            &self.http,
            &self.url(PATH_PROVIDER_OVERVIEW),
            &[("providerAddress", provider_address.to_string())],
        )
    }

    // ---------------------------------------------------------------
    // Reward claim: encode → sign → submit
    // ---------------------------------------------------------------

    /// `/dex/v2/incentive/encode-v2` — returns `{orderCode, rewardEncodeItems: [{transaction, ...}]}`.
    pub(crate) fn encode_reward(
        &self,
        wallet_address: &str,
        position_addresses: &[String],
        bonus_type: Option<i32>,
    ) -> Result<Value, String> {
        let mut body = json!({
            "walletAddress": wallet_address,
            "positionAddresses": position_addresses,
        });
        if let Some(t) = bonus_type {
            body["type"] = json!(t);
        }
        byreal_post(&self.http, &self.url(PATH_REWARD_ENCODE), &body)
    }

    /// `/dex/v2/incentive/order-v2` — submit signed reward claim.
    pub(crate) fn submit_reward_order(
        &self,
        order_code: &str,
        wallet_address: &str,
        signed_tx_payload: Value,
    ) -> Result<Value, String> {
        let body = json!({
            "orderCode": order_code,
            "walletAddress": wallet_address,
            "signedTxPayload": signed_tx_payload,
        });
        byreal_post(&self.http, &self.url(PATH_REWARD_ORDER), &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_concatenation() {
        let c = LpClient::new().expect("client should build");
        assert!(c.url(PATH_TOP_POSITIONS).contains("api2.byreal.io"));
        assert!(c.url(PATH_REWARD_ORDER).ends_with(PATH_REWARD_ORDER));
    }
}
