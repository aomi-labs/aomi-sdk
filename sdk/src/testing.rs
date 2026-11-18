//! Helpers for native unit tests of tools.

use crate::Ctx;
use crate::http::{HeaderMap, Response};

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

/// An HTTP reply as the host would return it, for testing code that decodes
/// one. Header names and values must be valid.
pub fn reply(status: u16, headers: &[(&str, &str)], body: impl Into<Vec<u8>>) -> Response {
    let mut map = HeaderMap::new();
    for (name, value) in headers {
        map.append(
            wstd::http::HeaderName::from_bytes(name.as_bytes()).expect("header name"),
            value.parse().expect("header value"),
        );
    }
    Response {
        status,
        headers: map,
        body: body.into(),
    }
}
