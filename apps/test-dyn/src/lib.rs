use aomi_sdk::{
    DynAomiTool, DynToolCallCtx, dyn_aomi_app,
    schemars::JsonSchema,
    serde_json::{Value, json},
};
use serde::Deserialize;

#[derive(Clone, Default)]
struct TestDynApp;

#[derive(Debug, Clone, Deserialize, JsonSchema, Default)]
struct PingArgs {
    #[serde(default)]
    message: Option<String>,
}

struct PingTool;

impl DynAomiTool for PingTool {
    type App = TestDynApp;
    type Args = PingArgs;

    const NAME: &'static str = "ping";
    const DESCRIPTION: &'static str =
        "Return a deterministic payload to verify dynamic app loading.";

    fn run(app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let _ = app;
        Ok(json!({
            "app": "test-dyn",
            "tool": "ping",
            "message": args.message.unwrap_or_else(|| "pong".to_string()),
            "session_id": ctx.session_id,
            "call_id": ctx.call_id,
        }))
    }
}

dyn_aomi_app!(
    app = TestDynApp,
    name = "test-dyn",
    version = "0.1.0",
    preamble = "A minimal dynamic test app used to verify plugin fetch, load, authorization, and API-key gating.",
    tools = [PingTool],
    namespaces = []
);
