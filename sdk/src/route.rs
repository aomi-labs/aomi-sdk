use serde::de::{self, Deserializer};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use crate::builder::{
    AfterStepBuilder, NextRoutesBuilder, NextStepBuilder, RouteBuilder, RouteTarget, host,
};

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
    /// store via the runtime's `RoutedEventBridge` — domains register a
    /// pending tool call when emitting their placeholder, and the runtime
    /// synthesizes a terminal `ToolCompletion` when the matching event
    /// arrives. There's no separate "on async callback" trigger from the
    /// router's perspective; everything resolves through aliases.
    OnBoundEvent { alias: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteStep {
    pub tool: String,
    pub args: Value,
    pub trigger: RouteTrigger,
    /// Alias under which this step's terminal result Value gets stored in the
    /// session's artifact store. Continuations declared with
    /// [`RouteTrigger::OnBoundEvent`] fire when their awaited alias is
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

    pub fn on_return_to<T>(args: Value) -> Self
    where
        T: RouteTarget,
    {
        Self::on_return(T::tool_name(), args)
    }

    pub fn on_bound_event(tool: impl Into<String>, args: Value, alias: impl Into<String>) -> Self {
        Self {
            tool: tool.into(),
            args,
            trigger: RouteTrigger::OnBoundEvent {
                alias: alias.into(),
            },
            bind_as: None,
            prompt: None,
        }
    }

    pub fn on_bound_to<T>(args: Value, alias: impl Into<String>) -> Self
    where
        T: RouteTarget,
    {
        Self::on_bound_event(T::tool_name(), args, alias)
    }

    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Publish this step's terminal result Value under the given alias in
    /// the session's artifact store. Continuations bound via
    /// [`RouteTrigger::OnBoundEvent`] consume the same alias.
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
