//! Post-build validation: load the built plugin, read its manifest, and
//! check that none of its tool names collide with tools from the host-side
//! namespaces the plugin declares.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::Path;

use aomi_sdk::{AOMI_SDK_VERSION, DynFnHandle, DynManifest};

/// Render the app's resolved permission manifest — the human-readable form
/// of its guard table with selectors/discriminators derived and the
/// declared⇒allowed defaults expanded. Printed at compile so guard drift is
/// caught at release review; `None` when the app ships no guard.
pub(crate) fn render_permissions(manifest: &DynManifest) -> Option<String> {
    let rendered: Vec<String> = manifest
        .skills
        .iter()
        .filter_map(render_skill_permissions)
        .collect();
    if rendered.is_empty() {
        None
    } else {
        Some(rendered.join("\n"))
    }
}

fn render_skill_permissions(skill: &aomi_sdk::AppSkillManifest) -> Option<String> {
    let guard = skill.guard.as_ref()?;
    let mut out = String::new();
    let _ = writeln!(out, "  permissions ({}):", guard.id);

    if let Some(evm) = &guard.evm {
        let chains = evm
            .chain_ids
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        let allowed: Vec<&String> = if evm.allowed_contracts.is_empty() {
            evm.contracts.keys().collect()
        } else {
            evm.allowed_contracts.iter().collect()
        };
        let selectors: Vec<&String> = if evm.allowed_selectors.is_empty() {
            evm.selectors.keys().collect()
        } else {
            evm.allowed_selectors.iter().collect()
        };
        for contract in &allowed {
            let address = evm
                .contracts
                .get(*contract)
                .map(String::as_str)
                .unwrap_or("?");
            if selectors.is_empty() {
                let _ = writeln!(
                    out,
                    "    evm[{chains}]: {contract} ({address}) — any selector"
                );
            }
            for name in &selectors {
                let authored = evm.selectors.get(*name).map(String::as_str).unwrap_or("?");
                let derived = aomi_sdk::resolve_selector(authored)
                    .map(|bytes| {
                        format!(
                            "0x{}",
                            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
                        )
                    })
                    .unwrap_or_else(|_| "<invalid>".to_string());
                let _ = writeln!(
                    out,
                    "    evm[{chains}]: {contract}.{authored} [{derived}] ({address})"
                );
            }
        }
        if !evm.approve_spenders.is_empty() {
            let _ = writeln!(
                out,
                "    evm approve spenders: {}",
                evm.approve_spenders.join(", ")
            );
        }
    }

    if let Some(svm) = &guard.svm {
        let clusters = if svm.clusters.is_empty() {
            "any".to_string()
        } else {
            svm.clusters.join(", ")
        };
        let programs: Vec<&String> = if svm.allowed_programs.is_empty() {
            svm.program_ids.keys().collect()
        } else {
            svm.allowed_programs.iter().collect()
        };
        let discriminators: Vec<&String> = if svm.allowed_discriminators.is_empty() {
            svm.discriminators.keys().collect()
        } else {
            svm.allowed_discriminators.iter().collect()
        };
        for program in &programs {
            let pubkey = svm
                .program_ids
                .get(*program)
                .map(String::as_str)
                .unwrap_or("?");
            let calls = if discriminators.is_empty() {
                "any instruction".to_string()
            } else {
                discriminators
                    .iter()
                    .filter_map(|name| svm.discriminators.get(*name).map(String::as_str))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let _ = writeln!(out, "    svm[{clusters}]: {program} ({pubkey}) — {calls}");
        }
    }

    if let Some(limits) = &guard.limits {
        if let Some(hard) = limits.hard_cap {
            let _ = writeln!(out, "    limit: hard_cap {hard}");
        }
        if let Some(confirm) = limits.confirm_cap {
            let _ = writeln!(out, "    limit: confirm_cap {confirm}");
        }
    }

    Some(out.trim_end().to_string())
}

// ── Known host-side namespace tools ──────────────────────────────────────────

fn namespace_tools() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m = HashMap::new();

    m.insert(
        "evm-core",
        vec![
            "brave_search",
            "commit_tx",
            "commit_message",
            "stage_tx",
            "simulate_batch",
            "view_state",
            "run_tx",
            "get_time_and_onchain_context",
            "get_contract",
            "get_account_info",
            "sync_chain",
        ],
    );

    m.insert(
        "svm-core",
        vec![
            "svm_commit_ix",
            "svm_commit_tx",
            "svm_get_account_info",
            "svm_get_context",
            "svm_get_program",
            "svm_get_token_holdings",
            "svm_sign_data",
            "svm_sign_tx",
            "svm_simulate_ix",
            "svm_simulate_tx",
            "svm_stage_ix",
            "svm_stage_tx",
        ],
    );
    m.insert(
        "svm-reads",
        vec![
            "svm_get_account_info",
            "svm_get_context",
            "svm_get_program",
            "svm_get_token_holdings",
        ],
    );
    m.insert(
        "svm-ix-broadcast",
        vec!["svm_commit_ix", "svm_simulate_ix", "svm_stage_ix"],
    );
    m.insert("svm-ix-sign", vec!["svm_simulate_ix", "svm_stage_ix"]);
    m.insert(
        "svm-tx-broadcast",
        vec!["svm_commit_tx", "svm_simulate_tx", "svm_stage_tx"],
    );
    m.insert(
        "svm-tx-sign",
        vec!["svm_sign_tx", "svm_simulate_tx", "svm_stage_tx"],
    );
    m.insert("svm-sign-data", vec!["svm_sign_data"]);
    m.insert("svm-bundle", vec![]);

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
    &["database", "forge"]
}

// ── FFI helpers ──────────────────────────────────────────────────────────────

pub(crate) fn read_manifest(path: &Path) -> Result<DynManifest, String> {
    let handle =
        unsafe { DynFnHandle::load(path).map_err(|e| format!("dlopen {}: {e}", path.display()))? };
    handle
        .call_manifest()
        .map_err(|e| format!("manifest read failed for {}: {e}", path.display()))
}

// ── Public API ───────────────────────────────────────────────────────────────

/// Load a built plugin, read its manifest, and validate it.
///
/// Returns the manifest on success, or the list of validation errors.
pub fn inspect_plugin(lib_path: &Path) -> Result<DynManifest, Vec<String>> {
    let manifest = match read_manifest(lib_path) {
        Ok(m) => m,
        Err(e) => return Err(vec![format!("{}: {e}", lib_path.display())]),
    };

    let mut errors = validate_manifest(&manifest);
    if manifest.sdk_version != AOMI_SDK_VERSION {
        errors.push(format!(
            "{}: plugin sdk_version '{}' does not match repo sdk version '{}'",
            manifest.name, manifest.sdk_version, AOMI_SDK_VERSION
        ));
    }

    if errors.is_empty() {
        Ok(manifest)
    } else {
        Err(errors)
    }
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
                    "{}: namespace '{}' is private to the host and not allowed in aomi-sdk",
                    manifest.name, ns
                ));
            }
        }
    }

    // Collect all host-side tool names the plugin explicitly inherits.
    let mut inherited: HashSet<&str> = HashSet::new();
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

    // App-skill blocks: structural validation (id scoping, sections, token
    // activation budget, guard-table bytes and allowlist references, digest,
    // and unique ids).
    // Shares the validator with the host's app loader — a build that
    // passes here loads.
    if let Err(skill_errors) = aomi_sdk::validate_app_skills(&manifest.name, &manifest.skills) {
        errors.extend(
            skill_errors
                .into_iter()
                .map(|error| format!("{}: {error}", manifest.name)),
        );
    }

    errors
}

#[cfg(test)]
mod tests {
    use aomi_sdk::{AOMI_SDK_VERSION, DynManifest, DynToolMetadata};

    #[test]
    fn validate_rejects_private_host_namespaces() {
        let manifest = DynManifest {
            sdk_version: AOMI_SDK_VERSION.to_string(),
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
            secrets: None,
            broadcast: None,
            evm_execution: None,
            skills: vec![],
        };

        let errors = super::validate_manifest(&manifest);

        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("namespace 'database' is private"));
    }

    #[test]
    fn validate_rejects_a_misscoped_skill() {
        let manifest = DynManifest {
            sdk_version: AOMI_SDK_VERSION.to_string(),
            name: "good-app".to_string(),
            version: "0.1.0".to_string(),
            preamble: "x".to_string(),
            tools: vec![],
            namespaces: None,
            secrets: None,
            broadcast: None,
            evm_execution: None,
            skills: vec![aomi_sdk::AppSkillManifest::from_parts(
                "other-app/trading",
                "Trade",
                vec![],
                vec![("instructions", "content")],
                None,
                vec![],
            )],
        };

        let errors = super::validate_manifest(&manifest);

        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("must be `good-app/"),
            "got: {}",
            errors[0]
        );
    }

    #[test]
    fn validate_allows_separate_guards_and_rejects_undescribed_skills() {
        let guard = r#"{"evm":{"contracts":{"R":"0x1111111111111111111111111111111111111111"},"chain_ids":[1]}}"#;
        let manifest = DynManifest {
            sdk_version: AOMI_SDK_VERSION.to_string(),
            name: "good-app".to_string(),
            version: "0.1.0".to_string(),
            preamble: "x".to_string(),
            tools: vec![],
            namespaces: None,
            secrets: None,
            broadcast: None,
            evm_execution: None,
            skills: vec![
                aomi_sdk::AppSkillManifest::from_parts(
                    "good-app/one",
                    "one",
                    vec![],
                    vec![("instructions", "a")],
                    Some(guard),
                    vec![],
                ),
                aomi_sdk::AppSkillManifest::from_parts(
                    "good-app/two",
                    "two",
                    vec![],
                    vec![("instructions", "b")],
                    Some(guard),
                    vec![],
                ),
                aomi_sdk::AppSkillManifest::from_parts(
                    "good-app/playbook",
                    "",
                    vec![],
                    vec![("workflow", "c")],
                    None,
                    vec![],
                ),
            ],
        };

        let errors = super::validate_manifest(&manifest);
        assert!(
            errors.iter().any(|e| e.contains("needs a description")),
            "{errors:?}"
        );
    }
}

#[cfg(test)]
mod inspect_tests {
    use super::*;

    #[test]
    fn inspect_plugin_reports_errors_for_a_missing_library() {
        let errors = inspect_plugin(Path::new("/nonexistent/libnope.so"))
            .expect_err("a missing library must not inspect cleanly");
        assert_eq!(errors.len(), 1);
        assert!(errors[0].contains("dlopen"), "got: {}", errors[0]);
    }
}
