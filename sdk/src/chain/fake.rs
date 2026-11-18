//! Recorded host replies for native tests: each read must match the next
//! expectation exactly (chain, view, method, params) and takes its reply.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::IntoFuture;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use serde_json::Value;

use super::View;
use crate::bindings::aomi::package::chain::ReadReply;
use crate::bindings::aomi::package::types::HostError;

struct Expect {
    target: String,
    method: String,
    params: Value,
    reply: Result<Value, HostError>,
}

thread_local! {
    static EXPECTED: RefCell<VecDeque<Expect>> = const { RefCell::new(VecDeque::new()) };
}

/// Expects one `evm-read` and answers it.
pub fn evm(
    chain_id: u64,
    view: View,
    method: &str,
    params: Value,
    reply: Result<Value, HostError>,
) {
    push(format!("evm {chain_id} {view:?}"), method, params, reply);
}

/// Expects one `svm-read` and answers it.
pub fn svm(cluster: &str, method: &str, params: Value, reply: Value) {
    push(format!("svm {cluster}"), method, params, Ok(reply));
}

/// Panics if an expected read never happened.
pub fn done() {
    EXPECTED.with(|e| assert!(e.borrow().is_empty(), "{} reads not made", e.borrow().len()));
}

fn push(target: String, method: &str, params: Value, reply: Result<Value, HostError>) {
    EXPECTED.with(|e| {
        e.borrow_mut().push_back(Expect {
            target,
            method: method.into(),
            params,
            reply,
        })
    });
}

fn answer(target: String, method: &str, params: &str) -> Result<ReadReply, HostError> {
    let next = EXPECTED
        .with(|e| e.borrow_mut().pop_front())
        .unwrap_or_else(|| panic!("unexpected read {target} {method} {params}"));
    let params: Value = serde_json::from_str(params).expect("params are JSON");
    assert_eq!(
        (target.as_str(), method, &params),
        (next.target.as_str(), next.method.as_str(), &next.params)
    );
    next.reply.map(|result| ReadReply {
        result: result.to_string(),
        at: None,
    })
}

pub fn evm_read(
    chain_id: u64,
    view: &View,
    method: &str,
    params: &str,
) -> Result<ReadReply, HostError> {
    answer(format!("evm {chain_id} {view:?}"), method, params)
}

pub fn svm_read(cluster: &str, method: &str, params: &str) -> Result<ReadReply, HostError> {
    answer(format!("svm {cluster}"), method, params)
}

/// Polls once: every host read resolves without yielding.
pub fn ready<T>(f: impl IntoFuture<Output = T>) -> T {
    match pin!(f.into_future()).poll(&mut Context::from_waker(Waker::noop())) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("a host read yielded"),
    }
}
