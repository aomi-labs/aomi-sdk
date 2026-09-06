//! Per-app secret declarations.
//!
//! Each plugin declares the named credentials it needs via the
//! `secrets = [...]` field on the [`dyn_aomi_app!`](crate::dyn_aomi_app)
//! macro. The host reads the declared slots from the plugin manifest and
//! gates app load on the user having ingested every `required: true` slot
//! into the runtime secret vault. At tool-call time the host pre-resolves
//! the slots for this app and injects raw values into
//! [`DynToolCallCtx::secrets`](crate::DynToolCallCtx::secrets); tools read
//! them via [`resolve_secret_value`](crate::resolve_secret_value). That is
//! the only channel: the plugin never reads the host process environment.
//!
//! ```rust,ignore
//! use aomi_sdk::Secret;
//!
//! const KEY: Secret = Secret::new(
//!     "LIMITLESS_API_KEY",
//!     "Limitless CTF Exchange API key id (from the dashboard).",
//!     true,
//! );
//! const SECRET: Secret = Secret::new(
//!     "LIMITLESS_API_SECRET",
//!     "Limitless API secret, base64-encoded as shown in the dashboard.",
//!     true,
//! );
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
/// declare slots at module scope without runtime initialization.
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
}

impl Secret {
    /// Declare a secret slot. `const fn` so plugins can keep declarations
    /// at module scope alongside the rest of their constants.
    pub const fn new(name: &'static str, description: &'static str, required: bool) -> Self {
        Self {
            name,
            description,
            required,
        }
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
}

impl From<&Secret> for SecretSlot {
    fn from(s: &Secret) -> Self {
        Self {
            name: s.name.to_string(),
            description: s.description.to_string(),
            required: s.required,
        }
    }
}

impl From<Secret> for SecretSlot {
    fn from(s: Secret) -> Self {
        (&s).into()
    }
}
