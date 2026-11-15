//! A two-tool package exercised natively through the WIT guest `package!` generates.

use aomi_sdk::bindings::Guest;
use aomi_sdk::bindings::aomi::package::types::{
    ContinuationData, Outcome as WitOutcome, SignRequest as WitSign, StageKind,
};
use aomi_sdk::{
    Audience, Ctx, Erc191Domain, Image, Outcome, Result, SignRequest, StageTx, Tool, testing,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Deserialize, schemars::JsonSchema)]
pub struct EchoArgs {
    text: String,
}

pub struct Echo;

impl Tool for Echo {
    const NAME: &'static str = "echo";
    const DESCRIPTION: &'static str = "Echo the text.";
    type Args = EchoArgs;

    async fn run(args: EchoArgs, ctx: &Ctx) -> Result<Outcome> {
        Ok(Outcome::data(
            json!({ "text": args.text, "call": ctx.call_id }),
        ))
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct State {
    order: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NoArgs {}

pub struct Submit;

impl Tool for Submit {
    const NAME: &'static str = "submit";
    const DESCRIPTION: &'static str = "Submit the signed order.";
    const AUDIENCE: Audience = Audience::Direct;
    const CONTINUATION: bool = true;
    type Args = NoArgs;

    async fn run(_args: NoArgs, ctx: &Ctx) -> Result<Outcome> {
        let c = ctx.continuation::<State>()?;
        Ok(Outcome::data(
            json!({ "order": c.state.order, "signature": c.signature }),
        ))
    }
}

aomi_sdk::package!(Echo, Submit);

fn data(outcome: WitOutcome) -> Value {
    match outcome {
        WitOutcome::Data(d) => serde_json::from_str(&d.value).unwrap(),
        other => panic!("expected data, got {other:?}"),
    }
}

#[test]
fn describe_lists_both_tools() {
    let manifest = AomiPackage::describe();
    assert_eq!(manifest.contract, 1);
    let [echo, submit] = manifest.tools.as_slice() else {
        panic!("two tools")
    };
    assert_eq!(
        (echo.name.as_str(), submit.name.as_str()),
        ("echo", "submit")
    );
    assert_eq!(echo.audience, Audience::Model);
    assert!(!echo.continuation);
    assert_eq!(submit.audience, Audience::Direct);
    assert!(submit.continuation);
    let schema: Value = serde_json::from_str(&echo.schema).unwrap();
    assert_eq!(schema["properties"]["text"]["type"], "string");
    assert_eq!(schema["required"], json!(["text"]));
}

#[test]
fn call_tool_dispatches_and_maps_errors() {
    let out = AomiPackage::call_tool("echo".into(), r#"{"text":"hi"}"#.into(), testing::ctx());
    assert_eq!(
        data(out.unwrap()),
        json!({ "text": "hi", "call": "test-call" })
    );

    let bad = AomiPackage::call_tool("echo".into(), r#"{"txt":1}"#.into(), testing::ctx());
    let e = bad.unwrap_err();
    assert_eq!(e.code, "invalid_args");
    assert!(e.message.contains("text"), "{}", e.message);

    let unknown = AomiPackage::call_tool("nope".into(), "{}".into(), testing::ctx());
    assert_eq!(unknown.unwrap_err().code, "unknown_tool");
}

#[test]
fn continuation_reaches_the_tool() {
    let missing = AomiPackage::call_tool("submit".into(), "{}".into(), testing::ctx());
    assert_eq!(missing.unwrap_err().code, "continuation_invalid");

    let mut ctx = testing::ctx();
    ctx.continuation = Some(ContinuationData {
        commit_id: "c1".into(),
        state: r#"{"order":"o-7"}"#.into(),
        signature: Some("0xsig".into()),
        tx_hash: None,
        commit_state: "confirmed".into(),
        source_tx_id: None,
    });
    let out = AomiPackage::call_tool("submit".into(), "{}".into(), ctx).unwrap();
    assert_eq!(data(out), json!({ "order": "o-7", "signature": "0xsig" }));
}

#[test]
fn data_outcome_carries_images() {
    let png = vec![0x89, b'P', b'N', b'G'];
    let out = Outcome::data(json!({ "pnl": 1 }))
        .image(Image::png(png.clone()).caption("PnL"))
        .wit()
        .unwrap();
    let WitOutcome::Data(d) = out else {
        panic!("data")
    };
    assert_eq!(d.value, r#"{"pnl":1}"#);
    assert_eq!(d.attachments.len(), 1);
    assert_eq!(d.attachments[0].bytes, png);
    assert_eq!(d.attachments[0].caption.as_deref(), Some("PnL"));
}

#[test]
fn prepared_outcome_carries_evidence_and_follow_up() {
    let out = Outcome::prepared(json!({ "route": "lifi" }))
        .tx(StageTx::evm(json!({ "to": "0x11", "data": "0x" })).evidence(2))
        .follow_up("status", "Check the bridge", &json!({ "id": "t-1" }))
        .failure_rule(&["revert"], "slippage", "Quote again")
        .wit()
        .unwrap();
    let WitOutcome::Prepared(p) = out else {
        panic!("prepared")
    };
    let tx = &p.txs[0];
    assert!(matches!(tx.stage, StageKind::EvmStageTx));
    assert_eq!(tx.evidence, Some(2));
    let follow_up = tx.follow_up.as_ref().unwrap();
    assert_eq!(
        (follow_up.tool.as_str(), follow_up.data.as_str()),
        ("status", r#"{"id":"t-1"}"#)
    );
    assert_eq!(p.failure_rules[0].any_of, ["revert"]);

    let orphan = Outcome::prepared(json!({})).follow_up("status", "", &json!({}));
    assert_eq!(orphan.wit().unwrap_err().code, "outcome_invalid");
    let wrong = Outcome::data(json!({})).tx(StageTx::svm_tx(json!({})));
    assert_eq!(wrong.wit().unwrap_err().code, "outcome_invalid");
}

#[test]
fn sign_outcome_carries_then_and_caps_state() {
    let domain = Erc191Domain {
        app: "venue".into(),
        chain_id: Some(1),
        verifying_contract: None,
        purpose: None,
    };
    let state = State {
        order: "o-7".into(),
    };
    let out = Outcome::sign(
        json!("Sign in"),
        SignRequest::personal("hello", "Log in", domain),
    )
    .then("submit", "Submit the order", &state)
    .expires_at(60)
    .wit()
    .unwrap();
    let WitOutcome::Sign(s) = out else {
        panic!("sign")
    };
    assert!(matches!(s.request, WitSign::EvmPersonal(ref p) if p.message == "hello"));
    assert_eq!(s.then.tool, "submit");
    assert_eq!(s.then.state, r#"{"order":"o-7"}"#);
    assert_eq!(s.then.expires_at, Some(60));

    let typed = || SignRequest::typed_data(json!({ "primaryType": "Order" }), "Approve");
    assert_eq!(
        Outcome::sign(json!({}), typed()).wit().unwrap_err().code,
        "outcome_invalid"
    );
    let big = State {
        order: "x".repeat(16 * 1024),
    };
    let capped = Outcome::sign(json!({}), typed()).then("submit", "", &big);
    assert_eq!(capped.wit().unwrap_err().code, "state_too_large");
}
