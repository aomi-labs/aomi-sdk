use aomi_sdk::{
    DynAomiTool, DynAsyncSink, DynToolCallCtx, dyn_aomi_app,
    schemars::JsonSchema,
    serde_json::{Value, json},
};
use serde::Deserialize;

#[derive(Clone, Default)]
struct ParityApp;

#[derive(Debug, Deserialize, JsonSchema)]
struct GreetArgs {
    name: String,
}

struct Greet;

impl DynAomiTool for Greet {
    type App = ParityApp;
    type Args = GreetArgs;

    const NAME: &'static str = "greet";
    const DESCRIPTION: &'static str = "Return a deterministic greeting.";

    fn run(_app: &Self::App, args: Self::Args, ctx: DynToolCallCtx) -> Result<Value, String> {
        Ok(json!({
            "greeting": format!("Hello, {}!", args.name),
            "session_id": ctx.session_id,
            "call_id": ctx.call_id,
        }))
    }
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CountArgs {
    upto: u64,
}

struct Count;

impl DynAomiTool for Count {
    type App = ParityApp;
    type Args = CountArgs;

    const NAME: &'static str = "count";
    const DESCRIPTION: &'static str = "Emit deterministic count updates.";
    const IS_ASYNC: bool = true;

    fn run_async(
        _app: &Self::App,
        args: Self::Args,
        ctx: DynToolCallCtx,
        sink: DynAsyncSink,
    ) -> Result<(), String> {
        for count in 1..=args.upto {
            let update = json!({
                "count": count,
                "session_id": ctx.session_id,
                "call_id": ctx.call_id,
            });
            if count == args.upto {
                sink.complete(update)?;
            } else {
                sink.emit(update)?;
            }
        }
        Ok(())
    }
}

dyn_aomi_app!(
    app = ParityApp,
    name = "wasm-parity",
    version = "0.1.0",
    preamble = "Exercise the native and WebAssembly SDK contracts.",
    tools = [Greet, Count],
    namespaces = []
);
