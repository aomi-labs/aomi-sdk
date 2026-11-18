//! Typed reads of the WIT `call-context`.

use serde::de::DeserializeOwned;

use crate::{Ctx, Family, Result, Wallet, err};

/// The continuation of a `sign` outcome or a prepared transaction's follow-up,
/// with the package's state parsed back into its own type.
#[derive(Clone, Debug)]
pub struct Continuation<T> {
    pub commit_id: String,
    /// The state from `then`, or the data from `follow_up`.
    pub state: T,
    /// A confirmed signature request: EVM 0x hex, SVM base64.
    pub signature: Option<String>,
    /// A confirmed transaction commit: its hash or signature.
    pub tx_hash: Option<String>,
    /// "awaiting", "signed", "broadcast", "confirmed", "rejected", "expired" or "failed".
    pub commit_state: String,
    /// The staged transaction of a batch the follow-up belongs to.
    pub source_tx_id: Option<String>,
}

impl Ctx {
    /// The wallet of `family` in scope for this call; at most one per family.
    pub fn wallet(&self, family: Family) -> Option<&Wallet> {
        self.wallets.iter().find(|w| w.family == family)
    }

    /// The continuation the host resolved for this call. Fails when the call
    /// has none or its state does not parse as `T`.
    pub fn continuation<T: DeserializeOwned>(&self) -> Result<Continuation<T>> {
        let c = self
            .continuation
            .as_ref()
            .ok_or_else(|| err("continuation_invalid", "this call has no continuation"))?;
        let state = serde_json::from_str(&c.state)
            .map_err(|e| err("continuation_invalid", format!("state: {e}")))?;
        Ok(Continuation {
            commit_id: c.commit_id.clone(),
            state,
            signature: c.signature.clone(),
            tx_hash: c.tx_hash.clone(),
            commit_state: c.commit_state.clone(),
            source_tx_id: c.source_tx_id.clone(),
        })
    }
}
