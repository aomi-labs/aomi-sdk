use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;
use serde_json::Value;

use crate::route::{RouteStep, RouteTrigger, ToolReturn};
use crate::types::DynAomiTool;

/// Type-level convenience for naming a routed target tool. The blanket impl
/// over [`DynAomiTool`] means an app's own tools auto-qualify; the
/// `add::<MyTool>(...)` / `after::<MyTool>(...)` builder methods simply
/// inline `MyTool::NAME` so callers don't have to repeat the string.
///
/// Stable host-provided tools can use marker types from [`host`]. Truly
/// dynamic tool names should still go through `add_named` / `after_named`.
pub trait RouteTarget {
    fn tool_name() -> &'static str;
}

impl<T> RouteTarget for T
where
    T: DynAomiTool,
{
    fn tool_name() -> &'static str {
        T::NAME
    }
}

/// Canonical marker targets for stable host-provided tools in the `common`
/// namespace. These are name-only wrappers for the route builder; they do not
/// model args or tool behavior.
pub mod host {
    use super::RouteTarget;

    macro_rules! host_target {
        ($name:ident, $tool:literal) => {
            #[derive(Debug, Clone, Copy, Default)]
            pub struct $name;

            impl RouteTarget for $name {
                fn tool_name() -> &'static str {
                    $tool
                }
            }
        };
    }

    host_target!(BraveSearch, "brave_search");
    host_target!(CommitTx, "commit_tx");
    host_target!(CommitEip712, "commit_eip712");
    host_target!(StageTx, "stage_tx");
    host_target!(SimulateBatch, "simulate_batch");
    host_target!(ViewState, "view_state");
    host_target!(RunTx, "run_tx");
    host_target!(GetTimeAndOnchainContext, "get_time_and_onchain_context");
    host_target!(GetContract, "get_contract");
    host_target!(GetAccountInfo, "get_account_info");
    host_target!(SyncChain, "sync_chain");
}

#[derive(Debug, Clone)]
struct DeferredRouteStep {
    step: RouteStep,
    awaited_alias: Option<String>,
}

pub struct RouteBuilder {
    value: Value,
    next_steps: Vec<RouteStep>,
    after_step: Option<DeferredRouteStep>,
    errors: Vec<String>,
}

impl RouteBuilder {
    pub(super) fn new(value: Value) -> Self {
        Self {
            value,
            next_steps: Vec::new(),
            after_step: None,
            errors: Vec::new(),
        }
    }

    pub fn next(mut self, f: impl FnOnce(&mut NextRoutesBuilder<'_>)) -> Self {
        let mut next = NextRoutesBuilder { route: &mut self };
        f(&mut next);
        self
    }

    pub fn after<T>(self, args: impl Serialize) -> AfterStepBuilder
    where
        T: RouteTarget,
    {
        self.after_named(T::tool_name(), args)
    }

    pub fn after_named(
        mut self,
        tool: impl Into<String>,
        args: impl Serialize,
    ) -> AfterStepBuilder {
        if self.after_step.is_some() {
            self.errors
                .push("RouteBuilder v1 supports at most one deferred `after` step".to_string());
        } else {
            self.after_step = Some(DeferredRouteStep {
                step: RouteStep {
                    tool: tool.into(),
                    args: serde_json::to_value(args).unwrap_or(Value::Null),
                    trigger: RouteTrigger::OnBoundEvent {
                        alias: String::new(),
                    },
                    bind_as: None,
                    prompt: None,
                },
                awaited_alias: None,
            });
        }

        AfterStepBuilder { route: self }
    }

    pub fn try_build(mut self) -> Result<ToolReturn, String> {
        let mut aliases = BTreeSet::new();
        let mut tool_counts: BTreeMap<&str, usize> = BTreeMap::new();
        for step in &self.next_steps {
            *tool_counts.entry(step.tool.as_str()).or_default() += 1;
            if let Some(alias) = step.bind_as.as_deref()
                && !aliases.insert(alias.to_string())
            {
                self.errors
                    .push(format!("duplicate bound alias `{alias}` in route plan"));
            }
        }

        for step in &self.next_steps {
            if let Some(alias) = step.bind_as.as_deref() {
                if !matches!(step.trigger, RouteTrigger::OnSyncReturn) {
                    self.errors.push(format!(
                        "bound artifact alias `{alias}` must be attached to an immediate `next` step"
                    ));
                }
                if !step.args.is_object() {
                    self.errors.push(format!(
                        "bound artifact producer `{}` must use object args in RouteBuilder v1",
                        step.tool
                    ));
                }
                if tool_counts
                    .get(step.tool.as_str())
                    .copied()
                    .unwrap_or_default()
                    > 1
                {
                    self.errors.push(format!(
                        "tool `{}` appears more than once in `next(...)`; bound producers must have unique tool names in RouteBuilder v1",
                        step.tool
                    ));
                }
            }
        }

        if let Some(after) = self.after_step.as_mut() {
            let Some(alias) = after.awaited_alias.clone() else {
                self.errors
                    .push("deferred `after(...)` step is missing `.awaits(\"alias\")`".to_string());
                return if self.errors.is_empty() {
                    Ok(ToolReturn::with_routes(self.value, self.next_steps))
                } else {
                    Err(self.errors.join("\n"))
                };
            };

            if !after.step.args.is_object() {
                self.errors.push(format!(
                    "deferred route step `{}` must use object args so the awaited alias can be injected",
                    after.step.tool
                ));
            }
            if !aliases.contains(&alias) {
                self.errors.push(format!(
                    "deferred route awaits unknown alias `{alias}`; bind it in `next(...)` first"
                ));
            }
            after.step.trigger = RouteTrigger::OnBoundEvent { alias };
        }

        if !self.errors.is_empty() {
            return Err(self.errors.join("\n"));
        }

        let mut routes = self.next_steps;
        if let Some(after) = self.after_step {
            routes.push(after.step);
        }
        Ok(ToolReturn::with_routes(self.value, routes))
    }

    pub fn build(self) -> ToolReturn {
        self.try_build()
            .unwrap_or_else(|err| panic!("invalid RouteBuilder plan: {err}"))
    }
}

pub struct NextRoutesBuilder<'a> {
    route: &'a mut RouteBuilder,
}

impl<'a> NextRoutesBuilder<'a> {
    pub fn add<T>(&mut self, args: impl Serialize) -> NextStepBuilder<'_>
    where
        T: RouteTarget,
    {
        self.push_step(T::tool_name(), args)
    }

    pub fn add_named(
        &mut self,
        tool: impl Into<String>,
        args: impl Serialize,
    ) -> NextStepBuilder<'_> {
        self.push_step(tool, args)
    }

    fn push_step(&mut self, tool: impl Into<String>, args: impl Serialize) -> NextStepBuilder<'_> {
        let index = self.route.next_steps.len();
        self.route.next_steps.push(RouteStep::on_return(
            tool.into(),
            serde_json::to_value(args).unwrap_or(Value::Null),
        ));
        NextStepBuilder {
            route: self.route,
            index,
        }
    }
}

pub struct NextStepBuilder<'a> {
    route: &'a mut RouteBuilder,
    index: usize,
}

impl<'a> NextStepBuilder<'a> {
    /// Publish this step's terminal result Value under the given alias.
    /// Continuations declared via `after(...).awaits(alias)` consume it.
    pub fn bind_as(self, alias: impl Into<String>) -> Self {
        self.route.next_steps[self.index].bind_as = Some(alias.into());
        self
    }

    pub fn note(self, note: impl Into<String>) -> Self {
        self.route.next_steps[self.index].prompt = Some(note.into());
        self
    }
}

pub struct AfterStepBuilder {
    route: RouteBuilder,
}

impl AfterStepBuilder {
    pub fn awaits(mut self, alias: impl Into<String>) -> Self {
        if let Some(after) = self.route.after_step.as_mut() {
            after.awaited_alias = Some(alias.into());
        }
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        if let Some(after) = self.route.after_step.as_mut() {
            after.step.prompt = Some(note.into());
        }
        self
    }

    pub fn next(mut self, f: impl FnOnce(&mut NextRoutesBuilder<'_>)) -> RouteBuilder {
        let mut next = NextRoutesBuilder {
            route: &mut self.route,
        };
        f(&mut next);
        self.route
    }

    pub fn build(self) -> ToolReturn {
        self.route.build()
    }

    pub fn try_build(self) -> Result<ToolReturn, String> {
        self.route.try_build()
    }
}

#[cfg(test)]
mod tests {
    use crate::route::{RouteStep, ToolReturn};
    use crate::{DynAomiApp, DynAomiTool, DynToolCallCtx};
    use serde_json::{Value, json};

    use super::*;

    #[derive(Clone, Default)]
    struct App;

    impl DynAomiApp for App {
        fn name(&self) -> &'static str {
            "test"
        }

        fn version(&self) -> &'static str {
            "0.1.0"
        }

        fn preamble(&self) -> &'static str {
            "test"
        }

        fn tools(&self) -> Vec<crate::DynToolMetadata> {
            Vec::new()
        }

        fn start_tool(
            &self,
            _name: &str,
            _args_json: &str,
            _ctx_json: &str,
            _sink: crate::DynAsyncSink,
        ) -> crate::DynToolDispatch {
            unreachable!()
        }
    }

    struct SubmitOrder;
    impl DynAomiTool for SubmitOrder {
        type App = App;
        type Args = serde_json::Value;

        const NAME: &'static str = "submit_order";
        const DESCRIPTION: &'static str = "submit";

        fn run(_app: &App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
            Ok(Value::Null)
        }
    }

    struct SyncTool;
    impl DynAomiTool for SyncTool {
        type App = App;
        type Args = serde_json::Value;

        const NAME: &'static str = "sync_tool";
        const DESCRIPTION: &'static str = "sync";

        fn run(_app: &App, _args: Self::Args, _ctx: DynToolCallCtx) -> Result<Value, String> {
            Ok(Value::Null)
        }
    }

    struct AsyncTool;
    impl DynAomiTool for AsyncTool {
        type App = App;
        type Args = serde_json::Value;

        const NAME: &'static str = "async_tool";
        const DESCRIPTION: &'static str = "async";
        const IS_ASYNC: bool = true;
    }

    #[test]
    fn plain_tool_return_serializes_to_the_raw_value() {
        let tool_return = ToolReturn::value(json!({"ok": true}));
        let serialized = serde_json::to_value(&tool_return).unwrap();
        assert_eq!(serialized, json!({"ok": true}));

        let roundtrip = ToolReturn::from_value(serialized).unwrap();
        assert_eq!(roundtrip.value, json!({"ok": true}));
        assert!(roundtrip.routes.is_empty());
    }

    #[test]
    fn routed_tool_return_serializes_to_an_envelope() {
        // OnBoundEvent-based plan: a producer step binds an alias and
        // a deferred consumer awaits it. This is the canonical wire shape
        // after the wallet-callback machinery was removed.
        let tool_return = ToolReturn::with_routes(
            json!({"status": "awaiting_wallet"}),
            [
                RouteStep::on_return("commit_eip712", json!({"typed_data": {}}))
                    .bind_as("clob_l1_signature"),
                RouteStep::on_bound_event(
                    "submit_polymarket_order",
                    json!({"market": "btc"}),
                    "clob_l1_signature",
                ),
            ],
        );

        let serialized = serde_json::to_value(&tool_return).unwrap();
        assert_eq!(
            serialized,
            json!({
                "__aomi_tool_return": true,
                "__aomi_tool_value": {"status": "awaiting_wallet"},
                "__aomi_tool_routes": [
                    {
                        "tool": "commit_eip712",
                        "args": {"typed_data": {}},
                        "trigger": {"type": "on_sync_return"},
                        "bind_as": "clob_l1_signature",
                    },
                    {
                        "tool": "submit_polymarket_order",
                        "args": {"market": "btc"},
                        "trigger": {
                            "type": "on_bound_event",
                            "alias": "clob_l1_signature",
                        },
                    }
                ],
            })
        );

        let roundtrip = ToolReturn::from_value(serialized).unwrap();
        assert!(roundtrip.has_routes());
        assert_eq!(roundtrip.routes.len(), 2);
    }

    #[test]
    fn route_builder_serializes_bound_artifact_plan() {
        let tool_return = ToolReturn::route(json!({"status": "awaiting_wallet"}))
            .next(|next| {
                // Stable host-provided tools can use typed route targets too.
                next.add::<host::CommitEip712>(json!({"typed_data": {}}))
                    .bind_as("clob_l1_signature")
                    .note("sign this first");
            })
            .after::<SubmitOrder>(json!({"market": "btc"}))
            .awaits("clob_l1_signature")
            .note("continue submit")
            .build();

        let serialized = serde_json::to_value(&tool_return).unwrap();
        assert_eq!(
            serialized,
            json!({
                "__aomi_tool_return": true,
                "__aomi_tool_value": {"status": "awaiting_wallet"},
                "__aomi_tool_routes": [
                    {
                        "tool": "commit_eip712",
                        "args": {"typed_data": {}},
                        "trigger": {"type": "on_sync_return"},
                        "bind_as": "clob_l1_signature",
                        "prompt": "sign this first",
                    },
                    {
                        "tool": "submit_order",
                        "args": {"market": "btc"},
                        "trigger": {
                            "type": "on_bound_event",
                            "alias": "clob_l1_signature",
                        },
                        "prompt": "continue submit",
                    }
                ]
            })
        );
    }

    #[test]
    fn route_builder_bind_as_works_for_any_tool() {
        // The router is alias-keyed: any tool can bind_as. There's no
        // per-tool eligibility check and no artifact-kind enum.
        let tool_return = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<SyncTool>(json!({"x": 1})).bind_as("tool_result");
            })
            .after::<SubmitOrder>(json!({}))
            .awaits("tool_result")
            .build();

        assert_eq!(
            tool_return.routes[0].bind_as.as_deref(),
            Some("tool_result")
        );
    }

    #[test]
    fn route_builder_async_tool_can_bind_as() {
        // Async tools' terminal completions land via the runtime's pending
        // event bridge; from the router's perspective they're just steps
        // that produce a Value. No SDK-side eligibility check.
        let tool_return = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<AsyncTool>(json!({"x": 1})).bind_as("from_async");
            })
            .after::<SubmitOrder>(json!({}))
            .awaits("from_async")
            .build();

        assert_eq!(tool_return.routes[0].bind_as.as_deref(), Some("from_async"));
    }

    #[test]
    fn route_builder_rejects_unknown_awaited_alias() {
        let err = ToolReturn::route(json!({"status": "ok"}))
            .after::<SubmitOrder>(json!({}))
            .awaits("missing_alias")
            .try_build()
            .expect_err("missing awaited alias should fail");

        assert!(err.contains("awaits unknown alias `missing_alias`"));
    }

    #[test]
    fn route_builder_rejects_duplicate_aliases() {
        let err = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<host::CommitEip712>(json!({"typed_data": {}}))
                    .bind_as("dup");
                next.add::<SyncTool>(json!({"x": 1})).bind_as("dup");
            })
            .after::<SubmitOrder>(json!({}))
            .awaits("dup")
            .try_build()
            .expect_err("duplicate aliases should fail");

        assert!(err.contains("duplicate bound alias `dup`"));
    }
}
