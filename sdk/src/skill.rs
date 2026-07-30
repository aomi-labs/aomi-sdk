//! App-scoped skill manifest: structured, versioned instruction + guard
//! content that ships inside the app artifact and activates by app binding.
//!
//! Unlike host skills (model-selected via `activate_skills`), an app skill is
//! **server-selected**: the host bakes its sections into the app's composed
//! preamble at app-build time and pre-activates it for hook/guard purposes.
//! It is never catalog-visible and no model text can summon it into another
//! app.
//!
//! The [`GuardTable`] here is the shared guard *data* schema ("code stays
//! compiled, data goes hot"): the host's compiled guard interpreters read
//! these tables at dispatch. App tables ride the release artifact; host
//! skill tables will ride the skills bundle through the same shape.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Whole-skill token budget (chars/4 estimate over all sections). Larger than
/// the 4k host-skill activation budget: an app skill is the app's whole
/// model-facing contract, not one of several activations competing for
/// context.
pub const APP_SKILL_TOKEN_BUDGET: usize = 8_000;

/// Canonical Solana cluster wire forms accepted in [`SvmGuard::clusters`].
pub const CLUSTERS: [&str; 4] = ["mainnet-beta", "devnet", "testnet", "localnet"];

/// One guard table. Host skills and app skills produce the same shape; the
/// host's compiled interpreter hooks enforce it at dispatch.
///
/// Allowlist semantics: an **empty** allowlist leaves that axis
/// unconstrained; a **non-empty** allowlist is strict (anything outside it is
/// vetoed). Tables only ever narrow — a table can never permit what another
/// table or a compiled host guard vetoed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardTable {
    /// Owning skill id — `"<app>/<slug>"` for app skills, the bare skill id
    /// for host skills.
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evm: Option<EvmGuard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub svm: Option<SvmGuard>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<LimitGuard>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvmGuard {
    /// Named contracts: `"ROUTER" -> "0x…"` (20-byte hex address).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub contracts: BTreeMap<String, String>,
    /// Named 4-byte selectors: `"DEPOSIT" -> "0xd0e30db0"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub selectors: BTreeMap<String, String>,
    /// Allowlists reference the named keys above.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_contracts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approve_spenders: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_selectors: Vec<String>,
    /// Chains this guard applies on. Required non-empty — an EVM guard
    /// without a chain scope cannot be interpreted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chain_ids: Vec<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SvmGuard {
    /// Named programs: `"ROUTER" -> base58 pubkey` (32 bytes).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub program_ids: BTreeMap<String, String>,
    /// Named 8-byte Anchor discriminators: `"ROUTE" -> "0xe517cb977ae3ad2a"`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub discriminators: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_programs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_discriminators: Vec<String>,
    /// Clusters this guard applies on (canonical wire forms, see
    /// [`CLUSTERS`]). Empty = any cluster.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clusters: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitGuard {
    /// Hard cap: block staged actions whose declared notional exceeds this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_notional_usd: Option<u64>,
    /// Soft cap: require explicit user confirmation above this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm_above_usd: Option<u64>,
}

/// One named markdown section of an app skill (e.g. `instructions`,
/// `workflows`, `action_rules`, `safety`). Rendered in declaration order
/// into the app's composed preamble.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSkillSection {
    pub name: String,
    pub content: String,
}

/// Host tool-hook binding declared by an app skill: attach the named
/// (host-compiled) hooks to `tool`. Hook names are validated against the
/// host's hook registry at app load — unknown names refuse the load, and
/// dispatch fails closed on any name that slips through.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynToolHookBinding {
    pub tool: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pre_call: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub post_call: Vec<String>,
}

/// The app skill block of a [`crate::DynManifest`]: structured instruction
/// sections, an optional guard table, and host hook bindings — versioned and
/// digest-covered with the release artifact. The skill's version **is** the
/// app version; updating skill content means shipping a new release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSkillManifest {
    /// `"<app-name>/<slug>"`, e.g. `"world-markets/trading"`.
    pub id: String,
    pub sections: Vec<AppSkillSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub guard: Option<GuardTable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hooks: Vec<DynToolHookBinding>,
    /// sha256 over the canonicalized id + sections + guard table, computed
    /// by [`AppSkillManifest::from_parts`]. Lets the host verify the skill
    /// content it loaded is exactly what the release digest covers.
    pub content_digest: String,
}

impl AppSkillManifest {
    /// Assemble the manifest from macro-collected parts, parsing the guard
    /// JSON and computing the content digest.
    ///
    /// # Panics
    ///
    /// Panics when `guard_json` is not a valid [`GuardTable`] — surfaced at
    /// `aomi-build compile` time (the post-build `inspect_plugin` pass calls
    /// `manifest()`), never in a validated production artifact.
    pub fn from_parts(
        id: &str,
        sections: Vec<(&str, &str)>,
        guard_json: Option<&str>,
        hooks: Vec<DynToolHookBinding>,
    ) -> Self {
        let guard = guard_json.map(|json| {
            serde_json::from_str::<GuardTable>(json).unwrap_or_else(|err| {
                panic!("app skill `{id}`: guard.json is not a valid GuardTable: {err}")
            })
        });
        let sections: Vec<AppSkillSection> = sections
            .into_iter()
            .map(|(name, content)| AppSkillSection {
                name: name.to_string(),
                content: content.to_string(),
            })
            .collect();
        let content_digest = digest(id, &sections, guard.as_ref());
        Self {
            id: id.to_string(),
            sections,
            guard,
            hooks,
            content_digest,
        }
    }

    /// Rough token estimate over every section (chars/4, same estimator as
    /// the host skill bundle).
    pub fn est_tokens(&self) -> usize {
        let chars: usize = self
            .sections
            .iter()
            .map(|section| section.content.chars().count())
            .sum();
        chars.div_ceil(4)
    }

    /// Structural validation, shared by `aomi-build compile` (refuses the
    /// build) and the host app loader (refuses the load). `app_name` is the
    /// owning manifest's app name; the skill id must be scoped under it.
    pub fn validate(&self, app_name: &str) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        match self.id.split_once('/') {
            Some((app, slug))
                if app == app_name
                    && !slug.is_empty()
                    && slug
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "-_".contains(c)) =>
            {
            }
            _ => errors.push(format!(
                "skill id `{}` must be `{app_name}/<slug>` with a lowercase slug",
                self.id
            )),
        }

        if self.sections.is_empty() {
            errors.push("skill declares no sections".to_string());
        }
        let mut seen = std::collections::HashSet::new();
        for section in &self.sections {
            if section.name.trim().is_empty() {
                errors.push("skill section with empty name".to_string());
            } else if !seen.insert(section.name.as_str()) {
                errors.push(format!("duplicate skill section `{}`", section.name));
            }
            if section.content.trim().is_empty() {
                errors.push(format!("skill section `{}` is empty", section.name));
            }
        }

        let est = self.est_tokens();
        if est > APP_SKILL_TOKEN_BUDGET {
            errors.push(format!(
                "skill sections estimate {est} tokens, over the {APP_SKILL_TOKEN_BUDGET} budget"
            ));
        }

        for binding in &self.hooks {
            if binding.tool.trim().is_empty() {
                errors.push("hook binding with empty tool name".to_string());
            }
            if binding.pre_call.is_empty() && binding.post_call.is_empty() {
                errors.push(format!(
                    "hook binding for `{}` declares no hooks",
                    binding.tool
                ));
            }
            for name in binding.pre_call.iter().chain(binding.post_call.iter()) {
                if name.trim().is_empty() {
                    errors.push(format!("hook binding for `{}` has an empty hook name", binding.tool));
                }
            }
        }

        if let Some(guard) = &self.guard {
            if guard.id != self.id {
                errors.push(format!(
                    "guard table id `{}` must match skill id `{}`",
                    guard.id, self.id
                ));
            }
            validate_guard(guard, &mut errors);
        }

        let expected = digest(&self.id, &self.sections, self.guard.as_ref());
        if self.content_digest != expected {
            errors.push(format!(
                "content_digest mismatch: manifest says {}, content hashes to {expected}",
                self.content_digest
            ));
        }

        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

fn validate_guard(guard: &GuardTable, errors: &mut Vec<String>) {
    if let Some(evm) = &guard.evm {
        for (name, address) in &evm.contracts {
            if !is_hex_bytes(address, 20) {
                errors.push(format!(
                    "evm contract `{name}` is not a 20-byte 0x-hex address: `{address}`"
                ));
            }
        }
        for (name, selector) in &evm.selectors {
            if !is_hex_bytes(selector, 4) {
                errors.push(format!(
                    "evm selector `{name}` is not a 4-byte 0x-hex value: `{selector}`"
                ));
            }
        }
        for (list, keys, table) in [
            ("allowed_contracts", &evm.allowed_contracts, &evm.contracts),
            ("approve_spenders", &evm.approve_spenders, &evm.contracts),
        ] {
            for key in keys {
                if !table.contains_key(key) {
                    errors.push(format!("evm {list} references undeclared contract `{key}`"));
                }
            }
        }
        for key in &evm.allowed_selectors {
            if !evm.selectors.contains_key(key) {
                errors.push(format!(
                    "evm allowed_selectors references undeclared selector `{key}`"
                ));
            }
        }
        if evm.chain_ids.is_empty() {
            errors.push("evm guard requires non-empty chain_ids".to_string());
        }
    }

    if let Some(svm) = &guard.svm {
        for (name, pubkey) in &svm.program_ids {
            match bs58::decode(pubkey).into_vec() {
                Ok(bytes) if bytes.len() == 32 => {}
                _ => errors.push(format!(
                    "svm program `{name}` is not a base58 32-byte pubkey: `{pubkey}`"
                )),
            }
        }
        for (name, discriminator) in &svm.discriminators {
            if !is_hex_bytes(discriminator, 8) {
                errors.push(format!(
                    "svm discriminator `{name}` is not an 8-byte 0x-hex value: `{discriminator}`"
                ));
            }
        }
        for key in &svm.allowed_programs {
            if !svm.program_ids.contains_key(key) {
                errors.push(format!(
                    "svm allowed_programs references undeclared program `{key}`"
                ));
            }
        }
        for key in &svm.allowed_discriminators {
            if !svm.discriminators.contains_key(key) {
                errors.push(format!(
                    "svm allowed_discriminators references undeclared discriminator `{key}`"
                ));
            }
        }
        for cluster in &svm.clusters {
            if !CLUSTERS.contains(&cluster.as_str()) {
                errors.push(format!(
                    "svm cluster `{cluster}` is not canonical (expected one of {CLUSTERS:?})"
                ));
            }
        }
    }

    if let Some(limits) = &guard.limits
        && let (Some(max), Some(confirm)) = (limits.max_notional_usd, limits.confirm_above_usd)
        && confirm > max
    {
        errors.push(format!(
            "limits confirm_above_usd ({confirm}) exceeds max_notional_usd ({max})"
        ));
    }
}

fn is_hex_bytes(value: &str, bytes: usize) -> bool {
    value
        .strip_prefix("0x")
        .is_some_and(|hex| hex.len() == bytes * 2 && hex.chars().all(|c| c.is_ascii_hexdigit()))
}

/// sha256 over the canonical rendering of id + sections + guard. The guard
/// serializes through its typed form (`BTreeMap`s, fixed field order), so the
/// digest is stable across whitespace/key-order differences in the source
/// `guard.json`.
fn digest(id: &str, sections: &[AppSkillSection], guard: Option<&GuardTable>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    hasher.update([0u8]);
    for section in sections {
        hasher.update(section.name.as_bytes());
        hasher.update([0u8]);
        hasher.update(section.content.as_bytes());
        hasher.update([0u8]);
    }
    if let Some(guard) = guard {
        let canonical = serde_json::to_value(guard).unwrap_or(Value::Null).to_string();
        hasher.update(canonical.as_bytes());
    }
    let mut out = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guard_json() -> &'static str {
        r#"{
            "id": "world-markets/trading",
            "evm": {
                "contracts": { "ROUTER": "0x1111111111111111111111111111111111111111" },
                "selectors": { "DEPOSIT": "0xd0e30db0" },
                "allowed_contracts": ["ROUTER"],
                "allowed_selectors": ["DEPOSIT"],
                "chain_ids": [1]
            },
            "svm": {
                "program_ids": { "ROUTER": "JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4" },
                "discriminators": { "ROUTE": "0xe517cb977ae3ad2a" },
                "allowed_programs": ["ROUTER"],
                "allowed_discriminators": ["ROUTE"],
                "clusters": ["mainnet-beta"]
            },
            "limits": { "max_notional_usd": 10000, "confirm_above_usd": 1000 }
        }"#
    }

    fn skill() -> AppSkillManifest {
        AppSkillManifest::from_parts(
            "world-markets/trading",
            vec![
                ("instructions", "Trade world markets."),
                ("safety", "Never exceed limits."),
            ],
            Some(guard_json()),
            vec![DynToolHookBinding {
                tool: "build_world_trade".to_string(),
                pre_call: vec!["value_at_risk".to_string()],
                post_call: vec![],
            }],
        )
    }

    #[test]
    fn valid_skill_passes_validation() {
        skill().validate("world-markets").expect("valid skill");
    }

    #[test]
    fn digest_is_deterministic_and_covers_content() {
        let a = skill();
        let b = skill();
        assert_eq!(a.content_digest, b.content_digest);

        let mut c = skill();
        c.sections[0].content.push_str(" (edited)");
        let recomputed = digest(&c.id, &c.sections, c.guard.as_ref());
        assert_ne!(c.content_digest, recomputed, "digest must cover sections");
        assert!(
            c.validate("world-markets").is_err(),
            "stale digest must fail validation"
        );
    }

    #[test]
    fn id_must_be_scoped_under_the_app() {
        let s = skill();
        let errors = s.validate("other-app").expect_err("wrong app must fail");
        assert!(errors.iter().any(|e| e.contains("must be `other-app/")));
    }

    #[test]
    fn guard_allowlists_must_reference_declared_keys() {
        let mut s = skill();
        s.guard
            .as_mut()
            .unwrap()
            .evm
            .as_mut()
            .unwrap()
            .allowed_contracts
            .push("UNDECLARED".to_string());
        s.content_digest = digest(&s.id, &s.sections, s.guard.as_ref());
        let errors = s.validate("world-markets").expect_err("must fail");
        assert!(errors.iter().any(|e| e.contains("undeclared contract `UNDECLARED`")));
    }

    #[test]
    fn guard_rejects_malformed_bytes() {
        let mut s = skill();
        {
            let evm = s.guard.as_mut().unwrap().evm.as_mut().unwrap();
            evm.selectors
                .insert("BAD".to_string(), "0x123".to_string());
            let svm = s.guard.as_mut().unwrap().svm.as_mut().unwrap();
            svm.program_ids
                .insert("BAD".to_string(), "not-base58!!".to_string());
            svm.clusters.push("mainnet".to_string()); // alias, not canonical
        }
        s.content_digest = digest(&s.id, &s.sections, s.guard.as_ref());
        let errors = s.validate("world-markets").expect_err("must fail");
        assert!(errors.iter().any(|e| e.contains("selector `BAD`")));
        assert!(errors.iter().any(|e| e.contains("program `BAD`")));
        assert!(errors.iter().any(|e| e.contains("cluster `mainnet`")));
    }

    #[test]
    fn invalid_guard_json_panics_with_context() {
        let result = std::panic::catch_unwind(|| {
            AppSkillManifest::from_parts("a/b", vec![("i", "x")], Some("{ not json"), vec![])
        });
        assert!(result.is_err());
    }

    #[test]
    fn empty_sections_and_over_budget_fail() {
        let empty = AppSkillManifest::from_parts("a/b", vec![], None, vec![]);
        assert!(empty.validate("a").is_err());

        let big = "x".repeat((APP_SKILL_TOKEN_BUDGET + 1) * 4);
        let over = AppSkillManifest::from_parts("a/b", vec![("instructions", big.as_str())], None, vec![]);
        let errors = over.validate("a").expect_err("over budget must fail");
        assert!(errors.iter().any(|e| e.contains("over the")));
    }
}
