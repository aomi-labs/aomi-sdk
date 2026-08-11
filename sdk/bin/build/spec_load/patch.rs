//! In-place fixes applied to a parsed spec before codegen.
//!
//! Each pass takes the whole spec, mutates it, and reports how many sites it
//! touched; `mod.rs` runs them in order and prints the totals. Pure functions
//! over the spec — no IO, no shared state.

use super::naming::{is_success, snake_case, synthesize_op_id};

/// progenitor allows exactly one media type per response. When a response
/// declares multiple (e.g. JSON + XML), keep `application/json` if present,
/// else the first remaining type, and drop the rest.
pub(super) fn collapse_response_content_to_json(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut collapsed = 0;
    fn fix(content: &mut indexmap::IndexMap<String, openapiv3::MediaType>) -> bool {
        if content.len() <= 1 {
            return false;
        }
        let keep = if content.contains_key("application/json") {
            Some("application/json".to_string())
        } else {
            content.keys().next().cloned()
        };
        let Some(keep) = keep else {
            return false;
        };
        let to_drop: Vec<String> = content.keys().filter(|k| **k != keep).cloned().collect();
        for k in &to_drop {
            content.shift_remove(k);
        }
        true
    }
    for item_ref in spec.paths.paths.values_mut() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        for op in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ]
        .into_iter()
        .flatten()
        {
            for resp in op.responses.responses.values_mut() {
                if let ReferenceOr::Item(r) = resp
                    && fix(&mut r.content)
                {
                    collapsed += 1;
                }
            }
            if let Some(ReferenceOr::Item(r)) = op.responses.default.as_mut()
                && fix(&mut r.content)
            {
                collapsed += 1;
            }
        }
    }
    collapsed
}

/// progenitor allows exactly one media type per request body. When a spec
/// declares multiple (e.g. JSON + XML + form), keep `application/json` if
/// present, else the first remaining type, and drop the rest.
pub(super) fn collapse_request_body_to_json(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut collapsed = 0;
    for item_ref in spec.paths.paths.values_mut() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        for op in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ]
        .into_iter()
        .flatten()
        {
            let Some(ReferenceOr::Item(rb)) = &mut op.request_body else {
                continue;
            };
            if rb.content.len() <= 1 {
                continue;
            }
            let preferred = if rb.content.contains_key("application/json") {
                Some("application/json".to_string())
            } else {
                rb.content.keys().next().cloned()
            };
            if let Some(keep) = preferred {
                let other_keys: Vec<String> =
                    rb.content.keys().filter(|k| **k != keep).cloned().collect();
                for k in &other_keys {
                    rb.content.shift_remove(k);
                }
                collapsed += 1;
            }
        }
    }
    collapsed
}

/// Walk the entire spec for `$ref: '#/components/schemas/<Name>'` references
/// and create a stub `additionalProperties: true` schema for any that aren't
/// defined. Lets progenitor's typify pass succeed even when the upstream spec
/// has a broken cross-reference (Limitless ships a few of these).
pub(super) fn stub_missing_schema_refs(spec: &mut openapiv3::OpenAPI) -> usize {
    // Serialize → walk JSON for refs → diff vs defined → inject stubs into
    // components.schemas. Cheap and avoids hand-walking openapiv3's enum tree.
    let json = match serde_json::to_string(&spec) {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let prefix = "\"#/components/schemas/";
    let mut referenced: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut idx = 0;
    while let Some(start) = json[idx..].find(prefix) {
        let from = idx + start + prefix.len();
        if let Some(end_offset) = json[from..].find('"') {
            referenced.insert(json[from..from + end_offset].to_string());
            idx = from + end_offset;
        } else {
            break;
        }
    }
    let components = spec.components.get_or_insert_with(Default::default);
    let mut stubbed = 0;
    for name in referenced {
        if components.schemas.contains_key(&name) {
            continue;
        }
        // Build a permissive stub: type=object, additionalProperties=true.
        let stub: openapiv3::Schema = serde_json::from_value(serde_json::json!({
            "type": "object",
            "additionalProperties": true
        }))
        .expect("stub schema literal must parse");
        components
            .schemas
            .insert(name, openapiv3::ReferenceOr::Item(stub));
        stubbed += 1;
    }
    stubbed
}

/// For every `{name}` placeholder in a path that doesn't have a corresponding
/// `parameters` entry on the operation (or path item), inject a default
/// `string` path parameter. Specs occasionally omit these declarations and
/// progenitor refuses to generate the operation otherwise.
pub(super) fn inject_missing_path_params(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ParameterData, ParameterSchemaOrContent, ReferenceOr, Schema};
    let mut injected = 0;
    let path_keys: Vec<String> = spec.paths.paths.keys().cloned().collect();
    for path_key in path_keys {
        // Collect placeholder names
        let mut placeholders: Vec<String> = Vec::new();
        let bytes = path_key.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'{' {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j] != b'}' {
                    j += 1;
                }
                if let Ok(name) = std::str::from_utf8(&bytes[start..j]) {
                    placeholders.push(name.to_string());
                }
                i = j + 1;
            } else {
                i += 1;
            }
        }
        if placeholders.is_empty() {
            continue;
        }
        let Some(item_ref) = spec.paths.paths.get_mut(&path_key) else {
            continue;
        };
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        // Path-level params apply to all ops; collect their names.
        let path_lvl_names: std::collections::HashSet<String> = item
            .parameters
            .iter()
            .filter_map(|p| match p {
                ReferenceOr::Item(Parameter::Path { parameter_data, .. }) => {
                    Some(parameter_data.name.clone())
                }
                _ => None,
            })
            .collect();
        for op_slot in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ] {
            let Some(op) = op_slot else { continue };
            let mut have: std::collections::HashSet<String> = path_lvl_names.clone();
            for p in &op.parameters {
                if let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = p {
                    have.insert(parameter_data.name.clone());
                }
            }
            for name in &placeholders {
                if have.contains(name) {
                    continue;
                }
                let stub_schema: Schema = serde_json::from_value(serde_json::json!({
                    "type": "string"
                }))
                .expect("stub schema literal must parse");
                let param = Parameter::Path {
                    parameter_data: ParameterData {
                        name: name.clone(),
                        description: None,
                        required: true,
                        deprecated: None,
                        format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(stub_schema)),
                        example: None,
                        examples: Default::default(),
                        explode: None,
                        extensions: Default::default(),
                    },
                    style: openapiv3::PathStyle::Simple,
                };
                op.parameters.push(ReferenceOr::Item(param));
                injected += 1;
            }
        }
    }
    injected
}

/// Path params with `schema: { example: "..." }` but NO `type:` make
/// progenitor fall back to `serde_json::Value`, which then double-quotes
/// strings on the wire. Default these to `type: string` (path params are
/// nearly always strings).
pub(super) fn retype_path_params_as_string(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ParameterSchemaOrContent, ReferenceOr, Schema};
    let mut fixed = 0;
    for item_ref in spec.paths.paths.values_mut() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        for op_slot in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ] {
            let Some(op) = op_slot else { continue };
            for p in &mut op.parameters {
                let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = p else {
                    continue;
                };
                let needs_retype = match &parameter_data.format {
                    ParameterSchemaOrContent::Schema(ReferenceOr::Item(schema)) => {
                        matches!(&schema.schema_kind, openapiv3::SchemaKind::Any(_))
                    }
                    _ => false,
                };
                if needs_retype {
                    let stub: Schema = serde_json::from_value(serde_json::json!({
                        "type": "string"
                    }))
                    .expect("stub schema literal must parse");
                    parameter_data.format =
                        ParameterSchemaOrContent::Schema(ReferenceOr::Item(stub));
                    fixed += 1;
                }
            }
        }
    }
    fixed
}

/// Pagination params commonly typed as `number` (== f64) in specs but the
/// server expects integers. Detect by name + retype to `integer`.
pub(super) fn retype_pagination_as_integer(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ParameterSchemaOrContent, ReferenceOr, Schema, SchemaKind, Type};
    let mut fixed = 0;
    const PAGINATION_NAMES: &[&str] = &[
        "limit", "page", "offset", "count", "size", "per_page", "perpage", "pagesize",
    ];
    for item_ref in spec.paths.paths.values_mut() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        for op_slot in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ] {
            let Some(op) = op_slot else { continue };
            for p in &mut op.parameters {
                let ReferenceOr::Item(param) = p else {
                    continue;
                };
                let parameter_data = match param {
                    Parameter::Query { parameter_data, .. }
                    | Parameter::Header { parameter_data, .. }
                    | Parameter::Path { parameter_data, .. }
                    | Parameter::Cookie { parameter_data, .. } => parameter_data,
                };
                let lname = parameter_data.name.to_ascii_lowercase();
                if !PAGINATION_NAMES.contains(&lname.as_str()) {
                    continue;
                }
                let ParameterSchemaOrContent::Schema(ReferenceOr::Item(schema)) =
                    &mut parameter_data.format
                else {
                    continue;
                };
                if matches!(schema.schema_kind, SchemaKind::Type(Type::Number(_))) {
                    let new_schema: Schema = serde_json::from_value(serde_json::json!({
                        "type": "integer",
                        "format": "int64"
                    }))
                    .expect("stub schema literal must parse");
                    *schema = new_schema;
                    fixed += 1;
                }
            }
        }
    }
    fixed
}

/// progenitor asserts `parameter_data.required` on every path param. Specs
/// occasionally violate this (it's an OpenAPI spec bug — path params MUST be
/// required per the spec), so we silently coerce.
pub(super) fn force_path_params_required(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ReferenceOr};
    let mut fixed = 0;
    for item_ref in spec.paths.paths.values_mut() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        for op_slot in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ] {
            let Some(op) = op_slot else { continue };
            for p_ref in &mut op.parameters {
                let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = p_ref else {
                    continue;
                };
                if !parameter_data.required {
                    parameter_data.required = true;
                    fixed += 1;
                }
            }
        }
    }
    fixed
}

/// progenitor doesn't handle `multipart/form-data` request bodies. Drop those
/// operations entirely from the spec — they need hand-written wrappers.
pub(super) fn drop_multipart_ops(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut dropped = 0;
    for item_ref in spec.paths.paths.values_mut() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        for op_slot in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ] {
            if let Some(op) = op_slot.as_ref() {
                let has_multipart = match &op.request_body {
                    Some(ReferenceOr::Item(rb)) => rb.content.contains_key("multipart/form-data"),
                    _ => false,
                };
                if has_multipart {
                    *op_slot = None;
                    dropped += 1;
                }
            }
        }
    }
    dropped
}

pub(super) fn downgrade_to_30(src: &str) -> String {
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

pub(super) fn fill_missing_operation_ids(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut count = 0;
    for (path, item_ref) in spec.paths.paths.iter_mut() {
        let item = match item_ref {
            ReferenceOr::Item(i) => i,
            ReferenceOr::Reference { .. } => continue,
        };
        for (method, op_opt) in [
            ("get", &mut item.get),
            ("put", &mut item.put),
            ("post", &mut item.post),
            ("delete", &mut item.delete),
            ("patch", &mut item.patch),
            ("head", &mut item.head),
            ("options", &mut item.options),
            ("trace", &mut item.trace),
        ] {
            if let Some(op) = op_opt
                && op.operation_id.is_none()
            {
                op.operation_id = Some(synthesize_op_id(method, path));
                count += 1;
            }
        }
    }
    count
}

pub(super) fn rename_wildcard_content_types(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut count = 0;
    fn fix(c: &mut indexmap::IndexMap<String, openapiv3::MediaType>, n: &mut usize) {
        if let Some(media) = c.shift_remove("*/*") {
            c.entry("application/json".to_string()).or_insert(media);
            *n += 1;
        }
    }
    for item_ref in spec.paths.paths.values_mut() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        for op in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ]
        .into_iter()
        .flatten()
        {
            if let Some(ReferenceOr::Item(req)) = &mut op.request_body {
                fix(&mut req.content, &mut count);
            }
            for resp_ref in op.responses.responses.values_mut() {
                if let ReferenceOr::Item(resp) = resp_ref {
                    fix(&mut resp.content, &mut count);
                }
            }
            if let Some(ReferenceOr::Item(resp)) = op.responses.default.as_mut() {
                fix(&mut resp.content, &mut count);
            }
        }
    }
    count
}

pub(super) fn dedupe_success_responses(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::ReferenceOr;
    let mut dropped = 0;
    for item_ref in spec.paths.paths.values_mut() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        for op in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ]
        .into_iter()
        .flatten()
        {
            // Pick which single response to keep:
            //   1. first 2xx with a body (real success type),
            //   2. else first 2xx (empty body — generated client returns ()),
            //   3. else nothing — every other response is dropped.
            //
            // progenitor's assertion is on response_type uniqueness, and a
            // response with no body still counts as `()` — distinct from a
            // typed body. Mixing empty 200 + bodied 202 panics. Keeping only
            // ONE response avoids every variant of that bug.
            let keep: Option<openapiv3::StatusCode> = {
                let bodied_2xx = op.responses.responses.iter().find_map(|(c, r)| {
                    if !is_success(c) {
                        return None;
                    }
                    matches!(r, ReferenceOr::Item(rr) if !rr.content.is_empty()).then(|| c.clone())
                });
                bodied_2xx.or_else(|| {
                    op.responses
                        .responses
                        .iter()
                        .find_map(|(c, _)| is_success(c).then(|| c.clone()))
                })
            };
            let to_remove: Vec<openapiv3::StatusCode> = op
                .responses
                .responses
                .keys()
                .filter(|c| Some(*c) != keep.as_ref())
                .cloned()
                .collect();
            for code in &to_remove {
                op.responses.responses.shift_remove(code);
                dropped += 1;
            }
            // Always drop the `default` response. progenitor counts it as a
            // response_type even when empty, which trips its `<= 1` assertion
            // whenever an op has both a 200 and a `default` (very common).
            if op.responses.default.is_some() {
                op.responses.default = None;
                dropped += 1;
            }
        }
    }
    dropped
}

pub(super) fn drop_param_name_collisions(spec: &mut openapiv3::OpenAPI) -> usize {
    use openapiv3::{Parameter, ReferenceOr};
    let mut dropped = 0;
    for item_ref in spec.paths.paths.values_mut() {
        let ReferenceOr::Item(item) = item_ref else {
            continue;
        };
        for op in [
            &mut item.get,
            &mut item.put,
            &mut item.post,
            &mut item.delete,
            &mut item.patch,
            &mut item.head,
            &mut item.options,
            &mut item.trace,
        ]
        .into_iter()
        .flatten()
        {
            let path_keys: std::collections::HashSet<String> = op
                .parameters
                .iter()
                .filter_map(|p| match p {
                    ReferenceOr::Item(Parameter::Path { parameter_data, .. }) => {
                        Some(snake_case(&parameter_data.name))
                    }
                    _ => None,
                })
                .collect();
            let before = op.parameters.len();
            op.parameters.retain(|p| match p {
                ReferenceOr::Item(Parameter::Query { parameter_data, .. })
                | ReferenceOr::Item(Parameter::Header { parameter_data, .. })
                | ReferenceOr::Item(Parameter::Cookie { parameter_data, .. }) => {
                    !path_keys.contains(&snake_case(&parameter_data.name))
                }
                _ => true,
            });
            dropped += before - op.parameters.len();
        }
    }
    dropped
}
