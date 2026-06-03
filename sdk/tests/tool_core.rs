mod common;

use aomi_sdk::{DynAomiTool, DynToolCallCtx};
use common::fixtures::{EchoArgs, EchoTool, TestApp};
use serde_json::{Value, json};

#[test]
fn descriptor_schema_generation() {
    let descriptor = EchoTool::descriptor(&TestApp);
    assert_eq!(descriptor.name, "echo");
    assert_eq!(descriptor.app, "test");
    assert!(!descriptor.supports_async);
    assert_eq!(
        descriptor
            .parameters_schema
            .get("type")
            .and_then(Value::as_str),
        Some("object")
    );
}

#[test]
fn run_with_routes_wraps_legacy_run() {
    let result = EchoTool::run_with_routes(
        &TestApp,
        EchoArgs {
            name: "cecilia".to_string(),
        },
        DynToolCallCtx {
            session_id: "session".to_string(),
            tool_name: "echo".to_string(),
            call_id: "call".to_string(),
            state_attributes: Default::default(),
            secrets: Default::default(),
        },
    )
    .unwrap();

    assert_eq!(result.value, json!({"name": "cecilia"}));
    assert!(result.routes.is_empty());
}
