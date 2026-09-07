//! `aomi-build manifest --lib <path>` — print a built plugin's DynManifest as
//! JSON. Consumed by platform-repo CI (community-apps `build_candidate.py`) to
//! record each app's declared secret slots in the release `manifest.json`.

use clap::Args;
use eyre::Result;
use std::path::{Path, PathBuf};

use crate::compile::validate::read_manifest;

#[derive(Args, Debug)]
pub struct ManifestArgs {
    /// Path to the built cdylib (`.so` / `.dylib`).
    #[arg(long)]
    pub lib: PathBuf,
}

pub(crate) fn manifest_json(lib: &Path) -> Result<String, String> {
    let manifest = read_manifest(lib)?;
    serde_json::to_string_pretty(&manifest).map_err(|e| format!("serialize manifest: {e}"))
}

pub fn run(args: ManifestArgs) -> Result<()> {
    match manifest_json(&args.lib) {
        Ok(json) => {
            println!("{json}");
            Ok(())
        }
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_json_errors_for_a_missing_library() {
        let err = manifest_json(Path::new("/nonexistent/libnope.so"))
            .expect_err("a missing library must not produce a manifest");
        assert!(err.contains("dlopen"), "got: {err}");
    }

    #[test]
    fn a_manifest_with_secrets_serializes_the_slots() {
        // Guards the exact contract build_candidate.py depends on:
        // `secrets` is an array of {name, description, required}.
        use aomi_sdk::{DynManifest, SecretSlot};
        let manifest = DynManifest {
            sdk_version: "3.0.2".into(),
            name: "binance".into(),
            version: "0.1.0".into(),
            preamble: String::new(),
            tools: vec![],
            namespaces: None,
            secrets: Some(vec![SecretSlot {
                name: "BINANCE_API_KEY".into(),
                description: "Binance dashboard API key.".into(),
                required: true,
            }]),
            broadcast: None,
            evm_execution: None,
            skills: vec![],
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&manifest).unwrap()).unwrap();
        assert_eq!(json["secrets"][0]["name"], "BINANCE_API_KEY");
        assert_eq!(json["secrets"][0]["required"], true);
    }
}
