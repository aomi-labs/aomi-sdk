//! Per-app secret declarations.
//!
//! Each plugin declares the named credentials it needs via the
//! `secrets = [...]` field on the [`dyn_aomi_app!`](crate::dyn_aomi_app)
//! macro. The host reads the declared slots from the plugin manifest and
//! gates app load on the user having ingested every `required: true` slot
//! into the runtime secret vault. At tool-call time the host pre-resolves
//! the slots for this app and injects raw values into
//! [`DynToolCallCtx::secrets`](crate::DynToolCallCtx::secrets). User-owned
//! slots must be read with
//! [`resolve_user_secret_value`](crate::resolve_user_secret_value), which
//! cannot fall back to an operator process environment.
//! Plugin code runs natively and must never log these values, include them in
//! errors, persist them, or return them from a tool. SDK-side redaction protects
//! common diagnostics but is not a sandbox for plugin code.
//!
//! ```rust,ignore
//! use aomi_sdk::Secret;
//!
//! const KEY: Secret = Secret::new(
//!     "LIMITLESS_API_KEY",
//!     "Limitless CTF Exchange API key id (from the dashboard).",
//!     true,
//! )
//! .user_owned();
//! const SECRET: Secret = Secret::new(
//!     "LIMITLESS_API_SECRET",
//!     "Limitless API secret, base64-encoded as shown in the dashboard.",
//!     true,
//! )
//! .user_owned();
//!
//! aomi_sdk::dyn_aomi_app!(
//!     app = LimitlessApp,
//!     name = "limitless",
//!     version = "0.1.0",
//!     preamble = "...",
//!     tools = [...],
//!     secrets = [KEY, SECRET],
//!     namespaces = ["evm-core"],
//! );
//! ```

use serde::{Deserialize, Serialize};

/// A secret slot declared by a plugin. Static const-friendly so apps can
/// declare slots at module scope without runtime initialization. Call
/// [`Secret::user_owned`] when every user must provide a separate value;
/// declarations are operator-managed by default.
#[derive(Debug, Clone, Copy)]
pub struct Secret {
    /// Canonical name. Must match the env-var / vault key the tool reads.
    /// Convention: SCREAMING_SNAKE_CASE.
    pub name: &'static str,
    /// One-sentence description shown to users in the settings UI and the
    /// app-load gate modal.
    pub description: &'static str,
    /// `true` if the app cannot load until this slot is filled. `false` if
    /// the app loads and only specific tools fail at call time when missing.
    pub required: bool,
    /// `true` when each user must supply their own value for this slot.
    /// Defaults to `false`, which preserves operator-managed secret behavior.
    pub user_own: bool,
}

impl Secret {
    /// Declare a secret slot. `const fn` so plugins can keep declarations
    /// at module scope alongside the rest of their constants.
    pub const fn new(name: &'static str, description: &'static str, required: bool) -> Self {
        Self {
            name,
            description,
            required,
            user_own: false,
        }
    }

    /// Mark this slot as a credential supplied by each user.
    /// Tools should read it with [`crate::resolve_user_secret_value`].
    pub const fn user_owned(mut self) -> Self {
        self.user_own = true;
        self
    }
}

/// Serialization shape of [`Secret`] that crosses the FFI boundary in
/// [`DynManifest`](crate::DynManifest). The host reads this to populate
/// `/api/control/apps` and decide whether to gate app load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretSlot {
    pub name: String,
    pub description: String,
    pub required: bool,
    /// Whether each user must supply their own value. Missing in manifests
    /// produced by earlier SDK versions means `false`.
    #[serde(default)]
    pub user_own: bool,
}

impl From<&Secret> for SecretSlot {
    fn from(s: &Secret) -> Self {
        Self {
            name: s.name.to_string(),
            description: s.description.to_string(),
            required: s.required,
            user_own: s.user_own,
        }
    }
}

impl From<Secret> for SecretSlot {
    fn from(s: Secret) -> Self {
        (&s).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_default_to_operator_owned() {
        let slot = SecretSlot::from(Secret::new("API_KEY", "API key.", true));

        assert!(!slot.user_own);
        assert_eq!(serde_json::to_value(slot).unwrap()["user_own"], false);
    }

    #[test]
    fn declarations_can_require_a_user_owned_value() {
        let slot = SecretSlot::from(Secret::new("API_KEY", "API key.", true).user_owned());

        assert!(slot.user_own);
        assert_eq!(serde_json::to_value(slot).unwrap()["user_own"], true);
    }

    #[test]
    fn older_manifest_slots_default_to_operator_owned() {
        let slot: SecretSlot = serde_json::from_value(serde_json::json!({
            "name": "API_KEY",
            "description": "API key.",
            "required": true
        }))
        .unwrap();

        assert!(!slot.user_own);
    }
}
