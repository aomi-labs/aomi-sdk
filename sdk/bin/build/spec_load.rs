//! Loading + preprocessing OpenAPI specs to satisfy progenitor, plus the
//! naming helpers (snake/pascal case, keyword escaping) shared with the tool
//! generator. The individual spec patches live in `spec_patch`, each with a
//! printed summary so the caller knows what was rewritten.

use eyre::{Context, Result};
use std::path::Path;

use crate::spec_patch as patch;

pub fn load_and_preprocess(spec_path: &Path) -> Result<openapiv3::OpenAPI> {
    let spec_text = std::fs::read_to_string(spec_path)
        .with_context(|| format!("failed to read {}", spec_path.display()))?;
    let spec_text = downgrade_to_30(&spec_text);
    let mut spec: openapiv3::OpenAPI =
        if spec_path.extension().and_then(|e| e.to_str()) == Some("json") {
            serde_json::from_str(&spec_text).context("spec is not valid JSON")?
        } else {
            serde_yaml::from_str(&spec_text).context("spec is not valid YAML")?
        };
    let n = patch::fill_missing_operation_ids(&mut spec);
    if n > 0 {
        println!("  filled in {n} missing operationId(s)");
    }
    let n = patch::rename_wildcard_content_types(&mut spec);
    if n > 0 {
        println!("  renamed {n} `*/*` content type(s) → application/json");
    }
    let n = patch::dedupe_success_responses(&mut spec);
    if n > 0 {
        println!("  dropped {n} duplicate success response(s) (progenitor allows only one)");
    }
    let n = patch::drop_param_name_collisions(&mut spec);
    if n > 0 {
        println!("  dropped {n} parameter(s) whose snake_case name collided with a path param");
    }
    let n = patch::drop_multipart_ops(&mut spec);
    if n > 0 {
        println!(
            "  dropped {n} operation(s) with multipart request bodies (progenitor doesn't support multipart)"
        );
    }
    let n = patch::force_path_params_required(&mut spec);
    if n > 0 {
        println!("  forced {n} path param(s) to required: true (progenitor asserts this)");
    }
    let n = patch::stub_missing_schema_refs(&mut spec);
    if n > 0 {
        println!("  stubbed {n} missing schema $ref(s) as additionalProperties: true");
    }
    let n = patch::inject_missing_path_params(&mut spec);
    if n > 0 {
        println!(
            "  injected {n} missing path param declaration(s) (spec had {{name}} in path but no parameter entry)"
        );
    }
    let n = patch::retype_path_params_as_string(&mut spec);
    if n > 0 {
        println!("  retyped {n} untyped path param(s) as string (default for path placeholders)");
    }
    let n = patch::retype_pagination_as_integer(&mut spec);
    if n > 0 {
        println!(
            "  retyped {n} pagination param(s) (limit/page/offset/...) from number to integer"
        );
    }
    let n = patch::collapse_request_body_to_json(&mut spec);
    if n > 0 {
        println!(
            "  collapsed {n} request body/ies to application/json only (progenitor allows one media type)"
        );
    }
    let n = patch::collapse_response_content_to_json(&mut spec);
    if n > 0 {
        println!("  collapsed {n} response(s) to one media type (progenitor allows one)");
    }
    // Write the post-processed spec to a sibling .preprocessed.yaml for
    // debugging when progenitor still chokes on something.
    if let Ok(yaml) = serde_yaml::to_string(&spec) {
        let dbg_path = spec_path.with_extension("preprocessed.yaml");
        let _ = std::fs::write(&dbg_path, yaml);
    }
    Ok(spec)
}

/// Text-level pre-parse patch: openapiv3/progenitor only speak 3.0, so rewrite
/// a 3.1 version stamp to 3.0.3 before deserialising.
fn downgrade_to_30(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut changed = false;
    for line in src.lines() {
        if let Some(rest) = line.strip_prefix("openapi: ") {
            let v = rest.trim().trim_matches(|c: char| c == '"' || c == '\'');
            if v.starts_with("3.1") {
                out.push_str("openapi: 3.0.3\n");
                changed = true;
                continue;
            }
        } else if let Some(rest) = line.strip_prefix("\"openapi\": ") {
            let v = rest.trim_end_matches(',').trim().trim_matches('"');
            if v.starts_with("3.1") {
                out.push_str("\"openapi\": \"3.0.3\",\n");
                changed = true;
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    if changed {
        println!("  downgraded openapi 3.1 → 3.0.3 (progenitor doesn't support 3.1)");
    }
    out
}

pub fn snake_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev_lower = false;
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            if prev_lower {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            prev_lower = false;
        } else if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_lower = true;
        } else {
            if !out.ends_with('_') {
                out.push('_');
            }
            prev_lower = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// Append `_` if `s` is a Rust 2024 keyword (matches progenitor's convention).
pub fn escape_keyword(s: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "do", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod",
        "move", "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super",
        "trait", "true", "try", "type", "typeof", "union", "unsafe", "unsized", "use", "virtual",
        "where", "while", "yield",
    ];
    if KEYWORDS.contains(&s) {
        format!("{s}_")
    } else {
        s.to_string()
    }
}

pub fn pascal_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut upper_next = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            if upper_next {
                out.push(ch.to_ascii_uppercase());
            } else {
                out.push(ch);
            }
            upper_next = false;
        } else {
            upper_next = true;
        }
    }
    out
}

/// True for a 2xx status code (or the `2XX` range).
pub fn is_success(code: &openapiv3::StatusCode) -> bool {
    match code {
        openapiv3::StatusCode::Code(c) => (200..300).contains(c),
        openapiv3::StatusCode::Range(2) => true,
        _ => false,
    }
}

pub fn default_server_url(spec: &openapiv3::OpenAPI) -> String {
    spec.servers
        .first()
        .map(|s| s.url.clone())
        .unwrap_or_default()
}
