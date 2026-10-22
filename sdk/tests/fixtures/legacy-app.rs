use aomi_sdk::{
    DynAomiTool, DynAsyncSink, DynToolCallCtx, dyn_aomi_app,
    schemars::JsonSchema,
    serde_json::{Value, json},
};
use serde::Deserialize;
use std::{thread, time::Duration};

#[derive(Clone, Default)]
struct HelloApp;

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct GreetArgs {
    name: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct SlowSyncArgs {
    duration_ms: u64,
    #[serde(default)]
    tag: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct CountArgs {
    upto: u64,
    #[serde(default)]
    delay_ms: Option<u64>,
    #[serde(default)]
    tag: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct FailArgs {
    message: String,
    #[serde(default)]
    tag: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
struct WaitForCancelArgs {
    #[serde(default)]
    delay_ms: Option<u64>,
    #[serde(default)]
    tag: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema, Default)]
struct EmptyArgs {}

fn sleep_if_needed(delay_ms: Option<u64>) {
    if let Some(delay_ms) = delay_ms
        && delay_ms > 0
    {
        thread::sleep(Duration::from_millis(delay_ms));
    }
}

struct GreetTool;

impl DynAomiTool for GreetTool {
    type App = HelloApp;
    type Args = GreetArgs;

    const NAME: &'static str = "greet";
    const DESCRIPTION: &'static str = "Return a greeting payload.";

    fn run(app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let _ = app;
        Ok(json!({
            "greeting": format!("Hello, {}!", args.name),
            "session_id": ctx.session_id,
            "plugin": "dyn-hello v0.1.0",
        }))
    }
}

struct SlowSyncTool;

impl DynAomiTool for SlowSyncTool {
    type App = HelloApp;
    type Args = SlowSyncArgs;

    const NAME: &'static str = "slow_sync";
    const DESCRIPTION: &'static str =
        "Sleep for a configurable duration and return tagged sync metadata.";

    fn run(app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let _ = app;
        sleep_if_needed(Some(args.duration_ms));
        Ok(json!({
            "slept_ms": args.duration_ms,
            "session_id": ctx.session_id,
            "tool": "slow_sync",
            "call_id": ctx.call_id,
            "tag": args.tag,
        }))
    }
}

struct CountAsyncTool;

impl DynAomiTool for CountAsyncTool {
    type App = HelloApp;
    type Args = CountArgs;

    const NAME: &'static str = "count_async";
    const DESCRIPTION: &'static str = "Emit async count updates up to a target.";
    const IS_ASYNC: bool = true;

    fn run_async(
        app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
        sink: DynAsyncSink,
    ) -> Result<(), String> {
        let _ = app;
        for i in 1..=args.upto {
            if sink.is_canceled() {
                return Ok(());
            }
            sleep_if_needed(args.delay_ms);
            if i < args.upto {
                sink.emit(json!({
                    "count": i,
                    "session_id": ctx.session_id,
                    "tool": "count_async",
                    "call_id": ctx.call_id,
                    "tag": args.tag,
                }))?;
            } else {
                sink.complete(json!({
                    "count": i,
                    "session_id": ctx.session_id,
                    "tool": "count_async",
                    "call_id": ctx.call_id,
                    "tag": args.tag,
                }))?;
            }
        }
        Ok(())
    }
}

struct FailAsyncTool;

impl DynAomiTool for FailAsyncTool {
    type App = HelloApp;
    type Args = FailArgs;

    const NAME: &'static str = "fail_async";
    const DESCRIPTION: &'static str = "Fail asynchronously with a tagged error.";
    const IS_ASYNC: bool = true;

    fn run_async(
        app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
        sink: DynAsyncSink,
    ) -> Result<(), String> {
        let _ = app;
        let _ = ctx;
        let _ = sink;
        match args.tag {
            Some(tag) => Err(format!("{} [tag:{}]", args.message, tag)),
            None => Err(args.message),
        }
    }
}

struct WaitForCancelAsyncTool;

impl DynAomiTool for WaitForCancelAsyncTool {
    type App = HelloApp;
    type Args = WaitForCancelArgs;

    const NAME: &'static str = "wait_for_cancel_async";
    const DESCRIPTION: &'static str = "Emit a waiting event and stay alive until canceled.";
    const IS_ASYNC: bool = true;

    fn run_async(
        app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
        sink: DynAsyncSink,
    ) -> Result<(), String> {
        let _ = app;
        sink.emit(json!({
            "event": "waiting",
            "session_id": ctx.session_id,
            "tool": "wait_for_cancel_async",
            "call_id": ctx.call_id,
            "tag": args.tag,
        }))?;

        while !sink.is_canceled() {
            sleep_if_needed(args.delay_ms.or(Some(5)));
        }

        Ok(())
    }
}

struct PanicSyncTool;

impl DynAomiTool for PanicSyncTool {
    type App = HelloApp;
    type Args = EmptyArgs;

    const NAME: &'static str = "panic_sync";
    const DESCRIPTION: &'static str = "Panic synchronously to test FFI panic containment.";

    fn run(app: &Self::App, _args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        let _ = app;
        panic!("panic_sync triggered for call_id={}", ctx.call_id);
    }
}

dyn_aomi_app!(
    app = HelloApp,
    name = "hello",
    version = "0.1.0",
    preamble = "You are a hello test plugin.",
    tools = [
        GreetTool,
        SlowSyncTool,
        CountAsyncTool,
        FailAsyncTool,
        WaitForCancelAsyncTool,
        PanicSyncTool
    ],
    namespaces = []
);
