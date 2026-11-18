//! An alloy provider over `evm-read` (spike S5a). [`Evm`] is the transport:
//! each JSON-RPC request becomes one host read at the view, keeping its id.
//! Only the host's allowlisted methods succeed, and a block argument must be
//! "latest" or absent; the view decides the block. alloy's `call` asks for
//! the "pending" block, so that tag is sent as "latest".

use std::borrow::Cow;
use std::task::{Context, Poll};

use alloy_json_rpc::{
    ErrorPayload, RequestPacket, Response, ResponsePacket, ResponsePayload, SerializedRequest,
};
use alloy_provider::RootProvider;
use alloy_rpc_client::RpcClient;
use alloy_transport::{TransportError, TransportFut};
use serde_json::Value;
use serde_json::value::{RawValue, to_raw_value};

use super::{Evm, evm_read};
use crate::{Error, err};

/// The JSON-RPC error code a host refusal carries; its `data` is the host code.
const HOST_REFUSAL: i64 = -32000;

impl Evm {
    /// A plain alloy provider whose reads see this view.
    pub fn provider(&self) -> RootProvider {
        RootProvider::new(RpcClient::new(self.clone(), false))
    }

    fn respond(&self, request: &SerializedRequest) -> Response {
        let params = latest(request.params().map_or("[]", RawValue::get));
        let payload = match evm_read(self.chain_id, &self.view, request.method(), &params) {
            Ok(reply) => match RawValue::from_string(reply.result) {
                Ok(result) => ResponsePayload::Success(result),
                Err(e) => refusal("invalid_json", e.to_string()),
            },
            Err(e) => refusal(&e.code, e.message),
        };
        Response {
            id: request.id().clone(),
            payload,
        }
    }
}

/// Rewrites a "pending" block argument (the second parameter) to "latest".
fn latest(params: &str) -> Cow<'_, str> {
    if !params.contains("\"pending\"") {
        return Cow::Borrowed(params);
    }
    match serde_json::from_str(params) {
        Ok(Value::Array(mut a)) if a.get(1).and_then(Value::as_str) == Some("pending") => {
            a[1] = "latest".into();
            Cow::Owned(Value::Array(a).to_string())
        }
        _ => Cow::Borrowed(params),
    }
}

fn refusal(code: &str, message: String) -> ResponsePayload {
    ResponsePayload::Failure(ErrorPayload {
        code: HOST_REFUSAL,
        message: message.into(),
        data: to_raw_value(code).ok(),
    })
}

impl tower::Service<RequestPacket> for Evm {
    type Response = ResponsePacket;
    type Error = TransportError;
    type Future = TransportFut<'static>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), TransportError>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, packet: RequestPacket) -> Self::Future {
        let reply = match packet {
            RequestPacket::Single(r) => ResponsePacket::Single(self.respond(&r)),
            RequestPacket::Batch(rs) => {
                ResponsePacket::Batch(rs.iter().map(|r| self.respond(r)).collect())
            }
        };
        Box::pin(async move { Ok(reply) })
    }
}

/// A host refusal keeps its code (`view_unsupported`, `chain_read_limit`, …);
/// any other provider failure is `chain_read_failed`.
impl From<TransportError> for Error {
    fn from(e: TransportError) -> Self {
        let refusal = e.as_error_resp().and_then(|p| {
            let code: String = serde_json::from_str(p.data.as_ref()?.get()).ok()?;
            Some(err(&code, p.message.to_string()))
        });
        refusal.unwrap_or_else(|| err("chain_read_failed", e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, U256, address, bytes};
    use alloy_provider::Provider;
    use alloy_rpc_types_eth::TransactionRequest;
    use serde_json::json;

    use super::*;
    use crate::bindings::aomi::package::types::HostError;
    use crate::chain::fake::{self, ready};
    use crate::chain::{self, View};

    const WETH: Address = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

    #[test]
    fn call_balance_and_block_number_go_through_evm_read() {
        let holder = address!("0x00000000219ab540356cBB839Cbe05303d7705Fa");
        // Mainnet-shaped replies: WETH decimals(), a balance, the head.
        fake::evm(
            1,
            View::Latest,
            "eth_call",
            json!([{ "to": WETH, "input": "0x313ce567" }, "latest"]),
            Ok(json!(
                "0x0000000000000000000000000000000000000000000000000000000000000012"
            )),
        );
        fake::evm(
            1,
            View::Latest,
            "eth_getBalance",
            json!([holder, "latest"]),
            Ok(json!("0x1b1ae4d6e2ef500000")),
        );
        fake::evm(
            1,
            View::Latest,
            "eth_blockNumber",
            json!([]),
            Ok(json!("0x15f2f3c")),
        );
        let p = chain::evm(1).provider();
        let tx = TransactionRequest::default()
            .to(WETH)
            .input(bytes!("313ce567").into());
        let out = ready(p.call(tx)).unwrap();
        assert_eq!(out[31], 18);
        let balance = ready(p.get_balance(holder)).unwrap();
        assert_eq!(balance, U256::from(500u128 * 10u128.pow(18)));
        assert_eq!(ready(p.get_block_number()).unwrap(), 0x15f2f3c);
        fake::done();
    }

    #[test]
    fn the_view_reaches_the_host() {
        fake::evm(
            8453,
            View::Staged(vec![7, 9]),
            "eth_getTransactionCount",
            json!([WETH, "latest"]),
            Ok(json!("0x2")),
        );
        let p = chain::evm(8453).at(View::Staged(vec![7, 9])).provider();
        assert_eq!(ready(p.get_transaction_count(WETH)).unwrap(), 2);
        fake::done();
    }

    #[test]
    fn a_host_refusal_keeps_its_code() {
        fake::evm(
            1,
            View::Block(5),
            "eth_getTransactionReceipt",
            json!(["0x0000000000000000000000000000000000000000000000000000000000000001"]),
            Err(HostError {
                code: "view_unsupported".into(),
                message: "receipts read latest only".into(),
            }),
        );
        let p = chain::evm(1).at(View::Block(5)).provider();
        let e: Error = ready(
            p.get_transaction_receipt(
                "0x0000000000000000000000000000000000000000000000000000000000000001"
                    .parse()
                    .unwrap(),
            ),
        )
        .unwrap_err()
        .into();
        assert_eq!(
            (e.code.as_str(), e.message.as_str()),
            ("view_unsupported", "receipts read latest only")
        );
        fake::done();
    }
}
