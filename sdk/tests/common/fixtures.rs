use aomi_sdk::{
    DynAomiApp, DynAomiTool, DynAsyncSink, DynToolCallCtx, DynToolDispatch, DynToolMetadata,
    DynToolResult,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone, Default)]
pub struct TestApp;

impl DynAomiApp for TestApp {
    fn name(&self) -> &'static str {
        "test"
    }

    fn version(&self) -> &'static str {
        "0.0.0"
    }

    fn preamble(&self) -> &'static str {
        "test preamble"
    }

    fn tools(&self) -> Vec<DynToolMetadata> {
        vec![EchoTool::descriptor(self)]
    }

    fn start_tool(&self, _: &str, _: &str, _: &str, _: DynAsyncSink) -> DynToolDispatch {
        DynToolDispatch::Ready(DynToolResult::err("not needed in this test"))
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct EchoArgs {
    pub name: String,
}

pub struct EchoTool;

impl DynAomiTool for EchoTool {
    type App = TestApp;
    type Args = EchoArgs;

    const NAME: &'static str = "echo";
    const DESCRIPTION: &'static str = "echo input";

    fn run(_app: &Self::App, args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        Ok(json!({"name": args.name}))
    }
}

pub struct SubmitOrder;

impl DynAomiTool for SubmitOrder {
    type App = TestApp;
    type Args = Value;

    const NAME: &'static str = "submit_order";
    const DESCRIPTION: &'static str = "submit";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        Ok(Value::Null)
    }
}

pub struct SyncTool;

impl DynAomiTool for SyncTool {
    type App = TestApp;
    type Args = Value;

    const NAME: &'static str = "sync_tool";
    const DESCRIPTION: &'static str = "sync";

    fn run(_app: &Self::App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
        Ok(Value::Null)
    }
}

pub struct AsyncTool;

impl DynAomiTool for AsyncTool {
    type App = TestApp;
    type Args = Value;

    const NAME: &'static str = "async_tool";
    const DESCRIPTION: &'static str = "async";
    const IS_ASYNC: bool = true;
}
