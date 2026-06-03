use ethers::types::H160;
use hl_ranger::{
    Actions, BulkCancel, BulkOrder, CancelRequest, ClientLimit, ClientOrder, ClientOrderRequest,
    UpdateLeverage,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) const DEFAULT_API_URL: &str = "https://api.hyperliquid.xyz";
pub(crate) const ZERO_ADDRESS: &str = "0x0000000000000000000000000000000000000000";

pub(crate) const MAINNET_CHAIN_ID: u64 = 1337; // L1 agent signing chain id (constant, not real arbitrum)
pub(crate) const MAINNET_SOURCE: &str = "a";

static PERPS_CLIENT: OnceLock<Result<PerpsClient, String>> = OnceLock::new();

pub(crate) fn perps_client() -> Result<&'static PerpsClient, String> {
    PERPS_CLIENT
        .get_or_init(PerpsClient::new)
        .as_ref()
        .map_err(|e| e.clone())
}

pub(crate) struct PerpsClient {
    http: reqwest::blocking::Client,
    api_url: String,
    coin_to_asset: OnceLock<Result<HashMap<String, u32>, String>>,
}

impl PerpsClient {
    pub(crate) fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("[byreal] failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_url: std::env::var("HYPERLIQUID_API_URL")
                .unwrap_or_else(|_| DEFAULT_API_URL.to_string()),
            coin_to_asset: OnceLock::new(),
        })
    }

    pub(crate) fn coin_to_asset(&self) -> Result<&HashMap<String, u32>, String> {
        self.coin_to_asset
            .get_or_init(|| self.fetch_coin_to_asset())
            .as_ref()
            .map_err(|e| e.clone())
    }

    fn fetch_coin_to_asset(&self) -> Result<HashMap<String, u32>, String> {
        let body = json!({"type": "meta"});
        let value = self.post_info(body)?;
        let universe = value
            .get("universe")
            .and_then(Value::as_array)
            .ok_or_else(|| "[byreal] /info meta response missing 'universe'".to_string())?;
        let mut map = HashMap::new();
        for (i, asset) in universe.iter().enumerate() {
            if let Some(name) = asset.get("name").and_then(Value::as_str) {
                map.insert(name.to_string(), i as u32);
            }
        }
        if map.is_empty() {
            return Err("[byreal] /info meta returned empty universe".to_string());
        }
        Ok(map)
    }

    pub(crate) fn lookup_asset(&self, coin: &str) -> Result<u32, String> {
        let map = self.coin_to_asset()?;
        map.get(coin).copied().ok_or_else(|| {
            format!("[byreal] unknown asset '{coin}' — not found in Hyperliquid universe")
        })
    }

    pub(crate) fn post_info(&self, body: Value) -> Result<Value, String> {
        let url = format!("{}/info", self.api_url);
        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("[byreal] /info request failed: {e}"))?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[byreal] /info failed ({status}): {text}"));
        }
        serde_json::from_str::<Value>(&text)
            .map_err(|e| format!("[byreal] /info decode failed: {e}; body: {text}"))
    }

    // -------- read endpoints (all hit /info) --------

    pub(crate) fn get_meta(&self) -> Result<Value, String> {
        self.post_info(json!({"type": "meta"}))
    }

    pub(crate) fn get_all_mids(&self) -> Result<Value, String> {
        self.post_info(json!({"type": "allMids"}))
    }

    pub(crate) fn get_l2_book(&self, coin: &str) -> Result<Value, String> {
        self.post_info(json!({"type": "l2Book", "coin": coin}))
    }

    pub(crate) fn get_account_state(&self, user: &str) -> Result<Value, String> {
        self.post_info(json!({"type": "clearinghouseState", "user": user}))
    }

    pub(crate) fn get_open_orders(&self, user: &str) -> Result<Value, String> {
        self.post_info(json!({"type": "openOrders", "user": user}))
    }

    pub(crate) fn get_user_fills(&self, user: &str) -> Result<Value, String> {
        self.post_info(json!({"type": "userFills", "user": user}))
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
        self.post_info(body)
    }

    pub(crate) fn get_candles(
        &self,
        coin: &str,
        interval: &str,
        start_time: u64,
        end_time: u64,
    ) -> Result<Value, String> {
        self.post_info(json!({
            "type": "candleSnapshot",
            "req": {
                "coin": coin,
                "interval": interval,
                "startTime": start_time,
                "endTime": end_time,
            }
        }))
    }

    pub(crate) fn post_exchange(&self, body: Value) -> Result<Value, String> {
        let url = format!("{}/exchange", self.api_url);
        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("[byreal] /exchange request failed: {e}"))?;
        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[byreal] /exchange failed ({status}): {text}"));
        }
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| format!("[byreal] /exchange decode failed: {e}; body: {text}"))?;
        // Hyperliquid returns {status: "ok"|"err", response: ...}; surface "err" as Err.
        if value.get("status").and_then(Value::as_str) == Some("err") {
            let msg = value
                .get("response")
                .and_then(Value::as_str)
                .unwrap_or("(no message)");
            return Err(format!("[byreal] exchange returned error: {msg}"));
        }
        Ok(value)
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Process-wide monotonic nonce source for L1 actions.
///
/// Hyperliquid rejects two L1 actions from the same signer with the same
/// nonce. The natural choice — millisecond wall-clock — collides when two
/// orders fire from the same agent within one millisecond (rare but real
/// under LLM-driven batching like "cancel and replace"). This helper
/// returns `max(now_ms, last_issued + 1)` atomically, guaranteeing strict
/// monotonicity within the process.
///
/// State is in-process only. If multiple Aomi runtimes serve the same
/// signer concurrently they can still race; Hyperliquid will reject the
/// loser and the tool's caller can retry. Persisting the high-water mark
/// across restarts hasn't been worth it yet — the kernel-side restart
/// already advances `now_ms` past anything we'd have issued before.
static LAST_NONCE_MS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn next_nonce_ms() -> u64 {
    let now = now_ms();
    // `fetch_update` returns the *previous* value of the atomic — which on
    // the very first invocation in a fresh process is the initializer (0),
    // not the timestamp we just stored. That gave us a nonce=0 on the very
    // first order-build per process restart, which Hyperliquid then rejects
    // ("nonce too low 0 < <real-ts>"). Compute the new value explicitly and
    // return *that*; the atomic still serves as the monotonic high-water
    // mark for collision-free subsequent calls.
    let mut new_nonce = now;
    LAST_NONCE_MS
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |prev| {
            let candidate = std::cmp::max(now, prev.saturating_add(1));
            new_nonce = candidate;
            Some(candidate)
        })
        .expect("fetch_update with always-Some closure is infallible");
    new_nonce
}

// ---------------------------------------------------------------------------
// Action constructors
//
// `OrderRequest` (the wire struct), `Limit`, and `Trigger` are pub but NOT
// re-exported at the hl_ranger crate root, so external callers can't name them
// directly. We go through `ClientOrderRequest::convert(&coin_to_asset)` which
// returns an `OrderRequest` we never have to name — type inference on the
// `vec![]` macro handles it.
// ---------------------------------------------------------------------------

pub(crate) struct OrderInputs<'a> {
    pub coin: &'a str,
    pub is_buy: bool,
    pub limit_px: f64,
    pub sz: f64,
    pub reduce_only: bool,
    pub tif: String, // "Gtc" | "Ioc" | "Alo"
}

pub(crate) fn build_order_action(
    inputs: &OrderInputs<'_>,
    coin_to_asset: &HashMap<String, u32>,
) -> Result<Actions, String> {
    let client_req = ClientOrderRequest {
        asset: inputs.coin.to_string(),
        is_buy: inputs.is_buy,
        reduce_only: inputs.reduce_only,
        limit_px: inputs.limit_px,
        sz: inputs.sz,
        cloid: None,
        order_type: ClientOrder::Limit(ClientLimit {
            tif: inputs.tif.clone(),
        }),
    };
    let order_req = client_req
        .convert(coin_to_asset)
        .map_err(|e| format!("[byreal] order convert: {e}"))?;
    Ok(Actions::Order(BulkOrder {
        orders: vec![order_req],
        grouping: "na".to_string(),
        builder: None,
    }))
}

pub(crate) fn build_cancel_action(asset_index: u32, oid: u64) -> Actions {
    Actions::Cancel(BulkCancel {
        cancels: vec![CancelRequest {
            asset: asset_index,
            oid,
        }],
    })
}

pub(crate) fn build_update_leverage_action(
    asset_index: u32,
    is_cross: bool,
    leverage: u32,
) -> Actions {
    Actions::UpdateLeverage(UpdateLeverage {
        asset: asset_index,
        is_cross,
        leverage,
    })
}

// ---------------------------------------------------------------------------
// Hashing + typed-data construction
// ---------------------------------------------------------------------------

/// Compute the L1 connection_id and assemble the EIP-712 typed_data envelope
/// for the host wallet. Returns (action_json, nonce, typed_data).
pub(crate) fn prepare_l1_action(
    action: Actions,
    vault_address: Option<H160>,
) -> Result<(Value, u64, Value), String> {
    let nonce = next_nonce_ms();
    let connection_id = action
        .hash(nonce, vault_address)
        .map_err(|e| format!("[byreal] action hash failed: {e}"))?;
    let action_json =
        serde_json::to_value(&action).map_err(|e| format!("[byreal] action serialize: {e}"))?;
    let typed_data = l1_agent_typed_data(connection_id.as_bytes());
    Ok((action_json, nonce, typed_data))
}

fn l1_agent_typed_data(connection_id: &[u8]) -> Value {
    json!({
        "domain": {
            "name": "Exchange",
            "version": "1",
            "chainId": MAINNET_CHAIN_ID,
            "verifyingContract": ZERO_ADDRESS,
        },
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "version", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"},
            ],
            "Agent": [
                {"name": "source", "type": "string"},
                {"name": "connectionId", "type": "bytes32"},
            ],
        },
        "primaryType": "Agent",
        "message": {
            "source": MAINNET_SOURCE,
            "connectionId": format!("0x{}", hex::encode(connection_id)),
        },
    })
}

// ---------------------------------------------------------------------------
// Signature handling — host returns 65-byte hex; split into r/s/v.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct ExchangeSignature {
    pub r: String,
    pub s: String,
    pub v: u8,
}

pub(crate) fn parse_signature(hex_sig: &str) -> Result<ExchangeSignature, String> {
    // Defensive normalization. The LLM occasionally passes the signature with
    // surrounding whitespace, a stray quote, or with the `0x` prefix on each
    // r/s/v chunk after concatenation. Strip all that before validating.
    let cleaned: String = hex_sig
        .trim()
        .trim_matches('"')
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    let trimmed = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned);
    // Reject any non-hex characters early with a clearer error than hex::decode's
    // generic "Odd number of digits".
    if !trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "[byreal] signature has non-hex characters (len {})",
            trimmed.len()
        ));
    }
    if trimmed.len() != 130 {
        return Err(format!(
            "[byreal] signature must be 65 bytes / 130 hex chars (got {} chars)",
            trimmed.len()
        ));
    }
    let bytes =
        hex::decode(trimmed).map_err(|e| format!("[byreal] signature is not valid hex: {e}"))?;
    if bytes.len() != 65 {
        return Err(format!(
            "[byreal] signature must be 65 bytes (got {})",
            bytes.len()
        ));
    }
    let r = format!("0x{}", hex::encode(&bytes[0..32]));
    let s = format!("0x{}", hex::encode(&bytes[32..64]));
    let mut v = bytes[64];
    if v < 27 {
        v += 27;
    }
    Ok(ExchangeSignature { r, s, v })
}

/// Build the body for POST /exchange.
pub(crate) fn build_exchange_body(
    action: Value,
    nonce: u64,
    sig: &ExchangeSignature,
    vault_address: Option<&str>,
) -> Value {
    let mut body = json!({
        "action": action,
        "nonce": nonce,
        "signature": {
            "r": sig.r,
            "s": sig.s,
            "v": sig.v,
        },
    });
    if let Some(v) = vault_address {
        body.as_object_mut()
            .unwrap()
            .insert("vaultAddress".to_string(), Value::String(v.to_string()));
    }
    body
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_nonce_ms_returns_current_timestamp_on_first_call() {
        // Regression: previously returned the AtomicU64 initializer (0)
        // because fetch_update returns the *old* value.
        let n = super::next_nonce_ms();
        assert!(
            n > 1_700_000_000_000,
            "next_nonce_ms must be a recent ms timestamp, got {n}"
        );
    }

    #[test]
    fn next_nonce_ms_is_monotonic() {
        let a = super::next_nonce_ms();
        let b = super::next_nonce_ms();
        let c = super::next_nonce_ms();
        assert!(a < b && b < c, "nonces must increase: {a}, {b}, {c}");
    }

    #[test]
    fn parse_signature_splits_65_byte_hex() {
        // r=0x11..11, s=0x22..22, v=0x1c (28)
        let r_hex = "11".repeat(32);
        let s_hex = "22".repeat(32);
        let sig = format!("0x{r_hex}{s_hex}1c");
        let parsed = parse_signature(&sig).expect("valid sig");
        assert_eq!(parsed.r, format!("0x{r_hex}"));
        assert_eq!(parsed.s, format!("0x{s_hex}"));
        assert_eq!(parsed.v, 28);
    }

    #[test]
    fn parse_signature_normalizes_low_v() {
        let r_hex = "11".repeat(32);
        let s_hex = "22".repeat(32);
        let sig = format!("0x{r_hex}{s_hex}00"); // v=0
        let parsed = parse_signature(&sig).expect("valid sig");
        assert_eq!(parsed.v, 27);
    }

    #[test]
    fn parse_signature_rejects_wrong_length() {
        assert!(parse_signature("0xdead").is_err());
    }

    #[test]
    fn l1_typed_data_carries_connection_id() {
        let cid = [0xab; 32];
        let td = l1_agent_typed_data(&cid);
        assert_eq!(td["primaryType"], "Agent");
        assert_eq!(td["message"]["source"], MAINNET_SOURCE);
        assert_eq!(
            td["message"]["connectionId"],
            format!("0x{}", "ab".repeat(32))
        );
        assert_eq!(td["domain"]["chainId"], MAINNET_CHAIN_ID);
    }

    #[test]
    fn order_action_serializes_to_expected_wire_shape() {
        let mut map = HashMap::new();
        map.insert("ETH".to_string(), 4);
        let action = build_order_action(
            &OrderInputs {
                coin: "ETH",
                is_buy: true,
                limit_px: 2000.0,
                sz: 0.1,
                reduce_only: false,
                tif: "Gtc".to_string(),
            },
            &map,
        )
        .expect("convert should succeed");
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "order");
        let order = &json["orders"][0];
        assert_eq!(order["a"], 4);
        assert_eq!(order["b"], true);
        assert_eq!(order["p"], "2000");
        assert_eq!(order["s"], "0.1");
        assert_eq!(order["r"], false);
        assert_eq!(order["t"]["limit"]["tif"], "Gtc");
        assert_eq!(json["grouping"], "na");
    }

    #[test]
    fn cancel_action_serializes_to_expected_wire_shape() {
        let action = build_cancel_action(4, 12345);
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "cancel");
        assert_eq!(json["cancels"][0]["a"], 4);
        assert_eq!(json["cancels"][0]["o"], 12345);
    }

    #[test]
    fn update_leverage_action_serializes() {
        let action = build_update_leverage_action(4, true, 5);
        let json = serde_json::to_value(&action).unwrap();
        assert_eq!(json["type"], "updateLeverage");
        assert_eq!(json["asset"], 4);
        assert_eq!(json["isCross"], true);
        assert_eq!(json["leverage"], 5);
    }

    #[test]
    fn exchange_body_omits_vault_when_absent() {
        let sig = ExchangeSignature {
            r: "0x11".to_string(),
            s: "0x22".to_string(),
            v: 28,
        };
        let body = build_exchange_body(json!({"type": "cancel"}), 1234, &sig, None);
        assert_eq!(body["nonce"], 1234);
        assert_eq!(body["signature"]["v"], 28);
        assert!(body.get("vaultAddress").is_none());
    }

    #[test]
    fn exchange_body_includes_vault_when_present() {
        let sig = ExchangeSignature {
            r: "0x11".to_string(),
            s: "0x22".to_string(),
            v: 28,
        };
        let body = build_exchange_body(json!({"type": "order"}), 1234, &sig, Some("0xdead"));
        assert_eq!(body["vaultAddress"], "0xdead");
    }
}
