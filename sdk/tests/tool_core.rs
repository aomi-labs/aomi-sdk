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
fn tool_context_debug_redacts_secret_values() {
    let secret = "debug-must-not-print-this-value";
    let ctx = aomi_sdk::testing::TestCtxBuilder::new("debug_test")
        .secret("DEMO_TOKEN", secret)
        .build();

    let debug = format!("{ctx:?}");
    assert!(debug.contains("secrets: \"<redacted>\""));
    assert!(!debug.contains(secret));
}

#[test]
fn user_secret_resolution_uses_only_the_injected_context() {
    let ctx = aomi_sdk::testing::TestCtxBuilder::new("secret_test")
        .secret("DEMO_TOKEN", "  injected-value  ")
        .build();
    assert_eq!(
        aomi_sdk::resolve_user_secret_value(&ctx, "DEMO_TOKEN", "missing").unwrap(),
        "injected-value"
    );

    let empty = aomi_sdk::testing::TestCtxBuilder::new("secret_test").build();
    assert_eq!(
        aomi_sdk::resolve_user_secret_value(&empty, "PATH", "missing").unwrap_err(),
        "missing",
        "user-owned resolution must not fall back to the process environment"
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
