use std::collections::BTreeSet;

use serde::Serialize;
use serde_json::Value;

use crate::route::{
    Enforcement, EnforcementPolicy, EnforcementStep, RouteStep, RouteTrigger, ToolReturn,
};
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
    host_target!(CommitTxs, "commit_txs");
    host_target!(CommitEip712, "commit_message");
    host_target!(StageTx, "stage_tx");
    host_target!(SimulateBatch, "simulate_batch");
    host_target!(ViewState, "view_state");
    host_target!(RunTx, "run_tx");
    host_target!(GetTimeAndOnchainContext, "get_time_and_onchain_context");
    host_target!(GetContract, "get_contract");
    host_target!(GetAccountInfo, "get_account_info");
    host_target!(SyncChain, "sync_chain");

    // SVM (Solana) primitives.
    //
    // `SvmSignTx` is the singular sign-only counterpart to `CommitEip712`:
    // takes a fully-built unsigned Solana transaction (base64 versioned/legacy
    // bytes) and returns the signed transaction bytes. The host wallet decodes
    // the tx, prompts the user for approval, signs with the connected SVM
    // wallet, and binds the signed bytes back to the route's awaited alias.
    //
    // Args contract:
    //   { "unsigned_tx": "<base64 serialized solana tx>",
    //     "description": "<human-readable summary for wallet UX>" }
    //
    // Bound artifact (string): the base64 signed tx bytes, ready to be POSTed
    // to whichever venue (e.g. byreal `/dex/v2/send-swap-tx`) is expecting it.
    //
    // Note: there is intentionally no `SignTxsSolana` (plural) — Solana
    // wallets sign one tx per user prompt. Apps that need multiple signed txs
    // should issue separate `SvmSignTx` route steps.
    host_target!(SvmSignTx, "svm_sign_tx");
}

#[derive(Debug, Clone)]
struct DeferredRouteStep {
    step: RouteStep,
    /// Aliases the after-step waits on. `.awaits(a)` pushes one; `.awaits_all([..])`
    /// extends. At `try_build` time: 1 alias → `OnBoundEvent`, 2+ → `OnAllBoundEvents`.
    awaited_aliases: Vec<String>,
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
                    enforcement: None,
                },
                awaited_aliases: Vec::new(),
            });
        }

        AfterStepBuilder { route: self }
    }

    pub fn try_build(mut self) -> Result<ToolReturn, String> {
        let mut aliases = BTreeSet::new();
        let enforced_producer_count = self
            .next_steps
            .iter()
            .filter(|step| step.enforcement.is_some())
            .count();
        if enforced_producer_count > 1 {
            self.errors.push(
                "RouteBuilder v1 supports at most one enforced producer in `next(...)`".to_string(),
            );
        }
        for step in &self.next_steps {
            if let Some(alias) = step.bind_as.as_deref() {
                record_route_alias(&mut aliases, &mut self.errors, alias);
            }
            if let Some(enforcement) = step.enforcement.as_ref() {
                for alias in step_enforcement_aliases(enforcement) {
                    record_route_alias(&mut aliases, &mut self.errors, alias);
                }
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
            }
            if step.enforcement.is_some() && !step.args.is_object() {
                self.errors.push(format!(
                    "enforced producer `{}` must use object args in RouteBuilder v1",
                    step.tool
                ));
            }
        }

        if let Some(after) = self.after_step.as_mut() {
            if after.awaited_aliases.is_empty() {
                self.errors
                    .push("deferred `after(...)` step is missing `.awaits(\"alias\")`".to_string());
                return if self.errors.is_empty() {
                    Ok(ToolReturn::with_routes(self.value, self.next_steps))
                } else {
                    Err(self.errors.join("\n"))
                };
            }

            if !after.step.args.is_object() {
                self.errors.push(format!(
                    "deferred route step `{}` must use object args so the awaited alias can be injected",
                    after.step.tool
                ));
            }
            let mut seen = BTreeSet::new();
            for awaited in &after.awaited_aliases {
                if awaited.trim().is_empty() {
                    self.errors
                        .push("deferred route awaits alias must not be empty".to_string());
                } else if !aliases.contains(awaited) {
                    self.errors.push(format!(
                        "deferred route awaits unknown alias `{awaited}`; produce it in `next(...)` or the attached enforcement first"
                    ));
                } else if !seen.insert(awaited.clone()) {
                    self.errors.push(format!(
                        "deferred route awaits alias `{awaited}` more than once"
                    ));
                }
            }
            after.step.trigger = if after.awaited_aliases.len() == 1 {
                RouteTrigger::OnBoundEvent {
                    alias: after.awaited_aliases.remove(0),
                }
            } else {
                RouteTrigger::OnAllBoundEvents {
                    aliases: std::mem::take(&mut after.awaited_aliases),
                }
            };
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

fn step_enforcement_aliases(enforcement: &Enforcement) -> impl Iterator<Item = &str> {
    enforcement_aliases(enforcement).into_iter()
}

fn record_route_alias(aliases: &mut BTreeSet<String>, errors: &mut Vec<String>, alias: &str) {
    if alias.trim().is_empty() {
        errors.push("bound alias must not be empty".to_string());
    } else if !aliases.insert(alias.to_string()) {
        errors.push(format!("duplicate bound alias `{alias}` in route plan"));
    }
}

fn enforcement_aliases(enforcement: &Enforcement) -> Vec<&str> {
    enforcement
        .steps
        .iter()
        .filter_map(EnforcementStep::bound_alias)
        .collect()
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
    /// Continuations declared via `after(...).awaits(alias)` or
    /// `after(...).awaits_all([..])` consume it.
    ///
    /// Aliases must be unique within a route plan, but the *tool name* does not
    /// have to be — a single plan may have multiple `commit_eip712` / `stage_tx`
    /// / `svm_sign_tx` steps each binding to a distinct alias. The runtime
    /// consumes aliases in FIFO order per tool name, so list the steps in the
    /// order you expect the LLM/user to drive them (use `.note(...)` to
    /// reinforce the order in the suggested-action prompt).
    pub fn bind_as(self, alias: impl Into<String>) -> Self {
        self.route.next_steps[self.index].bind_as = Some(alias.into());
        self
    }

    pub fn note(self, note: impl Into<String>) -> Self {
        self.route.next_steps[self.index].prompt = Some(note.into());
        self
    }

    pub fn enforce(
        self,
        on_failure: EnforcementPolicy,
        f: impl FnOnce(&mut EnforcementBuilder<'_>),
    ) -> Self {
        let mut steps = Vec::new();
        let mut builder = EnforcementBuilder { steps: &mut steps };
        f(&mut builder);
        self.route.next_steps[self.index].enforcement = Some(Enforcement { steps, on_failure });
        self
    }
}

pub struct EnforcementBuilder<'a> {
    steps: &'a mut Vec<EnforcementStep>,
}

impl<'a> EnforcementBuilder<'a> {
    pub fn add<T>(&mut self, args: impl Serialize) -> EnforcementStepBuilder<'_>
    where
        T: RouteTarget,
    {
        self.add_named(T::tool_name(), args)
    }

    pub fn add_named(
        &mut self,
        tool: impl Into<String>,
        args: impl Serialize,
    ) -> EnforcementStepBuilder<'_> {
        let index = self.steps.len();
        self.steps.push(EnforcementStep {
            tool: tool.into(),
            args: serde_json::to_value(args).unwrap_or(Value::Null),
            bind_as: None,
        });
        EnforcementStepBuilder {
            steps: self.steps,
            index,
        }
    }
}

pub struct EnforcementStepBuilder<'a> {
    steps: &'a mut Vec<EnforcementStep>,
    index: usize,
}

impl<'a> EnforcementStepBuilder<'a> {
    /// Publish this enforced step's terminal result Value under the given alias.
    pub fn bind_as(self, alias: impl Into<String>) -> Self {
        self.steps[self.index].bind_as = Some(alias.into());
        self
    }
}

pub struct AfterStepBuilder {
    route: RouteBuilder,
}

impl AfterStepBuilder {
    /// Wait for the named artifact alias produced earlier in this route plan.
    /// Calling this more than once accumulates aliases — the after-step then
    /// fires only when **all** awaited aliases are bound, with each bound
    /// value injected into its args under the alias's name. Equivalent to
    /// `.awaits_all([..])`.
    pub fn awaits(mut self, alias: impl Into<String>) -> Self {
        if let Some(after) = self.route.after_step.as_mut() {
            after.awaited_aliases.push(alias.into());
        }
        self
    }

    /// Wait for **all** of the named aliases. The after-step fires only when
    /// every alias in `aliases` has been bound; each bound value is injected
    /// into the after-step args under its alias name.
    ///
    /// Use this for combined-signing flows (e.g. 0x gasless: Permit2
    /// approval + Settler metaTransaction trade) where the submit tool needs
    /// every signature in one call.
    pub fn awaits_all<I, S>(mut self, aliases: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if let Some(after) = self.route.after_step.as_mut() {
            after
                .awaited_aliases
                .extend(aliases.into_iter().map(Into::into));
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
    use crate::route::{EnforcementPolicy, RouteStep, ToolReturn};
    use crate::{DynAomiApp, DynAomiTool, DynToolCallCtx};
    use serde_json::{json, Value};

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
    fn route_builder_serializes_solana_sign_plan() {
        // Mirror of `route_builder_serializes_bound_artifact_plan` but for
        // the SVM (Solana) sign-only flow: app builds an unsigned tx, host
        // signs via SvmSignTx, the bound `signed_tx` artifact then feeds
        // into the submit step which forwards the signed bytes upstream.
        let tool_return = ToolReturn::route(json!({"status": "awaiting_wallet"}))
            .next(|next| {
                next.add::<host::SvmSignTx>(json!({
                    "unsigned_tx": "AgAB...base64...",
                    "description": "Swap 1 USDC for 0.005 SOL via byreal RFQ",
                }))
                .bind_as("signed_tx")
                .note("sign this Solana swap");
            })
            .after::<SubmitOrder>(json!({"venue": "byreal-rfq"}))
            .awaits("signed_tx")
            .note("submit signed tx to venue")
            .build();

        let serialized = serde_json::to_value(&tool_return).unwrap();
        assert_eq!(
            serialized,
            json!({
                "__aomi_tool_return": true,
                "__aomi_tool_value": {"status": "awaiting_wallet"},
                "__aomi_tool_routes": [
                    {
                        "tool": "svm_sign_tx",
                        "args": {
                            "unsigned_tx": "AgAB...base64...",
                            "description": "Swap 1 USDC for 0.005 SOL via byreal RFQ",
                        },
                        "trigger": {"type": "on_sync_return"},
                        "bind_as": "signed_tx",
                        "prompt": "sign this Solana swap",
                    },
                    {
                        "tool": "submit_order",
                        "args": {"venue": "byreal-rfq"},
                        "trigger": {
                            "type": "on_bound_event",
                            "alias": "signed_tx",
                        },
                        "prompt": "submit signed tx to venue",
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
    fn route_builder_enforcement_can_satisfy_awaited_alias() {
        let tool_return = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<host::StageTx>(json!({"to": "0x1", "data": {"raw": "0x"}}))
                    .enforce(EnforcementPolicy::Stop, |enforce| {
                        enforce.add::<host::SimulateBatch>(json!({}));
                        enforce
                            .add::<host::CommitTxs>(json!({"aa_preference": "auto"}))
                            .bind_as("transaction_hash");
                    });
            })
            .after::<SubmitOrder>(json!({"quote_id": "quote-1"}))
            .awaits("transaction_hash")
            .build();

        assert_eq!(tool_return.routes[0].bind_as, None);
        assert!(tool_return.routes[0].enforcement.is_some());
        assert!(matches!(
            &tool_return.routes[1].trigger,
            RouteTrigger::OnBoundEvent { alias } if alias == "transaction_hash"
        ));
    }

    #[test]
    fn route_builder_rejects_multiple_enforced_producers() {
        let err = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<host::StageTx>(json!({"to": "0x1"}))
                    .enforce(EnforcementPolicy::Stop, |_| {});
                next.add::<host::StageTx>(json!({"to": "0x2"}))
                    .enforce(EnforcementPolicy::Stop, |_| {});
            })
            .try_build()
            .expect_err("multiple enforced producers should fail");

        assert!(err.contains("at most one enforced producer"));
    }

    #[test]
    fn route_builder_rejects_non_object_enforced_producer_args() {
        let err = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<host::StageTx>(json!(["not", "an", "object"]))
                    .enforce(EnforcementPolicy::Stop, |_| {});
            })
            .try_build()
            .expect_err("non-object enforced producer args should fail");

        assert!(err.contains("must use object args"));
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

    #[test]
    fn route_builder_rejects_empty_bound_aliases() {
        let err = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<SyncTool>(json!({"x": 1})).bind_as("   ");
            })
            .try_build()
            .expect_err("empty bound aliases should fail");

        assert!(err.contains("bound alias must not be empty"));
    }

    #[test]
    fn route_builder_rejects_empty_awaited_aliases() {
        let err = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<SyncTool>(json!({"x": 1})).bind_as("artifact");
            })
            .after::<SubmitOrder>(json!({}))
            .awaits(" ")
            .try_build()
            .expect_err("empty awaited aliases should fail");

        assert!(err.contains("awaits alias must not be empty"));
    }

    #[test]
    fn route_builder_allows_repeated_tool_with_distinct_binds() {
        // Lifts the prior "bound producers must have unique tool names"
        // constraint. Two `commit_eip712` steps in one plan, each binding to a
        // distinct alias, is the combined approve+trade shape used by 0x
        // gasless (Permit2 + Settler) and by Permit2-style flows for other
        // DEX integrations.
        let tool_return = ToolReturn::route(json!({"status": "awaiting_wallet"}))
            .next(|next| {
                next.add::<host::CommitEip712>(json!({"typed_data": {"approval": true}}))
                    .bind_as("approval")
                    .note("Sign the Permit2 approval first.");
                next.add::<host::CommitEip712>(json!({"typed_data": {"trade": true}}))
                    .bind_as("trade")
                    .note("Then sign the gasless trade.");
            })
            .after::<SubmitOrder>(json!({"chain_id": 1}))
            .awaits_all(["approval", "trade"])
            .build();

        assert_eq!(tool_return.routes.len(), 3);
        assert_eq!(tool_return.routes[0].tool, "commit_eip712");
        assert_eq!(tool_return.routes[0].bind_as.as_deref(), Some("approval"));
        assert_eq!(tool_return.routes[1].tool, "commit_eip712");
        assert_eq!(tool_return.routes[1].bind_as.as_deref(), Some("trade"));
        match &tool_return.routes[2].trigger {
            RouteTrigger::OnAllBoundEvents { aliases } => {
                assert_eq!(aliases, &vec!["approval".to_string(), "trade".to_string()]);
            }
            other => panic!("expected OnAllBoundEvents, got {other:?}"),
        }
    }

    #[test]
    fn route_builder_awaits_called_twice_upgrades_to_multi_alias() {
        // Sugar: `.awaits(a).awaits(b)` is the same as `.awaits_all([a, b])`.
        let tool_return = ToolReturn::route(json!({"status": "awaiting_wallet"}))
            .next(|next| {
                next.add::<host::CommitEip712>(json!({"typed_data": {"a": true}}))
                    .bind_as("approval");
                next.add::<host::CommitEip712>(json!({"typed_data": {"t": true}}))
                    .bind_as("trade");
            })
            .after::<SubmitOrder>(json!({"chain_id": 1}))
            .awaits("approval")
            .awaits("trade")
            .build();

        match &tool_return.routes[2].trigger {
            RouteTrigger::OnAllBoundEvents { aliases } => {
                assert_eq!(aliases, &vec!["approval".to_string(), "trade".to_string()]);
            }
            other => panic!("expected OnAllBoundEvents, got {other:?}"),
        }
    }

    #[test]
    fn route_builder_single_awaits_still_uses_on_bound_event() {
        // Existing single-alias semantics are unchanged: one `.awaits(..)`
        // call produces `OnBoundEvent`, not `OnAllBoundEvents`. This keeps the
        // wire shape stable for every flow that worked before the multi-alias
        // extension landed.
        let tool_return = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<host::CommitEip712>(json!({"typed_data": {}}))
                    .bind_as("signature");
            })
            .after::<SubmitOrder>(json!({}))
            .awaits("signature")
            .build();

        match &tool_return.routes[1].trigger {
            RouteTrigger::OnBoundEvent { alias } => assert_eq!(alias, "signature"),
            other => panic!("expected OnBoundEvent, got {other:?}"),
        }
    }

    #[test]
    fn route_builder_rejects_unknown_alias_in_awaits_all() {
        let err = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<host::CommitEip712>(json!({"typed_data": {}}))
                    .bind_as("approval");
            })
            .after::<SubmitOrder>(json!({}))
            .awaits_all(["approval", "missing"])
            .try_build()
            .expect_err("unknown alias in awaits_all should fail");

        assert!(err.contains("awaits unknown alias `missing`"));
    }

    #[test]
    fn route_builder_rejects_duplicate_awaited_aliases() {
        let err = ToolReturn::route(json!({"status": "ok"}))
            .next(|next| {
                next.add::<host::CommitEip712>(json!({"typed_data": {}}))
                    .bind_as("approval");
            })
            .after::<SubmitOrder>(json!({}))
            .awaits_all(["approval", "approval"])
            .try_build()
            .expect_err("duplicate awaited aliases should fail");

        assert!(err.contains("more than once"));
    }
}
