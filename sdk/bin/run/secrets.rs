//! Resolve declared secret slots from process environment.
//!
//! The backend has a per-app vault; the dev runtime keeps it dead simple:
//! each declared slot maps to an env var of the same name. A required
//! slot that's missing aborts startup; an optional slot that's missing
//! just gets a warning.
//!
//! The resulting `HashMap<String,String>` is injected into every
//! `DynToolCallCtx.secrets`, which means the plugin's
//! `aomi_sdk::resolve_secret_value(ctx, …)` finds the value through its
//! standard lookup chain (`ctx.secrets[name]` beats env directly, but
//! since we read from env that's a wash).

use std::collections::HashMap;

use anyhow::{Result, bail};

use aomi_sdk::DynManifest;

/// Walk `manifest.secrets`, read each slot from env, return the map that
/// will go into `DynToolCallCtx.secrets`.
pub fn resolve(manifest: &DynManifest) -> Result<HashMap<String, String>> {
    let Some(slots) = manifest.secrets.as_deref() else {
        return Ok(HashMap::new());
    };

    let mut out = HashMap::new();
    let mut missing_required = Vec::new();
    let mut missing_optional = Vec::new();

    for slot in slots {
        match std::env::var(&slot.name) {
            Ok(value) if !value.trim().is_empty() => {
                out.insert(slot.name.clone(), value);
            }
            _ => {
                if slot.required {
                    missing_required.push(slot.name.clone());
                } else {
                    missing_optional.push(slot.name.clone());
                }
            }
        }
    }

    if !missing_optional.is_empty() {
        eprintln!(
            "  ⚠ optional secrets unset: {}",
            missing_optional.join(", ")
        );
    }

    if !missing_required.is_empty() {
        bail!(
            "required secrets unset: {}\n  Set them in your environment or pass --env-file.",
            missing_required.join(", ")
        );
    }

    Ok(out)
}
