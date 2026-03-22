//! GameFi dynamic plugin — Molinar 3D world bot agent.

use aomi_sdk::schemars::JsonSchema;
use aomi_sdk::*;
use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Clone, Default)]
pub(crate) struct MolinarApp;

pub(crate) use crate::tool::*;

// ============================================================================
// Molinar Client (blocking)
// ============================================================================

pub(crate) const DEFAULT_MOLINAR_API: &str = "https://molinar.ai/api/bot";

#[derive(Clone)]
pub struct MolinarClient {
    pub(crate) http: reqwest::blocking::Client,
    pub(crate) api_endpoint: String,
}

impl MolinarClient {
    pub fn new() -> Result<Self, String> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("failed to build HTTP client: {e}"))?;
        Ok(Self {
            http,
            api_endpoint: std::env::var("MOLINAR_API_ENDPOINT")
                .unwrap_or_else(|_| DEFAULT_MOLINAR_API.to_string()),
        })
    }

    pub fn get_json(&self, url: &str, op: &str) -> Result<Value, String> {
        let response = self
            .http
            .get(url)
            .send()
            .map_err(|e| format!("[molinar] {op} request failed ({url}): {e}"))?;

        let status = response.status();
        let body = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[molinar] {op} failed ({url}): {status} {body}"));
        }

        serde_json::from_str::<Value>(&body)
            .map_err(|e| format!("[molinar] {op} decode failed ({url}): {e}"))
    }

    pub fn post_json(&self, url: &str, body: &Value, op: &str) -> Result<Value, String> {
        let response = self
            .http
            .post(url)
            .json(body)
            .send()
            .map_err(|e| format!("[molinar] {op} request failed ({url}): {e}"))?;

        let status = response.status();
        let text = response.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format!("[molinar] {op} failed ({url}): {status} {text}"));
        }

        serde_json::from_str::<Value>(&text)
            .map_err(|e| format!("[molinar] {op} decode failed ({url}): {e}"))
    }

    pub fn with_source(value: Value) -> Value {
        match value {
            Value::Object(mut map) => {
                map.insert("source".to_string(), Value::String("molinar".to_string()));
                Value::Object(map)
            }
            other => json!({
                "source": "molinar",
                "data": other,
            }),
        }
    }

    // ── API Methods ──────────────────────────────────────────────────────

    /// GET /{botId}/state
    pub fn get_state(&self, bot_id: &str) -> Result<Value, String> {
        let url = format!("{}/{}/state", self.api_endpoint, bot_id);
        let value = self.get_json(&url, "get_state")?;
        Ok(Self::with_source(value))
    }

    /// GET /{botId}/look
    pub fn look(&self, bot_id: &str) -> Result<Value, String> {
        let url = format!("{}/{}/look", self.api_endpoint, bot_id);
        let value = self.get_json(&url, "look")?;
        Ok(Self::with_source(value))
    }

    /// POST /{botId}/move
    pub fn move_bot(&self, bot_id: &str, payload: Value) -> Result<Value, String> {
        let url = format!("{}/{}/move", self.api_endpoint, bot_id);
        let value = self.post_json(&url, &payload, "move")?;
        Ok(Self::with_source(value))
    }

    /// POST /{botId}/jump
    pub fn jump(&self, bot_id: &str) -> Result<Value, String> {
        let url = format!("{}/{}/jump", self.api_endpoint, bot_id);
        let value = self.post_json(&url, &json!({}), "jump")?;
        Ok(Self::with_source(value))
    }

    /// POST /{botId}/chat
    pub fn send_chat(&self, bot_id: &str, message: &str) -> Result<Value, String> {
        let url = format!("{}/{}/chat", self.api_endpoint, bot_id);
        let body = json!({ "message": message });
        let value = self.post_json(&url, &body, "chat")?;
        Ok(Self::with_source(value))
    }

    /// GET /{botId}/chat
    pub fn get_chat(&self, bot_id: &str) -> Result<Value, String> {
        let url = format!("{}/{}/chat", self.api_endpoint, bot_id);
        let value = self.get_json(&url, "get_chat")?;
        Ok(Self::with_source(value))
    }

    /// GET /{botId}/chat/new
    pub fn get_new_messages(&self, bot_id: &str) -> Result<Value, String> {
        let url = format!("{}/{}/chat/new", self.api_endpoint, bot_id);
        let value = self.get_json(&url, "get_new_messages")?;
        Ok(Self::with_source(value))
    }

    /// GET /{botId}/players
    pub fn get_players(&self, bot_id: &str) -> Result<Value, String> {
        let url = format!("{}/{}/players", self.api_endpoint, bot_id);
        let value = self.get_json(&url, "get_players")?;
        Ok(Self::with_source(value))
    }

    /// POST /{botId}/collect
    pub fn collect_coins(&self, bot_id: &str) -> Result<Value, String> {
        let url = format!("{}/{}/collect", self.api_endpoint, bot_id);
        let value = self.post_json(&url, &json!({}), "collect")?;
        Ok(Self::with_source(value))
    }

    /// POST /{botId}/explore
    pub fn explore(&self, bot_id: &str) -> Result<Value, String> {
        let url = format!("{}/{}/explore", self.api_endpoint, bot_id);
        let value = self.post_json(&url, &json!({}), "explore")?;
        Ok(Self::with_source(value))
    }

    /// POST /{botId}/create
    pub fn create_object(&self, bot_id: &str, prompt: &str) -> Result<Value, String> {
        let url = format!("{}/{}/create", self.api_endpoint, bot_id);
        let body = json!({ "prompt": prompt });
        let value = self.post_json(&url, &body, "create_object")?;
        Ok(Self::with_source(value))
    }

    /// POST /{botId}/customize
    pub fn customize(&self, bot_id: &str, payload: Value) -> Result<Value, String> {
        let url = format!("{}/{}/customize", self.api_endpoint, bot_id);
        let value = self.post_json(&url, &payload, "customize")?;
        Ok(Self::with_source(value))
    }

    /// POST /{botId}/ping
    pub fn ping(&self, bot_id: &str, payload: Value) -> Result<Value, String> {
        let url = format!("{}/{}/ping", self.api_endpoint, bot_id);
        let value = self.post_json(&url, &payload, "ping")?;
        Ok(Self::with_source(value))
    }
}

// ============================================================================
// Helper: extract bot_id from context
// ============================================================================

pub(crate) fn get_bot_id(ctx: &DynToolCallCtx) -> Result<String, String> {
    ctx.state_attributes
        .get("bot_id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| {
            "bot_id not found in context state_attributes — ensure the integration sets bot_id"
                .to_string()
        })
}

// ============================================================================
// Tool 1: Get World State
// ============================================================================

pub(crate) struct MolinarGetState;

#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MolinarGetStateArgs {}
