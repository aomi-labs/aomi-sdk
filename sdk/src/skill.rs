//! App-scoped skill manifest: structured, versioned instruction + guard
//! content that ships inside the app artifact. Every app skill joins only its
//! bound app's catalog and reaches the model through `activate_skills`.
//!
//! The [`GuardTable`] here is the shared guard *data* schema ("code stays
//! compiled, data goes hot"): the host's compiled guard interpreters read
//! these tables at dispatch. App tables ride the release artifact; host
//! skill tables will ride the skills bundle through the same shape.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// Whole-skill token budget (chars/4 estimate over all sections). App skills
/// activate through the host's skill engine and share its default 4k window.
pub const APP_SKILL_TOKEN_BUDGET: usize = 4_000;

/// Canonical Solana cluster wire forms accepted in [`SvmGuard::clusters`].
pub const CLUSTERS: [&str; 4] = ["mainnet-beta", "devnet", "testnet", "localnet"];

/// One guard table. Host skills and app skills produce the same shape; the
/// host's compiled interpreter hooks enforce it at dispatch.
///
/// Allowlist semantics — **declared implies allowed**: an omitted/empty
/// allowlist with a non-empty declaration map means *every declared name*;
/// a non-empty allowlist is a strict subset. An axis with nothing declared
/// is unconstrained. Tables only ever narrow — a table can never permit
/// what another table or a compiled host guard vetoed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuardTable {
    /// Owning skill id — `"<app>/<slug>"` for app skills, the bare skill id
    /// for host skills. Optional in authored `guard.json`:
    /// [`AppSkillManifest::from_parts`] defaults it to the skill id.
    #[serde(default)]
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
    /// Named selectors. The value is either a 4-byte hex selector
    /// (`"0xd0e30db0"`) or a canonical Solidity function signature
    /// (`"deposit()"`, `"buy(uint256,address)"`) derived via
    /// `keccak256(sig)[..4]` — see [`resolve_selector`]. The wire keeps the
    /// authored form; derivation happens at validation and enforcement.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub selectors: BTreeMap<String, String>,
    /// Allowlists reference the named keys above. Omitted = every declared
    /// name (see [`GuardTable`] semantics).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_contracts: Vec<String>,
    /// Extra narrowing axis, NOT an allowlist: empty = `approve()` spender
    /// unrestricted (the approve call itself still needs an allowed
    /// target + selector). Non-empty = only these named contracts may be
    /// approved as spenders.
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
    /// Named discriminators. The value is either an 8-byte hex value
    /// (`"0xe517cb977ae3ad2a"`) or an Anchor instruction name (`"route"`)
    /// derived via `sha256("global:<name>")[..8]` — see
    /// [`resolve_discriminator`]. The wire keeps the authored form.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub discriminators: BTreeMap<String, String>,
    /// Allowlists reference the named keys above. Omitted = every declared
    /// name (see [`GuardTable`] semantics).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_programs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_discriminators: Vec<String>,
    /// Clusters this guard applies on (canonical wire forms, see
    /// [`CLUSTERS`]). Empty = any cluster.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clusters: Vec<String>,
}

/// Numeric caps on a tool-call argument the host compares at dispatch.
///
/// Units are app-defined: the value is whatever number the limited arg
/// carries (USD whole dollars, token base units, share count, …). The
/// guard does not interpret currency — it only compares the arg to these
/// thresholds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LimitGuard {
    /// Reject the call when the limited numeric arg exceeds this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hard_cap: Option<u64>,
    /// Require explicit user confirmation when the limited numeric arg
    /// exceeds this (must be ≤ `hard_cap` when both are set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm_cap: Option<u64>,
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
    /// One-line intent rendered into the pass-1 skill index.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// "When to use" keywords for the skill index.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
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
        description: &str,
        tags: Vec<String>,
        sections: Vec<(&str, &str)>,
        guard_json: Option<&str>,
        hooks: Vec<DynToolHookBinding>,
    ) -> Self {
        let guard = guard_json.map(|json| {
            let mut guard = serde_json::from_str::<GuardTable>(json).unwrap_or_else(|err| {
                panic!("app skill `{id}`: guard.json is not a valid GuardTable: {err}")
            });
            // Authored guard.json may omit `id` — it defaults to the skill
            // id (filled before the digest, so the artifact carries it).
            if guard.id.is_empty() {
                guard.id = id.to_string();
            }
            guard
        });
        let sections: Vec<AppSkillSection> = sections
            .into_iter()
            .map(|(name, content)| AppSkillSection {
                name: name.to_string(),
                content: content.to_string(),
            })
            .collect();
        let content_digest = digest(id, description, &sections, guard.as_ref());
        Self {
            id: id.to_string(),
            description: description.trim().to_string(),
            tags,
            sections,
            guard,
            hooks,
            content_digest,
        }
    }

    /// The slug half of `"<app>/<slug>"` (the whole id if unscoped).
    pub fn slug(&self) -> &str {
        self.id.rsplit('/').next().unwrap_or(&self.id)
    }

    /// Render every section as `### {name}\n{content}` blocks joined by a
    /// blank line for the activated skill instruction payload.
    pub fn render_sections(&self) -> String {
        self.sections
            .iter()
            .map(|section| format!("### {}\n{}", section.name, section.content.trim()))
            .collect::<Vec<_>>()
            .join("\n\n")
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
                    && slug.chars().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || "-_".contains(c)
                    }) => {}
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
        if self.description.trim().is_empty() {
            errors.push(format!(
                "skill `{}` needs a description (the model selects it from the skill index)",
                self.id
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
                    errors.push(format!(
                        "hook binding for `{}` has an empty hook name",
                        binding.tool
                    ));
                }
            }
        }

        if let Some(guard) = &self.guard {
            // Empty = defaulted by `from_parts`; only an explicit id can
            // mismatch.
            if !guard.id.is_empty() && guard.id != self.id {
                errors.push(format!(
                    "guard table id `{}` must match skill id `{}`",
                    guard.id, self.id
                ));
            }
            validate_guard(guard, &mut errors);
        }

        let expected = digest(
            &self.id,
            &self.description,
            &self.sections,
            self.guard.as_ref(),
        );
        if self.content_digest != expected {
            errors.push(format!(
                "content_digest mismatch: manifest says {}, content hashes to {expected}",
                self.content_digest
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Resolve a selector value to its 4 bytes: exact `0x` hex, or a canonical
/// Solidity function signature (`"buy(uint256,address)"` — no spaces, full
/// type names) derived via `keccak256(sig)[..4]`. The single source of the
/// resolution rule — the host's guard resolver calls this too, so validation
/// and enforcement can never drift.
pub fn resolve_selector(value: &str) -> Result<[u8; 4], String> {
    if value.starts_with("0x") {
        return parse_hex_bytes(value)
            .ok_or_else(|| format!("`{value}` is not a 4-byte 0x-hex selector"));
    }
    if !value.ends_with(')') || !value.contains('(') || value.contains(char::is_whitespace) {
        return Err(format!(
            "`{value}` is neither 4-byte 0x-hex nor a canonical function signature \
             like `buy(uint256,address)` (no spaces, full type names)"
        ));
    }
    use tiny_keccak::{Hasher, Keccak};
    let mut hasher = Keccak::v256();
    hasher.update(value.as_bytes());
    let mut out = [0u8; 32];
    hasher.finalize(&mut out);
    Ok([out[0], out[1], out[2], out[3]])
}

/// Resolve a discriminator value to its 8 bytes: exact `0x` hex, or an
/// Anchor instruction name (`"route"`) derived via
/// `sha256("global:<name>")[..8]` — the same convention the host interpreter
/// applies to `encode` specs at dispatch.
pub fn resolve_discriminator(value: &str) -> Result<[u8; 8], String> {
    if value.starts_with("0x") {
        return parse_hex_bytes(value)
            .ok_or_else(|| format!("`{value}` is not an 8-byte 0x-hex discriminator"));
    }
    let is_anchor_name = !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !is_anchor_name {
        return Err(format!(
            "`{value}` is neither 8-byte 0x-hex nor an Anchor instruction name \
             like `route` (lowercase snake_case)"
        ));
    }
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(format!("global:{value}").as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&hash[..8]);
    Ok(out)
}

fn parse_hex_bytes<const N: usize>(value: &str) -> Option<[u8; N]> {
    let hex = value.strip_prefix("0x")?;
    if hex.len() != N * 2 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut out = [0u8; N];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(out)
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
            if let Err(error) = resolve_selector(selector) {
                errors.push(format!("evm selector `{name}`: {error}"));
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
            if let Err(error) = resolve_discriminator(discriminator) {
                errors.push(format!("svm discriminator `{name}`: {error}"));
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
        && let (Some(hard), Some(confirm)) = (limits.hard_cap, limits.confirm_cap)
        && confirm > hard
    {
        errors.push(format!(
            "limits confirm_cap ({confirm}) exceeds hard_cap ({hard})"
        ));
    }
}

fn is_hex_bytes(value: &str, bytes: usize) -> bool {
    value
        .strip_prefix("0x")
        .is_some_and(|hex| hex.len() == bytes * 2 && hex.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Validate a whole `skills` set: every entry valid on its own and ids unique.
pub fn validate_app_skills(app_name: &str, skills: &[AppSkillManifest]) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for skill in skills {
        if let Err(mut own) = skill.validate(app_name) {
            errors.append(&mut own);
        }
        if !seen.insert(skill.id.as_str()) {
            errors.push(format!("duplicate app skill id `{}`", skill.id));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// sha256 over the canonical rendering of id + description +
/// sections + guard. The guard serializes through its typed form
/// (`BTreeMap`s, fixed field order), so the digest is stable across
/// whitespace/key-order differences in the source `guard.json`.
fn digest(
    id: &str,
    description: &str,
    sections: &[AppSkillSection],
    guard: Option<&GuardTable>,
) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(id.as_bytes());
    hasher.update([0u8]);
    hasher.update(description.trim().as_bytes());
    hasher.update([0u8]);
    for section in sections {
        hasher.update(section.name.as_bytes());
        hasher.update([0u8]);
        hasher.update(section.content.as_bytes());
        hasher.update([0u8]);
    }
    if let Some(guard) = guard {
        let canonical = serde_json::to_value(guard)
            .unwrap_or(Value::Null)
            .to_string();
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

    #[test]
    fn description_enters_the_digest_and_wire_form_has_no_mode() {
        let skill = AppSkillManifest::from_parts(
            "app/x",
            "when the task needs x",
            vec!["x".into()],
            vec![("instructions", "hi")],
            None,
            vec![],
        );
        let described = AppSkillManifest::from_parts(
            "app/x",
            "described",
            vec![],
            vec![("instructions", "hi")],
            None,
            vec![],
        );
        assert_ne!(
            skill.content_digest, described.content_digest,
            "description is digest-covered"
        );
        assert!(described.validate("app").is_ok());
        assert!(skill.validate("app").is_ok());

        let json = serde_json::to_string(&skill).unwrap();
        assert!(!json.contains("activation"));
        let round_trip: AppSkillManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip, skill);
    }

    #[test]
    fn skill_requires_a_description_and_fits_the_activation_budget() {
        let undescribed = AppSkillManifest::from_parts(
            "app/x",
            "   ",
            vec![],
            vec![("instructions", "hi")],
            None,
            vec![],
        );
        let errors = undescribed.validate("app").unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("needs a description")),
            "{errors:?}"
        );

        let big = "x".repeat(APP_SKILL_TOKEN_BUDGET * 4 + 4);
        let over = AppSkillManifest::from_parts(
            "app/x",
            "big",
            vec![],
            vec![("instructions", &big)],
            None,
            vec![],
        );
        let errors = over.validate("app").unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("over the 4000 budget")),
            "{errors:?}"
        );
    }

    #[test]
    fn validate_app_skills_enforces_unique_ids_and_allows_separate_guards() {
        let guard = r#"{"evm":{"contracts":{"R":"0x1111111111111111111111111111111111111111"},"chain_ids":[1]}}"#;
        let a = AppSkillManifest::from_parts(
            "app/a",
            "a",
            vec![],
            vec![("instructions", "a")],
            Some(guard),
            vec![],
        );
        let b = AppSkillManifest::from_parts(
            "app/b",
            "b",
            vec![],
            vec![("instructions", "b")],
            Some(guard),
            vec![],
        );
        let c = AppSkillManifest::from_parts(
            "app/c",
            "c",
            vec![],
            vec![("workflow", "c")],
            Some(guard),
            vec![],
        );
        assert!(
            validate_app_skills("app", &[a.clone(), c.clone()]).is_ok(),
            "each activated skill may carry its own guard"
        );
        assert!(validate_app_skills("app", &[a.clone(), b]).is_ok());
        let errors = validate_app_skills("app", &[a.clone(), a]).unwrap_err();
        assert!(
            errors.iter().any(|e| e.contains("duplicate app skill id")),
            "{errors:?}"
        );
        assert!(validate_app_skills("app", &[]).is_ok());
    }

    #[test]
    fn render_sections_preserves_declared_order() {
        let skill = AppSkillManifest::from_parts(
            "app/x",
            "x",
            vec![],
            vec![("instructions", "  one  "), ("safety", "two\n")],
            None,
            vec![],
        );
        assert_eq!(
            skill.render_sections(),
            "### instructions\none\n\n### safety\ntwo"
        );
        assert_eq!(skill.slug(), "x");
    }

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
            "limits": { "hard_cap": 10000, "confirm_cap": 1000 }
        }"#
    }

    fn skill() -> AppSkillManifest {
        AppSkillManifest::from_parts(
            "world-markets/trading",
            "Trade world markets",
            vec![],
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
        let recomputed = digest(&c.id, &c.description, &c.sections, c.guard.as_ref());
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
        s.content_digest = digest(&s.id, &s.description, &s.sections, s.guard.as_ref());
        let errors = s.validate("world-markets").expect_err("must fail");
        assert!(
            errors
                .iter()
                .any(|e| e.contains("undeclared contract `UNDECLARED`"))
        );
    }

    #[test]
    fn guard_rejects_malformed_bytes() {
        let mut s = skill();
        {
            let evm = s.guard.as_mut().unwrap().evm.as_mut().unwrap();
            evm.selectors.insert("BAD".to_string(), "0x123".to_string());
            let svm = s.guard.as_mut().unwrap().svm.as_mut().unwrap();
            svm.program_ids
                .insert("BAD".to_string(), "not-base58!!".to_string());
            svm.clusters.push("mainnet".to_string()); // alias, not canonical
        }
        s.content_digest = digest(&s.id, &s.description, &s.sections, s.guard.as_ref());
        let errors = s.validate("world-markets").expect_err("must fail");
        assert!(errors.iter().any(|e| e.contains("selector `BAD`")));
        assert!(errors.iter().any(|e| e.contains("program `BAD`")));
        assert!(errors.iter().any(|e| e.contains("cluster `mainnet`")));
    }

    #[test]
    fn selector_values_resolve_from_hex_or_signature() {
        // Known vectors: keccak256("transfer(address,uint256)")[..4] and
        // keccak256("approve(address,uint256)")[..4].
        assert_eq!(
            resolve_selector("transfer(address,uint256)").unwrap(),
            [0xa9, 0x05, 0x9c, 0xbb]
        );
        assert_eq!(
            resolve_selector("approve(address,uint256)").unwrap(),
            [0x09, 0x5e, 0xa7, 0xb3]
        );
        assert_eq!(
            resolve_selector("0xa9059cbb").unwrap(),
            resolve_selector("transfer(address,uint256)").unwrap()
        );
        assert!(resolve_selector("0x123").is_err(), "short hex");
        assert!(
            resolve_selector("transfer (address)").is_err(),
            "spaces are not canonical"
        );
        assert!(resolve_selector("not a signature").is_err());
    }

    #[test]
    fn discriminator_values_resolve_from_hex_or_anchor_name() {
        use sha2::{Digest, Sha256};
        let expected: [u8; 8] = Sha256::digest(b"global:route")[..8].try_into().unwrap();
        assert_eq!(resolve_discriminator("route").unwrap(), expected);
        assert_eq!(
            resolve_discriminator("0xe517cb977ae3ad2a").unwrap(),
            [0xe5, 0x17, 0xcb, 0x97, 0x7a, 0xe3, 0xad, 0x2a]
        );
        assert!(resolve_discriminator("Route").is_err(), "not snake_case");
        assert!(resolve_discriminator("0x12").is_err(), "short hex");
    }

    #[test]
    fn minimal_authored_guard_validates_with_defaults() {
        // What a developer actually writes: no id, no allowlists, signature
        // selectors. Declared ⇒ allowed; id defaults to the skill id.
        let guard = r#"{
            "evm": {
                "contracts": { "ROUTER": "0x1111111111111111111111111111111111111111" },
                "selectors": { "BUY": "buy(uint256,address)" },
                "chain_ids": [1]
            }
        }"#;
        let skill = AppSkillManifest::from_parts(
            "world-markets/trading",
            "Trade world markets",
            vec![],
            vec![("instructions", "Trade.")],
            Some(guard),
            vec![],
        );
        skill
            .validate("world-markets")
            .expect("minimal guard is valid");
        assert_eq!(
            skill.guard.as_ref().unwrap().id,
            "world-markets/trading",
            "id defaults to the skill id before the digest"
        );
        // Deterministic digest over the filled form.
        let again = AppSkillManifest::from_parts(
            "world-markets/trading",
            "Trade world markets",
            vec![],
            vec![("instructions", "Trade.")],
            Some(guard),
            vec![],
        );
        assert_eq!(skill.content_digest, again.content_digest);
    }

    #[test]
    fn invalid_guard_json_panics_with_context() {
        let result = std::panic::catch_unwind(|| {
            AppSkillManifest::from_parts(
                "a/b",
                "b",
                vec![],
                vec![("i", "x")],
                Some("{ not json"),
                vec![],
            )
        });
        assert!(result.is_err());
    }

    #[test]
    fn empty_sections_and_over_budget_fail() {
        let empty = AppSkillManifest::from_parts("a/b", "b", vec![], vec![], None, vec![]);
        assert!(empty.validate("a").is_err());

        let big = "x".repeat((APP_SKILL_TOKEN_BUDGET + 1) * 4);
        let over = AppSkillManifest::from_parts(
            "a/b",
            "b",
            vec![],
            vec![("instructions", big.as_str())],
            None,
            vec![],
        );
        let errors = over.validate("a").expect_err("over budget must fail");
        assert!(errors.iter().any(|e| e.contains("over the")));
    }
}
