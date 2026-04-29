use serde::de::{self, Deserializer};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const TOOL_RETURN_MARKER: &str = "__aomi_tool_return";
pub const TOOL_RETURN_VALUE_KEY: &str = "__aomi_tool_value";
pub const TOOL_RETURN_ROUTES_KEY: &str = "__aomi_tool_routes";

/// Marker trait identifying an out-of-band callback kind that a [`RouteStep`]
/// can wait on. Each impl declares a stable wire `ID` matching the callback's
/// `type` field on the host event stream. Adding new callback kinds (e.g.
/// timer expiry, oracle update) is purely additive — define a new unit struct,
/// `impl AsyncCallback`, and add a runtime matcher arm.
pub trait AsyncCallback: Send + Sync + 'static {
    /// Stable wire identifier for this callback kind. The host matches this
    /// against the incoming callback's `type` field.
    const NAME: &'static str;
}

/// EVM transaction completion (signed/broadcast) — payload includes
/// `transaction_hash`.
pub struct WalletTxComplete;
impl AsyncCallback for WalletTxComplete {
    const NAME: &'static str = "wallet:tx_complete";
}

/// EIP-712 typed-data signature completion — payload includes `signature`.
pub struct WalletEip712Complete;
impl AsyncCallback for WalletEip712Complete {
    const NAME: &'static str = "wallet_eip712_response";
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RouteTrigger {
    /// Fires inline after the emitting tool's `run` returns. The host renders
    /// these as the next system prompt the LLM sees in the same completion
    /// cycle.
    OnSyncReturn,
    /// Fires when the host receives a matching async callback (currently:
    /// wallet events). The `kind` string equals the [`AsyncCallback::ID`] of
    /// the callback type the route waits on.
    OnAsyncCallback {
        /// [`AsyncCallback::ID`] of the awaited callback kind.
        kind: String,
        /// Optional named field to splice from the callback payload into the
        /// next call's args. When `None`, the host uses the default for the
        /// callback kind (`signature` for EIP-712, `transaction_hash` for tx).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        callback_field: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteStep {
    pub tool: String,
    pub args: Value,
    pub trigger: RouteTrigger,
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
            prompt: None,
        }
    }

    /// Bind this step to a typed [`AsyncCallback`] kind. Future callback
    /// kinds (timers, oracles, custom plugin events) plug in here by
    /// adding an `impl AsyncCallback` and a host-side matcher arm.
    pub fn on_async_callback<C: AsyncCallback>(tool: impl Into<String>, args: Value) -> Self {
        Self {
            tool: tool.into(),
            args,
            trigger: RouteTrigger::OnAsyncCallback {
                kind: C::NAME.to_string(),
                callback_field: None,
            },
            prompt: None,
        }
    }

    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    pub fn callback_field(mut self, callback_field: impl Into<String>) -> Self {
        if let RouteTrigger::OnAsyncCallback {
            callback_field: slot,
            ..
        } = &mut self.trigger
        {
            *slot = Some(callback_field.into());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
}
