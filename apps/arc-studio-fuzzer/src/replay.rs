//! Wire shapes shared with the host's `evm-sim` replay tool. The app emits
//! [`ReplayBatch`] for `sim_replay_traces` and reads back its compact output
//! as [`CompactTrace`]. Limits mirror `aomi_evm::world::ReplayBatch`.

use aomi_sdk::schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Checkpoint the campaign asks the host to take after setup.
pub(crate) const BASELINE: &str = "arc-fuzz-baseline";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ReplayCall {
    pub label: String,
    pub from: String,
    pub to: String,
    pub data: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReplayTrace {
    pub id: String,
    pub calls: Vec<ReplayCall>,
    pub probes: Vec<ReplayCall>,
}

/// Arguments for one `sim_replay_traces` call (the model adds `compact`).
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReplayBatch {
    pub snapshot: &'static str,
    pub chain_id: u64,
    pub watch: Vec<String>,
    pub traces: Vec<ReplayTrace>,
}

impl ReplayBatch {
    pub const MAX_TRACES: usize = 64;
    pub const MAX_CALLS: usize = 8;
    pub const MAX_PROBES: usize = 24;
    pub const MAX_WATCH: usize = 16;
    pub const MAX_EXECUTIONS: usize = 512;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReplayStatus {
    Succeeded,
    Reverted,
    Halted,
    Failed,
    /// The host could not execute the call; says nothing about the contract.
    EngineError,
}

impl ReplayStatus {
    pub fn succeeded(self) -> bool {
        self == Self::Succeeded
    }

    pub fn conclusive(self) -> bool {
        self != Self::EngineError
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Reverted => "reverted",
            Self::Halted => "halted",
            Self::Failed => "failed",
            Self::EngineError => "engine_error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplayProbe {
    pub label: String,
    pub status: ReplayStatus,
    #[serde(default)]
    pub return_data: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReplayBalance {
    pub address: String,
    /// Decimal atomic units.
    pub before: String,
    pub after: String,
}

/// One compact trace exactly as `sim_replay_traces` returned it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CompactTrace {
    pub id: String,
    pub statuses: Vec<ReplayStatus>,
    #[serde(default)]
    pub probes: Vec<ReplayProbe>,
    /// Only watched balances that changed.
    #[serde(default)]
    pub balances: Vec<ReplayBalance>,
}

impl CompactTrace {
    /// Every planned call ran and the host executed it.
    pub fn conclusive(&self, planned_calls: usize) -> bool {
        self.statuses.len() == planned_calls
            && self.statuses.iter().all(|status| status.conclusive())
    }
}
