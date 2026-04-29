use serde::de::{self, Deserializer};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::types::DynAomiTool;

pub const TOOL_RETURN_MARKER: &str = "__aomi_tool_return";
pub const TOOL_RETURN_VALUE_KEY: &str = "__aomi_tool_value";
pub const TOOL_RETURN_ROUTES_KEY: &str = "__aomi_tool_routes";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RouteTrigger {
    /// Fires inline after the emitting tool's `run` returns. The host renders
    /// these as the next system prompt the LLM sees in the same completion
    /// cycle.
    OnSyncReturn,
    /// Fires when an earlier immediate step in the same route plan binds a
    /// named artifact under `alias`. Out-of-band events (wallet callbacks,
    /// game state updates, exec completions) feed into the same artifact
    /// store via the runtime's `PendingEventBridge` — domains register a
    /// pending tool call when emitting their placeholder, and the runtime
    /// synthesizes a terminal `ToolCompletion` when the matching event
    /// arrives. There's no separate "on async callback" trigger from the
    /// router's perspective; everything resolves through aliases.
    OnBoundArtifact { alias: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteStep {
    pub tool: String,
    pub args: Value,
    pub trigger: RouteTrigger,
    /// Alias under which this step's terminal result Value gets stored in the
    /// session's artifact store. Continuations declared with
    /// [`RouteTrigger::OnBoundArtifact`] fire when their awaited alias is
    /// bound. The router is purely alias-keyed — no per-domain typing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bind_as: Option<String>,
    /// Literal text the host injects into the LLM context when this step
    /// fires. When `None`, the host renders a default template per
    /// [`RouteTrigger`] variant. Apps override this when they need a specific
    /// voice (e.g. "preserve args exactly" vs "call only if still desired").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
}

impl RouteStep {
    pub fn on_return(tool: impl Into<String>, args: Value) -> Self {
        Self {
            tool: tool.into(),
            args,
            trigger: RouteTrigger::OnSyncReturn,
            bind_as: None,
            prompt: None,
        }
    }

    pub fn on_bound_artifact(
        tool: impl Into<String>,
        args: Value,
        alias: impl Into<String>,
    ) -> Self {
        Self {
            tool: tool.into(),
            args,
            trigger: RouteTrigger::OnBoundArtifact {
                alias: alias.into(),
            },
            bind_as: None,
            prompt: None,
        }
    }

    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Publish this step's terminal result Value under the given alias in
    /// the session's artifact store. Continuations bound via
    /// [`RouteTrigger::OnBoundArtifact`] consume the same alias.
    pub fn bind_as(mut self, alias: impl Into<String>) -> Self {
        self.bind_as = Some(alias.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolReturn {
    pub value: Value,
    pub routes: Vec<RouteStep>,
}

impl ToolReturn {
    pub fn value(value: impl Serialize) -> Self {
        Self {
            value: serde_json::to_value(value).unwrap_or(Value::Null),
            routes: Vec::new(),
        }
    }

    pub fn with_route(value: impl Serialize, route: RouteStep) -> Self {
        Self::with_routes(value, [route])
    }

    pub fn with_routes(value: impl Serialize, routes: impl IntoIterator<Item = RouteStep>) -> Self {
        Self {
            value: serde_json::to_value(value).unwrap_or(Value::Null),
            routes: routes.into_iter().collect(),
        }
    }

    pub fn route(value: impl Serialize) -> RouteBuilder {
        RouteBuilder::new(serde_json::to_value(value).unwrap_or(Value::Null))
    }

    pub fn has_routes(&self) -> bool {
        !self.routes.is_empty()
    }

    pub fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    pub fn from_value(value: Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(value)
    }
}

impl Serialize for ToolReturn {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if self.routes.is_empty() {
            return self.value.serialize(serializer);
        }

        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry(TOOL_RETURN_MARKER, &true)?;
        map.serialize_entry(TOOL_RETURN_VALUE_KEY, &self.value)?;
        map.serialize_entry(TOOL_RETURN_ROUTES_KEY, &self.routes)?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for ToolReturn {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Object(mut obj)
                if obj
                    .get(TOOL_RETURN_MARKER)
                    .and_then(Value::as_bool)
                    .unwrap_or(false) =>
            {
                let value = obj.remove(TOOL_RETURN_VALUE_KEY).unwrap_or(Value::Null);
                let routes = match obj.remove(TOOL_RETURN_ROUTES_KEY) {
                    Some(routes) => serde_json::from_value(routes).map_err(de::Error::custom)?,
                    None => Vec::new(),
                };
                Ok(Self { value, routes })
            }
            other => Ok(Self {
                value: other,
                routes: Vec::new(),
            }),
        }
    }
}

impl From<Value> for ToolReturn {
    fn from(value: Value) -> Self {
        Self {
            value,
            routes: Vec::new(),
        }
    }
}

/// Type-level convenience for naming a routed target tool. The blanket impl
/// over [`DynAomiTool`] means an app's own tools auto-qualify; the
/// `add::<MyTool>(...)` / `after::<MyTool>(...)` builder methods simply
/// inline `MyTool::NAME` so callers don't have to repeat the string.
///
/// This trait carries no wallet- or domain-specific knowledge. Tools that
/// aren't statically typed at the call site (e.g. host-provided
/// `commit_eip712`, `stage_tx`, `commit_tx`) are referenced by string via
/// `add_named` / `after_named` instead.
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
    fn new(value: Value) -> Self {
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
                    trigger: RouteTrigger::OnBoundArtifact {
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
            after.step.trigger = RouteTrigger::OnBoundArtifact { alias };
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
    use super::*;
    use crate::{DynAomiApp, DynAomiTool, DynToolCallCtx};
    use serde_json::json;

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
        let tool_return = ToolReturn::with_route(
            json!({"status": "awaiting_wallet"}),
            RouteStep::on_async_callback::<WalletEip712Complete>(
                "submit_polymarket_order",
                json!({"market": "btc"}),
            )
            .callback_field("clob_l1_signature"),
        );

        let serialized = serde_json::to_value(&tool_return).unwrap();
        assert_eq!(
            serialized,
            json!({
                "__aomi_tool_return": true,
                "__aomi_tool_value": {"status": "awaiting_wallet"},
                "__aomi_tool_routes": [{
                    "tool": "submit_polymarket_order",
                    "args": {"market": "btc"},
                    "trigger": {
                        "type": "on_async_callback",
                        "kind": "wallet_eip712_response",
                        "callback_field": "clob_l1_signature",
                    },
                }],
            })
        );

        let roundtrip = ToolReturn::from_value(serialized).unwrap();
        assert!(roundtrip.has_routes());
        assert_eq!(roundtrip.routes.len(), 1);
    }

    #[test]
    fn on_async_callback_uses_trait_id_for_kind() {
        let tx_step = RouteStep::on_async_callback::<WalletTxComplete>("submit", json!({}));
        match tx_step.trigger {
            RouteTrigger::OnAsyncCallback { kind, .. } => {
                assert_eq!(kind, WalletTxComplete::NAME);
                assert_eq!(kind, "wallet:tx_complete");
            }
            _ => panic!("expected OnAsyncCallback"),
        }
    }

    #[test]
    fn route_builder_serializes_bound_artifact_plan() {
        let tool_return = ToolReturn::route(json!({"status": "awaiting_wallet"}))
            .next(|next| {
                // Host-provided wallet tools are referenced by string —
                // the SDK doesn't enumerate them or attach domain typing.
                next.add_named("commit_eip712", json!({"typed_data": {}}))
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
                            "type": "on_bound_artifact",
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
                next.add_named("commit_eip712", json!({"typed_data": {}}))
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
