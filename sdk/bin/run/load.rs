//! Load a plugin cdylib and surface its manifest.
//!
//! This is a thin wrapper around `DynFnHandle::load` + `call_manifest`
//! that prints a friendly startup summary so the developer can see what
//! the runtime actually picked up — tool names, namespaces, secrets,
//! SDK version.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};

use aomi_sdk::{AOMI_SDK_VERSION, DynFnHandle, DynManifest};

pub struct LoadedPlugin {
    pub handle: Arc<DynFnHandle>,
    pub manifest: DynManifest,
    #[allow(dead_code)]
    pub path: PathBuf,
}

/// `dlopen` the plugin, validate the SDK version, and read its manifest.
pub fn load_plugin(path: &Path) -> Result<LoadedPlugin> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("plugin path not found: {}", path.display()))?;

    // SAFETY: developers point this at a file they themselves just built;
    // SDK-version mismatch is rejected inside `DynFnHandle::load`.
    //
    // The SDK uses `eyre`; convert through `anyhow!` so `?` lines up with
    // the rest of the binary's `anyhow::Result`.
    let handle = unsafe { DynFnHandle::load(&canonical) }
        .map_err(|e| anyhow::anyhow!("failed to load plugin at {}: {e}", canonical.display()))?;

    let manifest = handle
        .call_manifest()
        .map_err(|e| anyhow::anyhow!("plugin returned an unparseable manifest: {e}"))?;

    Ok(LoadedPlugin {
        handle: Arc::new(handle),
        manifest,
        path: canonical,
    })
}

/// Print a human-readable startup summary. Stays terse so it doesn't bury
/// the REPL prompt under a wall of text.
pub fn print_manifest_summary(manifest: &DynManifest) {
    eprintln!(
        "▶ {} v{}  (aomi-sdk {})",
        manifest.name, manifest.version, manifest.sdk_version
    );
    if manifest.sdk_version != AOMI_SDK_VERSION {
        // Should never happen — DynFnHandle::load gates on it — but be
        // loud just in case the gate is ever loosened.
        eprintln!(
            "  ⚠ SDK version mismatch: plugin={} host={}",
            manifest.sdk_version, AOMI_SDK_VERSION
        );
    }

    if manifest.tools.is_empty() {
        eprintln!("  tools      : (none)");
    } else {
        eprintln!(
            "  tools      : {} ({})",
            manifest.tools.len(),
            manifest
                .tools
                .iter()
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    match manifest.namespaces.as_deref() {
        None | Some(&[]) => eprintln!("  namespaces : (none)"),
        Some(ns) => eprintln!("  namespaces : {} (stubbed)", ns.join(", ")),
    }

    match manifest.secrets.as_deref() {
        None | Some(&[]) => {}
        Some(slots) => {
            let names: Vec<_> = slots
                .iter()
                .map(|s| {
                    if s.required {
                        format!("{}*", s.name)
                    } else {
                        s.name.clone()
                    }
                })
                .collect();
            eprintln!("  secrets    : {}  (* = required)", names.join(", "));
        }
    }
    eprintln!();
}
