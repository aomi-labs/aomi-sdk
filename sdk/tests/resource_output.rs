mod common;

use aomi_sdk::{
    DynAomiTool, DynToolMetadata, ResourceOutputDeclaration, RouteStep, RouteTarget, ToolReturn,
    host,
};
use common::fixtures::{EchoTool, TestApp};
use serde_json::json;

fn declaration() -> ResourceOutputDeclaration {
    serde_json::from_value(json!({
        "kind":"data.result@1","name":"quote","summary_pointer":"/economics",
        "sensitivity":"private","outputs":[{
            "name":"transaction","pointer":"/transaction","kind":"evm.transaction@1",
            "sensitivity":"private"
        }]
    }))
    .unwrap()
}

#[test]
fn legacy_manifest_remains_readable_with_no_declaration() {
    let metadata: DynToolMetadata = serde_json::from_value(json!({
        "name":"echo","app":"test","description":"Echo",
        "parameters_schema":{},"supports_async":false
    }))
    .unwrap();
    assert!(metadata.resource_output.is_none());
    assert!(EchoTool::descriptor(&TestApp).resource_output.is_none());
    assert!(
        serde_json::to_value(metadata)
            .unwrap()
            .get("resource_output")
            .is_none()
    );
}

#[test]
fn declaration_rejects_authority_fields_and_ambiguous_exports() {
    let valid = declaration();
    valid.validate().unwrap();
    for field in [
        "uri",
        "owner",
        "application",
        "thread",
        "invocation",
        "domain",
    ] {
        let mut wire = serde_json::to_value(&valid).unwrap();
        wire[field] = json!("untrusted");
        assert!(serde_json::from_value::<ResourceOutputDeclaration>(wire).is_err());
    }
    let mut invalid = valid.clone();
    invalid.outputs.push(invalid.outputs[0].clone());
    assert!(invalid.validate().is_err());
    invalid = valid.clone();
    invalid.outputs[0].pointer = "/bad~2escape".into();
    assert!(invalid.validate().is_err());
    invalid = valid.clone();
    invalid.outputs[0].pointer = "/part".repeat(17);
    assert!(invalid.validate().is_err());
    invalid = valid;
    invalid.outputs = (0..33)
        .map(|i| {
            let mut export = invalid.outputs[0].clone();
            export.name = format!("part_{i}");
            export
        })
        .collect();
    assert!(invalid.validate().is_err());
    for kind in [
        "data.result@+1",
        "data.result@01",
        "data.result@0",
        "data.result@",
    ] {
        let mut invalid = declaration();
        invalid.kind = kind.into();
        assert!(invalid.validate().is_err());
    }
    let mut invalid = declaration();
    invalid.schema = Some("x".repeat(8193));
    assert!(invalid.validate().is_err());
    invalid.schema = None;
    invalid.outputs[0].schema = Some("x".repeat(8193));
    assert!(invalid.validate().is_err());
}

#[test]
fn declared_output_does_not_replace_raw_route_and_callback_bytes() {
    let raw = json!({"message":"ab".repeat(4096),"attestation":"cd".repeat(2048),
        "transaction_hash":"0xdeadbeef","signature":"signed_exact","quote_id":"quote_7",
        "route_id":"route_3","submit_type":"venue"});
    let routes = vec![
        RouteStep::on_return_to::<host::EvmStageTx>(raw.clone()).bind_as("staged"),
        RouteStep::on_bound_event("submit_quote", raw.clone(), "wallet_result"),
    ];
    let returned = ToolReturn::with_routes(raw.clone(), routes.clone());
    let decoded = ToolReturn::from_value(serde_json::to_value(returned).unwrap()).unwrap();
    assert_eq!(decoded.value, raw);
    assert_eq!(decoded.routes, routes);
    declaration().validate().unwrap();
    assert_eq!(decoded.routes[0].args, raw);
    assert_eq!(decoded.routes[1].args, raw);
}

#[test]
fn evm_markers_emit_current_host_names() {
    assert_eq!(host::EvmStageTx::tool_name(), "evm_stage_tx");
    assert_eq!(host::StageTx::tool_name(), host::EvmStageTx::tool_name());
    assert_eq!(host::EvmCommitTxs::tool_name(), "evm_commit_txs");
    assert_eq!(
        host::CommitTxs::tool_name(),
        host::EvmCommitTxs::tool_name()
    );
    assert_eq!(host::CommitTx::tool_name(), host::EvmCommitTxs::tool_name());
    assert_eq!(host::SvmCommitTxs::tool_name(), "svm_commit_txs");
    assert_eq!(
        host::SvmCommitIx::tool_name(),
        host::SvmCommitTxs::tool_name()
    );
    assert_eq!(
        host::SvmCommitTx::tool_name(),
        host::SvmCommitTxs::tool_name()
    );
}
