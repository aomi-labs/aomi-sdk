//! Read-only chain access over the host's `evm-read` and `svm-read`: plain
//! JSON-RPC params in, the JSON `result` out. With the `alloy` feature,
//! [`Evm::provider`] is an alloy provider over the same import.

use serde_json::{Value, json};

use crate::Result;
use crate::bindings::aomi::package::chain as host;

#[cfg(test)]
use fake::{evm_read, svm_read};
#[cfg(not(test))]
use host::{evm_read, svm_read};

pub use host::View;

#[cfg(test)]
pub(crate) mod fake;
#[cfg(feature = "alloy")]
mod provider;

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
        let reply = evm_read(self.chain_id, &self.view, method, &params.to_string())?;
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
        let reply = svm_read(&self.cluster, method, &params.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::fake::{self, ready};
    use super::*;

    #[test]
    fn helpers_send_their_method_params_and_view() {
        let owner = "0x00000000219ab540356cBB839Cbe05303d7705Fa";
        fake::evm(
            8453,
            View::Staged(vec![3]),
            "eth_getBalance",
            json!([owner, "latest"]),
            Ok(json!("0x10")),
        );
        let balance = ready(evm(8453).at(View::Staged(vec![3])).balance(owner)).unwrap();
        assert_eq!(balance, json!("0x10"));
        let key = "So11111111111111111111111111111111111111112";
        fake::svm(
            "devnet",
            "getAccountInfo",
            json!([key, { "encoding": "base64" }]),
            json!({ "value": null }),
        );
        assert_eq!(
            ready(svm("devnet").account(key)).unwrap(),
            json!({ "value": null })
        );
        fake::done();
    }
}
