//! Post-build validation: load the built plugin, read its manifest, and
//! check that none of its tool names collide with tools from the host-side
//! namespaces the plugin declares.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use aomi_sdk::{DynFnHandle, DynManifest};

// ── Known host-side namespace tools ──────────────────────────────────────────

fn namespace_tools() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    m.insert(
        "common",
        vec![
            "brave_search",
            "send_transaction_to_wallet",
            "send_eip712_to_wallet",
            "encode_and_view",
            "encode_and_simulate",
            "get_time_and_onchain_context",
            "get_contract_abi",
            "get_contract_source_code",
            "get_contract_from_etherscan",
            "get_account_info",
            "get_account_transaction_history",
        ],
    );

    m.insert(
        "database",
        vec![
            "admin_create_api_key",
            "admin_list_api_keys",
            "admin_update_api_key",
            "admin_list_users",
            "admin_update_user",
            "admin_delete_user",
            "admin_list_sessions",
            "admin_update_session",
            "admin_delete_session",
            "admin_list_contracts",
            "admin_update_contract",
            "admin_delete_contract",
        ],
    );

    m.insert("forge", vec!["set_execution_plan", "next_groups"]);

    m
}

fn private_namespaces() -> &'static [&'static str] {
    &["common", "database", "forge"]
}

// ── FFI helpers ──────────────────────────────────────────────────────────────

fn read_manifest(path: &Path) -> Result<DynManifest, String> {
    let handle =
        unsafe { DynFnHandle::load(path).map_err(|e| format!("dlopen {}: {e}", path.display()))? };
    handle
        .call_manifest()
        .map_err(|e| format!("manifest read failed for {}: {e}", path.display()))
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Validate a built plugin library.
///
/// Returns a list of error messages (empty = pass).
pub fn validate_plugin(lib_path: &Path) -> Vec<String> {
    let manifest = match read_manifest(lib_path) {
        Ok(m) => m,
        Err(e) => {
            return vec![format!("{}: {e}", lib_path.display())];
        }
    };

    validate_manifest(&manifest)
}

fn validate_manifest(manifest: &DynManifest) -> Vec<String> {
    let mut errors = Vec::new();

    let ns_tools = namespace_tools();

    if let Some(ref declared) = manifest.namespaces {
        for ns in declared {
            if private_namespaces()
                .iter()
                .any(|private_ns| private_ns == &ns.as_str())
            {
                errors.push(format!(
                    "{}: namespace '{}' is private to the host and not allowed in aomi-apps",
                    manifest.name, ns
                ));
            }
        }
    }

    // Collect all host-side tool names the plugin will inherit.
    let mut inherited: HashSet<&str> = HashSet::new();
    // CommonNamespace is always injected (common_namespace: true by default).
    if let Some(tools) = ns_tools.get("common") {
        inherited.extend(tools.iter());
    }
    if let Some(ref declared) = manifest.namespaces {
        for ns in declared {
            if let Some(tools) = ns_tools.get(ns.as_str()) {
                inherited.extend(tools.iter());
            }
        }
    }

    // Check each plugin tool against inherited names.
    let mut seen = HashSet::new();
    for tool in &manifest.tools {
        if inherited.contains(tool.name.as_str()) {
            errors.push(format!(
                "{}: tool '{}' collides with a host namespace tool",
                manifest.name, tool.name,
            ));
        }
        if !seen.insert(&tool.name) {
            errors.push(format!(
                "{}: duplicate tool '{}' in manifest",
                manifest.name, tool.name,
            ));
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use aomi_sdk::{DYN_ABI_VERSION, DynManifest, DynToolMetadata};

    #[test]
    fn validate_rejects_private_host_namespaces() {
        let manifest = DynManifest {
            abi_version: DYN_ABI_VERSION,
            name: "bad-app".to_string(),
            version: "0.1.0".to_string(),
            preamble: "x".to_string(),
            tools: vec![DynToolMetadata {
                name: "bad_tool".to_string(),
                app: "bad-app".to_string(),
                description: "x".to_string(),
                parameters_schema: aomi_sdk::serde_json::json!({}),
                supports_async: false,
                namespace: None,
            }],
            namespaces: Some(vec!["database".to_string()]),
        };

        let errors = super::validate_manifest(&manifest);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("namespace 'database' is private"));
    }
}
