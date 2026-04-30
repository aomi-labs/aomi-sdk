use crate::types::{BalanceParams, RpcRequest, TransferTransaction};
use aomi_sdk::schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct PelagosApp;

const DEFAULT_URL: &str = "http://localhost:8080";
const URL_ENV: &str = "PELAGOS_RPC_URL";

#[derive(Clone)]
pub(crate) struct PelagosClient {
    http: reqwest::blocking::Client,
    pub(crate) base_url: String,
}

impl PelagosClient {
    pub(crate) fn new(override_url: Option<&str>) -> Result<Self, String> {
        let base_url = {
            let raw = override_url
                .map(str::to_string)
                .or_else(|| std::env::var(URL_ENV).ok())
                .unwrap_or_else(|| DEFAULT_URL.to_string());
            let trimmed = raw.trim_end_matches('/').to_string();
            if trimmed.is_empty() {
                return Err("PELAGOS_RPC_URL cannot be empty".to_string());
            }
            trimmed
        };

        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;

        Ok(Self { http, base_url })
    }

    pub(crate) fn health(&self) -> Result<Value, String> {
        let url = format!("{}/health", self.base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .map_err(|e| format!("health check failed: {e}"))?;

        let status = resp.status();
        let body = resp.text().unwrap_or_default();

        if !status.is_success() {
            return Err(format!("health endpoint returned {status}: {body}"));
        }

        Ok(serde_json::from_str::<Value>(&body).unwrap_or_else(|_| json!({ "raw": body })))
    }

    fn rpc<T: Serialize>(&self, method: &str, params: T) -> Result<Value, String> {
        let url = format!("{}/rpc", self.base_url);
        let body = RpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method,
            params,
        };

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| format!("RPC {method} failed: {e}"))?;

        let status = resp.status();
        let text = resp.text().unwrap_or_default();

        if !status.is_success() {
            return Err(format!("RPC {method} HTTP {status}: {text}"));
        }

        let decoded: Value =
            serde_json::from_str(&text).map_err(|e| format!("RPC {method} decode error: {e}"))?;

        if let Some(err) = decoded.get("error") {
            return Err(format!("RPC {method} error: {err}"));
        }

        Ok(decoded.get("result").cloned().unwrap_or(Value::Null))
    }

    pub(crate) fn get_balance(&self, user: &str, token: &str) -> Result<Value, String> {
        self.rpc("getBalance", [BalanceParams { user, token }])
    }

    pub(crate) fn tx_status(&self, hash: &str) -> Result<Value, String> {
        self.rpc("getTransactionStatus", [hash])
    }

    pub(crate) fn tx_receipt(&self, hash: &str) -> Result<Value, String> {
        self.rpc("getTransactionReceipt", [hash])
    }

    pub(crate) fn send_transaction(&self, tx: &TransferTransaction) -> Result<Value, String> {
        self.rpc("sendTransaction", [tx])
    }

    pub(crate) fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        self.rpc(method, params)
    }
}

pub(crate) fn client_from(base_url: Option<&str>) -> Result<PelagosClient, String> {
    PelagosClient::new(base_url)
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct HealthArgs {
    /// Appchain base URL, e.g. `http://localhost:8080`. Defaults to `PELAGOS_RPC_URL` or `http://localhost:8080`.
    pub(crate) base_url: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GetBalanceArgs {
    /// Appchain base URL. Defaults to `PELAGOS_RPC_URL` or `http://localhost:8080`.
    pub(crate) base_url: Option<String>,
    /// Account name or address to query, e.g. `alice`.
    pub(crate) user: String,
    /// Token symbol to query, e.g. `USDT`.
    pub(crate) token: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct TxHashArgs {
    /// Appchain base URL. Defaults to `PELAGOS_RPC_URL` or `http://localhost:8080`.
    pub(crate) base_url: Option<String>,
    /// Transaction hash to look up, e.g. `0xabc123…`.
    pub(crate) tx_hash: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SendArgs {
    /// Appchain base URL. Defaults to `PELAGOS_RPC_URL` or `http://localhost:8080`.
    pub(crate) base_url: Option<String>,
    /// Sender account name or address.
    pub(crate) sender: String,
    /// Recipient account name or address.
    pub(crate) receiver: String,
    /// Integer token amount in the smallest denomination (e.g. `1000` for 1000 base units).
    pub(crate) value: u64,
    /// Token symbol, e.g. `USDT`.
    pub(crate) token: String,
    /// Unique transaction hash supplied by the caller (hex string, e.g. `0xdeadbeef`).
    pub(crate) hash: String,
    /// Must be `true` to confirm the write. Do not submit without explicit user approval.
    pub(crate) confirm: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct CallArgs {
    /// Appchain base URL. Defaults to `PELAGOS_RPC_URL` or `http://localhost:8080`.
    pub(crate) base_url: Option<String>,
    /// JSON-RPC method name, e.g. `getCustomData`.
    pub(crate) method: String,
    /// JSON-encoded array of positional params, e.g. `[{\"key\":\"val\"}]`. Pass `[]` for no params.
    pub(crate) params_json: String,
    /// Set `true` when calling a state-changing method; the user must confirm first.
    pub(crate) confirm: Option<bool>,
}

pub(crate) fn is_dedicated_method(method: &str) -> bool {
    matches!(
        method,
        "getBalance" | "getTransactionStatus" | "getTransactionReceipt" | "sendTransaction"
    )
}

pub(crate) fn looks_mutating(method: &str) -> bool {
    let lower = method.to_ascii_lowercase();
    !lower.starts_with("get")
        && !lower.starts_with("list")
        && !lower.starts_with("query")
        && !lower.starts_with("fetch")
}
