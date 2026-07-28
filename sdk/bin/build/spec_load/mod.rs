//! Loading + preprocessing OpenAPI specs to satisfy progenitor.
//!
//! Real-world specs frequently violate progenitor's strict expectations.
//! Each helper here is a small, named patch with a printed summary so the
//! caller knows what was rewritten.

use eyre::{Context, Result};
use std::path::Path;

mod naming;
mod patch;

// Re-exported so existing call sites (`crate::spec_load::snake_case`, …) keep
// working after the split.
pub use naming::{default_server_url, escape_keyword, pascal_case, snake_case};
use patch::*;

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
    let n = fill_missing_operation_ids(&mut spec);
    if n > 0 {
        println!("  filled in {n} missing operationId(s)");
    }
    let n = rename_wildcard_content_types(&mut spec);
    if n > 0 {
        println!("  renamed {n} `*/*` content type(s) → application/json");
    }
    let n = dedupe_success_responses(&mut spec);
    if n > 0 {
        println!("  dropped {n} duplicate success response(s) (progenitor allows only one)");
    }
    let n = drop_param_name_collisions(&mut spec);
    if n > 0 {
        println!("  dropped {n} parameter(s) whose snake_case name collided with a path param");
    }
    let n = drop_multipart_ops(&mut spec);
    if n > 0 {
        println!(
            "  dropped {n} operation(s) with multipart request bodies (progenitor doesn't support multipart)"
        );
    }
    let n = force_path_params_required(&mut spec);
    if n > 0 {
        println!("  forced {n} path param(s) to required: true (progenitor asserts this)");
    }
    let n = stub_missing_schema_refs(&mut spec);
    if n > 0 {
        println!("  stubbed {n} missing schema $ref(s) as additionalProperties: true");
    }
    let n = inject_missing_path_params(&mut spec);
    if n > 0 {
        println!(
            "  injected {n} missing path param declaration(s) (spec had {{name}} in path but no parameter entry)"
        );
    }
    let n = retype_path_params_as_string(&mut spec);
    if n > 0 {
        println!("  retyped {n} untyped path param(s) as string (default for path placeholders)");
    }
    let n = retype_pagination_as_integer(&mut spec);
    if n > 0 {
        println!(
            "  retyped {n} pagination param(s) (limit/page/offset/...) from number to integer"
        );
    }
    let n = collapse_request_body_to_json(&mut spec);
    if n > 0 {
        println!(
            "  collapsed {n} request body/ies to application/json only (progenitor allows one media type)"
        );
    }
    let n = collapse_response_content_to_json(&mut spec);
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
