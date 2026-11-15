//! Helpers for native unit tests of tools.

use crate::Ctx;

/// A call context with fixed ids, no wallets, no chains and no continuation.
/// Set fields on it for the case under test.
pub fn ctx() -> Ctx {
    Ctx {
        call_id: "test-call".into(),
        thread_id: "test-thread".into(),
        account_id: None,
        agent_id: "test/agent".into(),
        wallets: Vec::new(),
        chains: Vec::new(),
        public: false,
        continuation: None,
    }
}
