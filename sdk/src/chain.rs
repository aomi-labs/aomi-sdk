//! Read-only chain access over the host's `evm-read` and `svm-read`: plain
//! JSON-RPC params in, the JSON `result` out.

use serde_json::{Value, json};

use crate::Result;
use crate::bindings::aomi::package::chain as host;

pub use host::View;

/// Reads on an EVM chain, at the latest block unless [`Evm::at`] says otherwise.
pub fn evm(chain_id: u64) -> Evm {
    Evm {
        chain_id,
        view: View::Latest,
    }
}

/// Reads on a Solana cluster ("mainnet-beta", "devnet"), at latest state.
pub fn svm(cluster: impl Into<String>) -> Svm {
    Svm {
        cluster: cluster.into(),
    }
}

#[derive(Clone, Debug)]
pub struct Evm {
    chain_id: u64,
    view: View,
}

impl Evm {
    /// Which state the reads see: latest, an exact block, or after this
    /// thread's staged transactions.
    pub fn at(mut self, view: View) -> Self {
        self.view = view;
        self
    }

    /// One allowlisted JSON-RPC method. The view decides the block, so a block
    /// parameter must be "latest" or absent.
    pub async fn read(&self, method: &str, params: Value) -> Result<Value> {
        let reply = host::evm_read(self.chain_id, &self.view, method, &params.to_string())?;
        Ok(serde_json::from_str(&reply.result)?)
    }

    /// `eth_call` with a transaction object (`to`, `data`, …).
    pub async fn call(&self, tx: Value) -> Result<Value> {
        self.read("eth_call", json!([tx, "latest"])).await
    }

    pub async fn balance(&self, address: &str) -> Result<Value> {
        self.read("eth_getBalance", json!([address, "latest"]))
            .await
    }

    pub async fn code(&self, address: &str) -> Result<Value> {
        self.read("eth_getCode", json!([address, "latest"])).await
    }

    pub async fn nonce(&self, address: &str) -> Result<Value> {
        self.read("eth_getTransactionCount", json!([address, "latest"]))
            .await
    }

    pub async fn block_number(&self) -> Result<Value> {
        self.read("eth_blockNumber", json!([])).await
    }

    pub async fn receipt(&self, hash: &str) -> Result<Value> {
        self.read("eth_getTransactionReceipt", json!([hash])).await
    }

    pub async fn transaction(&self, hash: &str) -> Result<Value> {
        self.read("eth_getTransactionByHash", json!([hash])).await
    }
}

#[derive(Clone, Debug)]
pub struct Svm {
    cluster: String,
}

impl Svm {
    /// One allowlisted Solana JSON-RPC method.
    pub async fn read(&self, method: &str, params: Value) -> Result<Value> {
        let reply = host::svm_read(&self.cluster, method, &params.to_string())?;
        Ok(serde_json::from_str(&reply.result)?)
    }

    /// `getAccountInfo`, data base64-encoded.
    pub async fn account(&self, address: &str) -> Result<Value> {
        self.read("getAccountInfo", json!([address, { "encoding": "base64" }]))
            .await
    }

    pub async fn balance(&self, address: &str) -> Result<Value> {
        self.read("getBalance", json!([address])).await
    }

    /// `getTokenAccountsByOwner`; `filter` is `{"mint": ..}` or `{"programId": ..}`.
    pub async fn token_accounts(&self, owner: &str, filter: Value) -> Result<Value> {
        self.read(
            "getTokenAccountsByOwner",
            json!([owner, filter, { "encoding": "jsonParsed" }]),
        )
        .await
    }

    /// `getSignaturesForAddress`, newest first.
    pub async fn signatures(&self, address: &str, limit: u32) -> Result<Value> {
        self.read(
            "getSignaturesForAddress",
            json!([address, { "limit": limit }]),
        )
        .await
    }

    pub async fn transaction(&self, signature: &str) -> Result<Value> {
        self.read(
            "getTransaction",
            json!([signature, { "encoding": "json", "maxSupportedTransactionVersion": 0 }]),
        )
        .await
    }
}
