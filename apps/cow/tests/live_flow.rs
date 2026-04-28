#[allow(dead_code)]
#[path = "../src/client.rs"]
mod client;
#[allow(dead_code)]
#[path = "../src/tool.rs"]
mod tool;

use alloy::signers::{SignerSync, local::PrivateKeySigner};
use alloy_dyn_abi::eip712::TypedData;
use aomi_sdk::{
    DynAomiApp, DynAomiTool, DynAsyncSink, DynToolCallCtx, DynToolDispatch, DynToolMetadata,
};
use serde_json::{Map, Value};
use std::str::FromStr;

const TEST_PRIVATE_KEY: &str = "0x0000000000000000000000000000000000000000000000000000000000000001";

impl DynAomiApp for client::CowApp {
    fn name(&self) -> &'static str {
        "cow-live-test"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn preamble(&self) -> &'static str {
        ""
    }

    fn tools(&self) -> Vec<DynToolMetadata> {
        Vec::new()
    }

    fn start_tool(
        &self,
        _name: &str,
        _args_json: &str,
        _ctx_json: &str,
        _sink: DynAsyncSink,
    ) -> DynToolDispatch {
        unreachable!("live_flow does not use dynamic dispatch")
    }
}

fn tool_ctx(tool_name: &str) -> DynToolCallCtx {
    DynToolCallCtx {
        session_id: "cow-live-test".to_string(),
        tool_name: tool_name.to_string(),
        call_id: format!("{tool_name}-live"),
        state_attributes: Map::new(),
    }
}

fn sign_typed_data(signer: &PrivateKeySigner, typed_data: &Value) -> Result<String, String> {
    let typed: TypedData = serde_json::from_value(typed_data.clone())
        .map_err(|e| format!("invalid typed data payload: {e}"))?;
    let hash = typed
        .eip712_signing_hash()
        .map_err(|e| format!("failed to hash typed data: {e}"))?;
    let signature = signer
        .sign_hash_sync(&hash)
        .map_err(|e| format!("failed to sign typed data: {e}"))?;
    Ok(signature.to_string())
}

#[test]
#[ignore = "live integration against CoW orderbook"]
fn live_quote_sign_submit_avoids_wrong_owner() {
    let signer =
        PrivateKeySigner::from_str(TEST_PRIVATE_KEY).expect("fixture private key should parse");
    let address = format!("{:#x}", signer.address());

    let quote = <client::GetCowSwapQuote as DynAomiTool>::run(
        &client::CowApp,
        client::GetCowSwapQuoteArgs {
            chain: "polygon".to_string(),
            sell_token: "usdc".to_string(),
            buy_token: "weth".to_string(),
            amount: 1.0,
            sender_address: address.clone(),
            receiver_address: None,
            order_side: Some("sell".to_string()),
            valid_to: None,
            partially_fillable: None,
            signing_scheme: Some("eip712".to_string()),
            slippage: None,
        },
        tool_ctx("get_cow_swap_quote"),
    )
    .expect("quote should succeed");

    let receiver = quote
        .pointer("/quote/receiver")
        .and_then(Value::as_str)
        .expect("quote should include receiver");
    assert!(receiver.eq_ignore_ascii_case(&address));

    let typed_data = quote
        .get("typed_data")
        .cloned()
        .expect("typed_data missing");
    assert_eq!(
        typed_data
            .pointer("/domain/verifyingContract")
            .and_then(Value::as_str),
        Some(client::COW_SETTLEMENT_CONTRACT)
    );

    let signature = sign_typed_data(&signer, &typed_data).expect("typed data should sign");
    let mut submit_args = quote
        .get("submit_args_template")
        .cloned()
        .expect("submit args template missing");
    submit_args["signed_order"]["signature"] = Value::String(signature);

    let submit_args: client::PlaceCowOrderArgs =
        serde_json::from_value(submit_args).expect("submit args should deserialize");
    let err = <client::PlaceCowOrder as DynAomiTool>::run(
        &client::CowApp,
        submit_args,
        tool_ctx("place_cow_order"),
    )
    .expect_err("fixture signer should not have live Polygon USDC balance");

    assert!(
        err.contains("InsufficientBalance"),
        "expected insufficient balance error, got: {err}"
    );
    assert!(
        !err.contains("WrongOwner"),
        "signature should no longer fail owner recovery: {err}"
    );
}
